//! Call accounting for existing sample adapters. This is not execution authorization:
//! the caller still must validate exact Draft/model/image scope and disable transport retries.
use annotagent_core::{
    CoreError, CoreResult, PipelineModelBackend, VisionModelBackend, VisionModelProvider,
};
use annotagent_storage::{ConversationCallAdmission, ConversationCallStatus, SqliteStore};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ConversationVisionCalls {
    store: Arc<SqliteStore>,
    project: String,
    task: Uuid,
    scope: String,
}

impl crate::LocalApplication {
    /// Builds accounting adapters only; does not grant permission or start inference.
    pub fn conversation_vision_calls(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        scope: &str,
    ) -> anyhow::Result<ConversationVisionCalls> {
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
        if budget.revoked || budget.current_grant.scope_hash != scope {
            anyhow::bail!("task authorization scope changed or was revoked");
        }
        Ok(ConversationVisionCalls {
            store: self.store.clone(),
            project: owner,
            task,
            scope: scope.into(),
        })
    }
}

impl ConversationVisionCalls {
    pub(crate) fn require_owner(&self, owner: &str) -> anyhow::Result<()> {
        if self.project != owner {
            anyhow::bail!("Sample budget does not belong to this Project");
        }
        Ok(())
    }
    pub fn pipeline(&self, inner: Arc<dyn PipelineModelBackend>) -> Arc<dyn PipelineModelBackend> {
        crate::sample_limits::SampleCalls::conversation(self.clone()).pipeline(inner)
    }
    pub fn provider(&self, inner: Arc<dyn VisionModelProvider>) -> Arc<dyn VisionModelProvider> {
        crate::sample_limits::SampleCalls::conversation(self.clone()).provider(inner)
    }
    pub fn backend(&self, inner: Arc<dyn VisionModelBackend>) -> Arc<dyn VisionModelBackend> {
        crate::sample_limits::SampleCalls::conversation(self.clone()).backend(inner)
    }
    pub(crate) fn begin(&self, request: &impl serde::Serialize) -> CoreResult<VisionCallReceipt> {
        let hash = annotagent_image_tools::sha256(
            &serde_json::to_vec(request)
                .map_err(|_| CoreError::Validation("Cannot freeze sample request".into()))?,
        );
        let id = Uuid::new_v4();
        let admission = self
            .store
            .reserve_conversation_call(&self.project, self.task, id, &self.scope, &hash)
            .map_err(|error| CoreError::Validation(error.to_string()))?;
        if admission != ConversationCallAdmission::Admitted {
            return Err(CoreError::Validation(
                "Existing model call cannot be executed twice".into(),
            ));
        }
        Ok(VisionCallReceipt {
            calls: self.clone(),
            id,
        })
    }
}

pub(crate) struct VisionCallReceipt {
    calls: ConversationVisionCalls,
    id: Uuid,
}
impl VisionCallReceipt {
    pub(crate) fn finish(self, succeeded: bool) -> CoreResult<()> {
        self.calls.store.finish_conversation_call(&self.calls.project, self.calls.task, self.id,
            if succeeded { ConversationCallStatus::Completed } else { ConversationCallStatus::InDoubt },
            serde_json::json!({"phase":"sample_inference","succeeded":succeeded,"cost_known":false,"response_storage":"existing_sample_artifacts"})
        ).map_err(|_| CoreError::Validation("Could not persist model receipt; do not assume the call was free".into()))?;
        Ok(())
    }
}
impl Drop for VisionCallReceipt {
    fn drop(&mut self) {
        let _ = self.calls.store.abandon_conversation_call(
            &self.calls.project,
            self.calls.task,
            self.id,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{PipelineInferenceRequest, PipelineInferenceResponse, VisionCapability};
    use annotagent_storage::{
        BeginConversationTask, ConversationCallGrant, ConversationMessageInput,
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio_util::sync::CancellationToken;

    fn setup(store: Arc<SqliteStore>) -> ConversationVisionCalls {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = ConversationMessageInput {
            reference: None,
            id: Uuid::new_v4(),
            text: "TEST offline allowance".into(),
            image: None,
        };
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        store
            .authorize_conversation_calls(
                &project,
                &ConversationCallGrant {
                    id: Uuid::new_v4(),
                    task_id: task,
                    scope_hash: "b".repeat(64),
                    maximum_calls: 3,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                },
            )
            .unwrap();
        ConversationVisionCalls {
            store,
            project,
            task,
            scope: "b".repeat(64),
        }
    }
    struct Counter(Arc<AtomicU64>);
    #[async_trait::async_trait]
    impl PipelineModelBackend for Counter {
        fn id(&self) -> &str {
            "TEST-counter"
        }
        fn capability(&self) -> VisionCapability {
            VisionCapability::Classification
        }
        async fn infer_pipeline(
            &self,
            _: PipelineInferenceRequest,
            _: CancellationToken,
        ) -> CoreResult<PipelineInferenceResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(PipelineInferenceResponse::default())
        }
    }
    fn request() -> PipelineInferenceRequest {
        PipelineInferenceRequest {
            protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: "TEST".into(),
            run_id: annotagent_core::RunId::new(),
            image_id: annotagent_core::ImageId::new(),
            node_id: "classify".into(),
            model_id: "TEST-counter".into(),
            operation: VisionCapability::Classification,
            image: None,
            input_artifacts: vec![],
            parameters: std::collections::BTreeMap::default(),
            timeout_ms: None,
        }
    }
    #[tokio::test]
    async fn native_calls_share_durable_spend_and_cancel_before_admission() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-calls.db");
        let calls = setup(Arc::new(SqliteStore::open(&path).unwrap()));
        // An interrupted earlier phase remains consumed.
        let abandoned = calls
            .begin(&serde_json::json!({"TEST":"earlier phase"}))
            .unwrap();
        let abandoned_id = abandoned.id;
        drop(abandoned);
        let counter = Arc::new(AtomicU64::new(0));
        let backend = calls.pipeline(Arc::new(Counter(counter.clone())));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(backend.infer_pipeline(request(), cancelled).await.is_err());
        let mut jobs = vec![];
        for _ in 0..12 {
            let backend = backend.clone();
            jobs.push(tokio::spawn(async move {
                backend
                    .infer_pipeline(request(), CancellationToken::new())
                    .await
                    .is_ok()
            }));
        }
        let mut completed = 0;
        for job in jobs {
            completed += usize::from(job.await.unwrap());
        }
        assert_eq!(completed, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        let project = calls.project.clone();
        let task = calls.task;
        drop(backend);
        drop(calls);
        let store = Arc::new(SqliteStore::open(path).unwrap());
        assert_eq!(
            store
                .conversation_call_budget(&project, task)
                .unwrap()
                .unwrap()
                .used_calls,
            3
        );
        assert_eq!(
            store
                .conversation_call(&project, task, abandoned_id)
                .unwrap()
                .unwrap()
                .status,
            ConversationCallStatus::InDoubt
        );
        let restored = ConversationVisionCalls {
            store,
            project,
            task,
            scope: "b".repeat(64),
        };
        assert!(
            restored
                .pipeline(Arc::new(Counter(counter.clone())))
                .infer_pipeline(request(), CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn changed_scope_revocation_and_failure_are_not_free_retries() {
        let calls = setup(Arc::new(SqliteStore::open_in_memory().unwrap()));
        let request = serde_json::json!({"TEST":"request"});
        let receipt = calls.begin(&request).unwrap();
        let id = receipt.id;
        receipt.finish(false).unwrap();
        assert_eq!(
            calls
                .store
                .conversation_call(&calls.project, calls.task, id)
                .unwrap()
                .unwrap()
                .status,
            ConversationCallStatus::InDoubt
        );
        let mut wrong = calls.clone();
        wrong.scope = "c".repeat(64);
        assert!(wrong.begin(&request).is_err());
        calls
            .store
            .revoke_conversation_calls(&calls.project, calls.task)
            .unwrap();
        assert!(calls.begin(&request).is_err());
        assert_eq!(
            calls
                .store
                .conversation_call_budget(&calls.project, calls.task)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
    }

    #[tokio::test]
    async fn per_test_limit_and_task_limit_both_apply_to_late_native_adapters() {
        let calls = setup(Arc::new(SqliteStore::open_in_memory().unwrap()));
        assert!(calls.require_owner("foreign").is_err());
        calls.require_owner(&calls.project).unwrap();
        let count = Arc::new(AtomicU64::new(0));
        let allowance = crate::sample_limits::SampleCalls::bounded_conversation(1, calls.clone());
        let first = allowance.pipeline(Arc::new(Counter(count.clone())));
        first
            .infer_pipeline(request(), CancellationToken::new())
            .await
            .unwrap();
        // Runtime constructs some native adapters only after input Artifacts exist.
        let late = allowance.clone().pipeline(Arc::new(Counter(count.clone())));
        assert!(
            late.infer_pipeline(request(), CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(
            calls
                .store
                .conversation_call_budget(&calls.project, calls.task)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
        // A new sample may reset its local ceiling, never the cumulative task spend.
        let next = crate::sample_limits::SampleCalls::bounded_conversation(10, calls.clone())
            .pipeline(Arc::new(Counter(count.clone())));
        for _ in 0..2 {
            next.infer_pipeline(request(), CancellationToken::new())
                .await
                .unwrap();
        }
        assert!(
            next.infer_pipeline(request(), CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
