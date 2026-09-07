//! Shared task ledger adapter for the existing Builder Provider contract.
use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelRequest, ModelResponse, VisionModelProvider,
};
use annotagent_storage::{ConversationCallAdmission, ConversationCallStatus, SqliteStore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct ConversationTaskProvider<'a> {
    inner: &'a dyn VisionModelProvider,
    store: &'a SqliteStore,
    project: String,
    task: Uuid,
    scope: String,
    model: String,
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
            inner,
            store: &self.store,
            project: owner,
            task,
            scope: scope.into(),
            model: model.into(),
        })
    }
}

struct PendingCall<'a> {
    provider: &'a ConversationTaskProvider<'a>,
    id: Uuid,
}
impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        let _ = self.provider.store.abandon_conversation_call(
            &self.provider.project,
            self.provider.task,
            self.id,
        );
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
        let id = Uuid::new_v4();
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
        let _guard = PendingCall { provider: self, id };
        let result = self.inner.complete(request, cancellation).await;
        let (status, evidence) = match &result {
            Ok(response) => (
                ConversationCallStatus::Completed,
                serde_json::json!({"phase":"builder_text","response":response}),
            ),
            Err(_) => (
                ConversationCallStatus::InDoubt,
                serde_json::json!({"phase":"builder_text","error":"Provider outcome and cost are unknown; this call remains consumed"}),
            ),
        };
        self.store
            .finish_conversation_call(&self.project, self.task, id, status, evidence)
            .map_err(|_| {
                CoreError::Provider(
                    "Could not persist model receipt; do not assume the call was free".into(),
                )
            })?;
        result
    }
}
