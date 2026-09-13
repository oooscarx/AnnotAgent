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
    conversation: Uuid,
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
            conversation,
            task,
            scope: scope.into(),
        })
    }
}

impl ConversationVisionCalls {
    pub(crate) fn attempt_observer_for_snapshot(
        &self,
        snapshot: &annotagent_core::ModelProfileSnapshot,
    ) -> anyhow::Result<Arc<dyn annotagent_provider::ModelAttemptObserver>> {
        let mut model = self
            .store
            .get_model_profile(snapshot.model_profile_id, Some(snapshot.revision))?;
        let mut provider = self.store.get_provider_profile(snapshot.provider_id)?;
        model.provider_id = snapshot.provider_id;
        model.remote_model_id.clone_from(&snapshot.remote_model_id);
        model
            .input_modalities
            .clone_from(&snapshot.input_modalities);
        model.protocol_features = snapshot.protocol_features.clone();
        model
            .task_capabilities
            .clone_from(&snapshot.task_capabilities);
        model.limits = snapshot.limits.clone();
        model
            .generation_defaults
            .clone_from(&snapshot.generation_defaults);
        provider.adapter = snapshot.provider_adapter;
        provider.base_url = snapshot.provider_base_url.clone();
        let selected = crate::PipelineBuilderModelRuntime {
            provider: provider.clone(),
            model: model.clone(),
            binding_source: annotagent_core::ModelBindingSource::WorkflowNode,
            locked: true,
        };
        Ok(Arc::new(crate::ConversationTaskAttemptObserver {
            store: self.store.clone(),
            project_id: self.project.clone(),
            conversation_id: self.conversation,
            task_id: self.task,
            model,
            provider,
            effective_request: serde_json::to_value(selected.effective_request()?)?,
        }))
    }
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
    pub(crate) fn id(&self) -> Uuid {
        self.id
    }
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
    use annotagent_core::{
        CapabilityDeclarationSource, CredentialReference, CredentialSource, GenerationDefaults,
        InputModality, ModelCapability, ModelImage, ModelLimits, ModelMessage, ModelPricing,
        ModelProfile, ModelProfileSnapshot, ModelProfileStatus, ModelRequest, ModelRole,
        PipelineInferenceRequest, PipelineInferenceResponse, PricingSource, ProtocolFeatures,
        ProviderAdapterKind, ProviderConnectionPolicy, ProviderHealthSnapshot,
        ProviderHealthStatus, ProviderProfile, UsageSource, VisionCapability,
    };
    use annotagent_provider::{OpenAiCompatibleConfig, OpenAiCompatibleProvider, OpenAiProtocol};
    use annotagent_storage::{
        BeginConversationTask, ConversationCallGrant, ConversationMessageInput,
    };
    use rust_decimal::Decimal;
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::atomic::{AtomicU64, Ordering},
    };
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
            conversation,
            task,
            scope: "b".repeat(64),
        }
    }

    #[tokio::test]
    async fn authorized_vision_provider_records_exact_physical_attempt_and_cost() {
        async fn completion() -> axum::Json<serde_json::Value> {
            axum::Json(serde_json::json!({
                "id":"TEST-vision-request",
                "choices":[{"message":{"role":"assistant","content":"done"}}],
                "usage":{"prompt_tokens":1500,"completion_tokens":500,"total_tokens":2000,
                    "prompt_tokens_details":{"cached_tokens":0}}
            }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new().route("/v1/chat/completions", axum::routing::post(completion)),
            )
            .await
            .unwrap();
        });

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let calls = setup(store.clone());
        let now = chrono::Utc::now();
        let provider = ProviderProfile {
            id: annotagent_core::ProviderId::new(),
            display_name: "TEST loopback".into(),
            preset_id: Some("TEST".into()),
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: format!("http://{address}/v1").parse().unwrap(),
            organization: None,
            workspace: None,
            credential_ref: Some(CredentialReference {
                provider_id: annotagent_core::ProviderId::new(),
                source: CredentialSource::SessionOnly,
                locator: "TEST".into(),
            }),
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy {
                maximum_retries: 0,
                ..ProviderConnectionPolicy::default()
            },
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("TEST".into()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let provider = ProviderProfile {
            credential_ref: Some(CredentialReference {
                provider_id: provider.id,
                source: CredentialSource::SessionOnly,
                locator: "TEST".into(),
            }),
            ..provider
        };
        store.save_provider_profile(&provider).unwrap();
        let model = ModelProfile {
            id: annotagent_core::ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST vision".into(),
            remote_model_id: "TEST-vision".into(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::VisionLanguage]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing {
                currency: "USD".into(),
                input_per_million_tokens: Some(Decimal::from(2)),
                output_per_million_tokens: Some(Decimal::from(8)),
                cached_input_per_million_tokens: Some(Decimal::ONE),
                per_image: Some(Decimal::ZERO),
                per_request: None,
                source: PricingSource::UserConfigured,
                updated_at: Some(now),
            },
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: true,
            created_at: now,
            updated_at: now,
        };
        store.save_model_profile(&model).unwrap();
        let snapshot = ModelProfileSnapshot::frozen(&model, &provider).unwrap();
        let concrete = OpenAiCompatibleProvider::new_with_api_key(
            OpenAiCompatibleConfig {
                endpoint: provider.base_url.to_string(),
                api_key_env: "UNUSED_TEST".into(),
                model: model.remote_model_id.clone(),
                protocol: OpenAiProtocol::ChatCompletions,
                request_timeout_seconds: 5,
                max_output_tokens: 100,
                temperature: 0.0,
                reasoning_mode: None,
                supports_tool_calls: false,
                supports_json_schema: false,
                streaming: false,
                response_mode: annotagent_provider::OpenAiResponseMode::Automatic,
                custom_headers: BTreeMap::new(),
                extra_request_fields: BTreeMap::new(),
                max_retries: 0,
                minimum_retry_delay_ms: 0,
                maximum_retry_delay_ms: 0,
            },
            Some("TEST-secret".into()),
        )
        .unwrap();
        concrete.set_attempt_observer(calls.attempt_observer_for_snapshot(&snapshot).unwrap());
        let wrapped = calls.provider(Arc::new(concrete));
        wrapped
            .complete(
                ModelRequest {
                    model: model.remote_model_id,
                    messages: vec![ModelMessage {
                        role: ModelRole::User,
                        content: "TEST".into(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    }],
                    images: vec![ModelImage {
                        id: "TEST-image".into(),
                        mime_type: "image/png".into(),
                        data_base64: "AA==".into(),
                    }],
                    tools: Vec::new(),
                    max_output_tokens: 100,
                    temperature: 0.0,
                    task_id: annotagent_core::TaskId::from("TEST"),
                    extra: BTreeMap::new(),
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();

        let usage = store
            .task_model_usage(&calls.project, calls.conversation, calls.task, 0, 50)
            .unwrap();
        assert_eq!(usage["summary"]["known_cost"], "0.007");
        assert_eq!(usage["attempts"]["items"][0]["image_count"], 1);
        assert_eq!(
            usage["attempts"]["items"][0]["usage_source"],
            serde_json::to_value(UsageSource::Actual).unwrap()
        );
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
        let conversation = calls.conversation;
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
            conversation,
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
