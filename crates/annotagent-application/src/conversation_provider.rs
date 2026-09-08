//! Shared task ledger adapter for the existing Builder Provider contract.
use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelRequest, ModelResponse, VisionModelProvider,
};
use annotagent_storage::{ConversationCallAdmission, ConversationCallStatus, SqliteStore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct ConversationTaskProvider<'a> {
    application: &'a crate::LocalApplication,
    inner: &'a dyn VisionModelProvider,
    store: &'a SqliteStore,
    project: String,
    task: Uuid,
    scope: String,
    model: String,
    recorded_call: Option<(Uuid, serde_json::Value)>,
}

impl crate::LocalApplication {
    /// Caller must resolve the approved Registry configuration with transport retries disabled.
    /// This adapter counts every completion, including Builder repair/retry turns.
    pub fn conversation_text_provider<'a>(
        &'a self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        scope: &str,
        model: &str,
        inner: &'a dyn VisionModelProvider,
    ) -> anyhow::Result<ConversationTaskProvider<'a>> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            anyhow::bail!("task does not belong to this conversation");
        }
        let owner = self.conversation_project_identity(project)?;
        let budget = self
            .store
            .conversation_call_budget(&owner, task)?
            .ok_or_else(|| anyhow::anyhow!("explicit task authorization required"))?;
        if budget.current_grant.scope_hash != scope || budget.revoked {
            anyhow::bail!("task authorization scope changed or was revoked");
        }
        Ok(ConversationTaskProvider {
            application: self,
            inner,
            store: &self.store,
            project: owner,
            task,
            scope: scope.into(),
            model: model.into(),
            recorded_call: None,
        })
    }
}

impl ConversationTaskProvider<'_> {
    /// Fixed identity and frozen evidence for a single bounded feedback call.
    /// The same task ledger and pending-human/expiry/Project limits still apply.
    pub(crate) fn for_feedback_call(mut self, id: Uuid, context: serde_json::Value) -> Self {
        self.recorded_call = Some((id, context));
        self
    }
}

struct PendingCall<'a> {
    provider: &'a ConversationTaskProvider<'a>,
    id: Uuid,
    settled: bool,
}
impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        let _ = self.provider.store.abandon_conversation_call(
            &self.provider.project,
            self.provider.task,
            self.id,
        );
        if self.provider.recorded_call.is_some() {
            if let Ok(mut calls) = self.provider.application.conversation_cancellations.lock() {
                if let Some(token) = calls.remove(&self.id) {
                    if !self.settled {
                        token.cancel();
                    }
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl VisionModelProvider for ConversationTaskProvider<'_> {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn capabilities(&self) -> ModelCapabilities {
        let mut capabilities = self.inner.capabilities();
        capabilities.vision = false;
        capabilities.multi_image = false;
        capabilities
    }
    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "Task cancelled before model admission".into(),
            ));
        }
        if request.model != self.model || !request.images.is_empty() {
            return Err(CoreError::Provider(
                "Text-phase model or image scope exceeded; no request sent".into(),
            ));
        }
        let id = self
            .recorded_call
            .as_ref()
            .map_or_else(Uuid::new_v4, |(id, _)| *id);
        let hash = annotagent_image_tools::sha256(
            &serde_json::to_vec(&request)
                .map_err(|_| CoreError::Provider("Cannot freeze model request".into()))?,
        );
        let admission = self
            .store
            .reserve_conversation_call(&self.project, self.task, id, &self.scope, &hash)
            .map_err(|error| CoreError::Provider(error.to_string()))?;
        if admission != ConversationCallAdmission::Admitted {
            return Err(CoreError::Provider(
                "Existing call cannot be executed twice".into(),
            ));
        }
        let mut guard = PendingCall {
            provider: self,
            id,
            settled: false,
        };
        if self.recorded_call.is_some() {
            self.application
                .conversation_cancellations
                .lock()
                .map_err(|_| CoreError::Provider("Cancellation registry unavailable".into()))?
                .insert(id, cancellation.clone());
            if self
                .store
                .conversation_call_cancellations(&self.project, self.task)
                .map_err(|error| CoreError::Provider(error.to_string()))?
                .iter()
                .any(|item| item.call_id == id)
                || !self
                    .store
                    .conversation_calls_active(&self.project, self.task)
                    .map_err(|error| CoreError::Provider(error.to_string()))?
            {
                cancellation.cancel();
            }
        }
        let sent = !cancellation.is_cancelled();
        let result = if sent {
            self.inner.complete(request, cancellation.clone()).await
        } else {
            Err(CoreError::Provider(
                "Cancelled before sending model request".into(),
            ))
        };
        let (status, mut evidence) = match &result {
            Ok(response) => (
                ConversationCallStatus::Completed,
                serde_json::json!({"phase":"builder_text","response":response}),
            ),
            Err(_) if !sent => (
                ConversationCallStatus::Failed,
                serde_json::json!({"phase":"builder_text","error":"Cancelled before sending model request"}),
            ),
            Err(_) => (
                ConversationCallStatus::InDoubt,
                serde_json::json!({"phase":"builder_text","error":"Provider outcome and cost are unknown; this call remains consumed"}),
            ),
        };
        if let Some((_, context)) = &self.recorded_call {
            evidence["phase"] = serde_json::json!("feedback_text");
            evidence["context"] = context.clone();
            evidence["cancelled"] = serde_json::json!(cancellation.is_cancelled());
        }
        self.store
            .finish_conversation_call(&self.project, self.task, id, status, evidence)
            .map_err(|_| {
                CoreError::Provider(
                    "Could not persist model receipt; do not assume the call was free".into(),
                )
            })?;
        guard.settled = true;
        result
    }
}
