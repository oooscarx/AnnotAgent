//! A shared sandbox-call allowance around existing model adapters, not a new executor.
use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelRequest, ModelResponse, VisionBackendKind,
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
}
impl SampleCalls {
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
        self.calls.reserve()?;
        self.inner.complete(request, cancellation).await
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
        self.calls.reserve()?;
        self.inner.infer(request, cancellation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
