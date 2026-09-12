//! A shared sandbox-call allowance around existing model adapters, not a new executor.
use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelRequest, ModelResponse,
    PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend, VisionBackendKind,
    VisionCapability, VisionInferenceRequest, VisionInferenceResponse, VisionModelBackend,
    VisionModelProvider,
};
use async_trait::async_trait;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct SampleCalls(Arc<CallAllowance>);
enum CallAllowance {
    Sample(AtomicU64),
    Batch(
        Arc<annotagent_storage::SqliteStore>,
        annotagent_core::BatchId,
    ),
    Conversation(crate::conversation_vision_calls::ConversationVisionCalls),
    BoundedConversation {
        local: SampleCalls,
        calls: crate::conversation_vision_calls::ConversationVisionCalls,
    },
}
impl SampleCalls {
    pub(crate) fn bounded_conversation(limit: u64, calls: crate::ConversationVisionCalls) -> Self {
        Self(Arc::new(CallAllowance::BoundedConversation {
            local: Self::new(limit),
            calls,
        }))
    }
    pub(crate) fn conversation(
        calls: crate::conversation_vision_calls::ConversationVisionCalls,
    ) -> Self {
        Self(Arc::new(CallAllowance::Conversation(calls)))
    }
    fn begin(
        &self,
        request: &impl serde::Serialize,
        cancellation: &CancellationToken,
    ) -> CoreResult<Option<crate::conversation_vision_calls::VisionCallReceipt>> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Validation(
                "Sample cancelled before model admission".into(),
            ));
        }
        match self.0.as_ref() {
            CallAllowance::Conversation(calls) => calls.begin(request).map(Some),
            CallAllowance::BoundedConversation { local, calls } => {
                local.reserve()?;
                calls.begin(request).map(Some)
            }
            _ => {
                self.reserve()?;
                Ok(None)
            }
        }
    }
    pub(crate) fn pipeline(
        &self,
        inner: Arc<dyn PipelineModelBackend>,
    ) -> Arc<dyn PipelineModelBackend> {
        Arc::new(LimitedPipeline {
            inner,
            calls: self.clone(),
        })
    }
    pub(crate) fn new(limit: u64) -> Self {
        Self(Arc::new(CallAllowance::Sample(AtomicU64::new(limit))))
    }
    pub(crate) fn batch(
        store: Arc<annotagent_storage::SqliteStore>,
        id: annotagent_core::BatchId,
    ) -> Self {
        Self(Arc::new(CallAllowance::Batch(store, id)))
    }
    fn reserve(&self) -> CoreResult<()> {
        let remaining = match self.0.as_ref() {
            CallAllowance::Conversation(_) | CallAllowance::BoundedConversation { .. } => {
                return Err(CoreError::Validation(
                    "Conversation calls require a frozen request receipt".into(),
                ));
            }
            CallAllowance::Batch(store, id) => {
                return store
                    .reserve_batch_model_call(*id)
                    .map_err(|error| CoreError::Validation(error.to_string()));
            }
            CallAllowance::Sample(remaining) => remaining,
        };
        remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .map(|_| ())
            .map_err(|_| {
                CoreError::Validation(
                    "Sample model-call allowance exhausted; no further model request was sent"
                        .to_owned(),
                )
            })
    }
    pub(crate) fn provider(
        &self,
        inner: Arc<dyn VisionModelProvider>,
    ) -> Arc<dyn VisionModelProvider> {
        Arc::new(LimitedProvider {
            inner,
            calls: self.clone(),
        })
    }
    pub(crate) fn backend(
        &self,
        inner: Arc<dyn VisionModelBackend>,
    ) -> Arc<dyn VisionModelBackend> {
        Arc::new(LimitedBackend {
            inner,
            calls: self.clone(),
        })
    }
}
struct LimitedPipeline {
    inner: Arc<dyn PipelineModelBackend>,
    calls: SampleCalls,
}
#[async_trait]
impl PipelineModelBackend for LimitedPipeline {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn capability(&self) -> VisionCapability {
        self.inner.capability()
    }
    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        let receipt = self.calls.begin(&request, &cancellation)?;
        let result = self.inner.infer_pipeline(request, cancellation).await;
        if let Some(receipt) = receipt {
            receipt.finish(result.is_ok())?;
        }
        result
    }
}
struct LimitedProvider {
    inner: Arc<dyn VisionModelProvider>,
    calls: SampleCalls,
}
#[async_trait]
impl VisionModelProvider for LimitedProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn capabilities(&self) -> ModelCapabilities {
        self.inner.capabilities()
    }
    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        let receipt = self.calls.begin(&request, &cancellation)?;
        let result = if let Some(receipt) = receipt.as_ref() {
            annotagent_provider::within_model_call(
                receipt.id().to_string(),
                self.inner.complete(request, cancellation),
            )
            .await
        } else {
            self.inner.complete(request, cancellation).await
        };
        if let Some(receipt) = receipt {
            receipt.finish(result.is_ok())?;
        }
        result
    }
}
struct LimitedBackend {
    inner: Arc<dyn VisionModelBackend>,
    calls: SampleCalls,
}
#[async_trait]
impl VisionModelBackend for LimitedBackend {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> VisionBackendKind {
        self.inner.kind()
    }
    fn capabilities(&self) -> Vec<VisionCapability> {
        self.inner.capabilities()
    }
    async fn infer(
        &self,
        request: VisionInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<VisionInferenceResponse> {
        let receipt = self.calls.begin(&request, &cancellation)?;
        let result = self.inner.infer(request, cancellation).await;
        if let Some(receipt) = receipt {
            receipt.finish(result.is_ok())?;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct CountingPipeline(Arc<AtomicU64>);
    #[async_trait]
    impl PipelineModelBackend for CountingPipeline {
        fn id(&self) -> &str {
            "offline-call-counter"
        }
        fn capability(&self) -> VisionCapability {
            VisionCapability::Classification
        }
        async fn infer_pipeline(
            &self,
            _request: PipelineInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<PipelineInferenceResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(PipelineInferenceResponse::default())
        }
    }
    fn pipeline_request() -> PipelineInferenceRequest {
        PipelineInferenceRequest {
            protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: "bounded-call-test".into(),
            run_id: annotagent_core::RunId::new(),
            image_id: annotagent_core::ImageId::new(),
            node_id: "classify".into(),
            model_id: "offline-call-counter".into(),
            operation: VisionCapability::Classification,
            image: None,
            input_artifacts: Vec::new(),
            parameters: std::collections::BTreeMap::new(),
            timeout_ms: None,
        }
    }
    #[tokio::test]
    async fn native_pipeline_calls_share_the_allowance_and_fail_before_inference() {
        let calls = SampleCalls::new(3);
        let invoked = Arc::new(AtomicU64::new(0));
        let first = calls.pipeline(Arc::new(CountingPipeline(invoked.clone())));
        let second = calls.pipeline(Arc::new(CountingPipeline(invoked.clone())));
        assert_eq!(first.id(), "offline-call-counter");
        assert_eq!(first.capability(), VisionCapability::Classification);
        // One reservation by another adapter leaves only two native calls.
        calls.reserve().unwrap();
        let mut handles = Vec::new();
        for index in 0..16 {
            let backend = if index % 2 == 0 {
                first.clone()
            } else {
                second.clone()
            };
            handles.push(tokio::spawn(async move {
                backend
                    .infer_pipeline(pipeline_request(), CancellationToken::new())
                    .await
                    .is_ok()
            }));
        }
        let mut successes = 0;
        for handle in handles {
            successes += usize::from(handle.await.unwrap());
        }
        assert_eq!(successes, 2);
        assert_eq!(invoked.load(Ordering::SeqCst), 2);
        assert!(calls.reserve().is_err());
        let batch = SampleCalls::batch(
            Arc::new(annotagent_storage::SqliteStore::open_in_memory().unwrap()),
            annotagent_core::BatchId::new(),
        );
        let backend = batch.pipeline(Arc::new(CountingPipeline(invoked.clone())));
        assert!(
            backend
                .infer_pipeline(pipeline_request(), CancellationToken::new())
                .await
                .is_err()
        );
        assert_eq!(
            invoked.load(Ordering::SeqCst),
            2,
            "missing durable allowance must fail closed"
        );
    }
    #[test]
    fn allowance_is_shared_across_images_models_and_parallel_calls() {
        let calls = SampleCalls::new(12);
        let successes = std::thread::scope(|scope| {
            let handles = (0..32)
                .map(|_| {
                    let calls = calls.clone();
                    scope.spawn(move || calls.reserve().is_ok())
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| usize::from(handle.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(successes, 12);
        assert!(calls.reserve().is_err());
    }
}
