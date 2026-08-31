"""FastAPI endpoint helpers for a versioned expert vision Worker."""

from __future__ import annotations

import inspect
import time
from collections.abc import Awaitable, Callable, Sequence
from datetime import datetime, timezone
from threading import Event

from fastapi import FastAPI, HTTPException

from .cancellation import CancellationRegistry
from .errors import map_exception
from .image import decode_image_bytes
from .models import (
    ArtifactKind,
    CancelRequest,
    CancelResponse,
    Capability,
    CapabilityResponse,
    ContractsResponse,
    ExpertModelManifest,
    HealthResponse,
    InferenceRequest,
    InferenceResponse,
    ModelAvailability,
    ModelsResponse,
    WarmupRequest,
    WarmupResponse,
    WorkerError,
    WorkerModelSummary,
)

InferenceHandler = Callable[
    [InferenceRequest, Event], InferenceResponse | Awaitable[InferenceResponse]
]
HealthHandler = Callable[[], HealthResponse | Awaitable[HealthResponse]]
WarmupHandler = Callable[[str], bool | Awaitable[bool]]


def create_worker_app(
    manifests: Sequence[ExpertModelManifest],
    infer: InferenceHandler,
    *,
    health: HealthHandler | None = None,
    warmup: WarmupHandler | None = None,
    max_image_bytes: int = 32_000_000,
) -> FastAPI:
    if not manifests:
        raise ValueError("at least one Expert Model Manifest is required")
    manifest_by_id = {manifest.model_id: manifest for manifest in manifests}
    if len(manifest_by_id) != len(manifests):
        raise ValueError("Expert Model Manifest identities must be unique")
    worker_ids = {
        manifest.connection.worker_id
        for manifest in manifests
        if manifest.connection.kind == "vision_worker_model"
    }
    if len(worker_ids) != 1 or any(
        manifest.connection.kind != "vision_worker_model" for manifest in manifests
    ):
        raise ValueError("Worker apps require manifests for exactly one Vision Worker identity")
    worker_id = next(iter(worker_ids))
    cancellations = CancellationRegistry()
    app = FastAPI(title="AnnotAgent Vision Worker", version="1")

    @app.get("/health", response_model=HealthResponse)
    async def get_health() -> HealthResponse:
        if health is not None:
            result = health()
            return await result if inspect.isawaitable(result) else result
        available = any(
            manifest.availability == ModelAvailability.AVAILABLE for manifest in manifests
        )
        missing_weights = all(
            manifest.availability == ModelAvailability.MISSING_WEIGHTS for manifest in manifests
        )
        status = "healthy" if available else ("unavailable" if missing_weights else "degraded")
        return HealthResponse(
            status=status,
            detail="manifest-derived status; run an explicit AnnotAgent sample test before registration",
            checked_at=datetime.now(timezone.utc),
        )

    @app.get("/v1/capabilities", response_model=CapabilityResponse)
    async def get_capabilities() -> CapabilityResponse:
        primary = manifests[0]
        input_types = [contract.data_type.root for contract in primary.input_contracts]
        output_types = [
            contract.data_type.root["artifact"]
            for contract in primary.output_contracts
            if isinstance(contract.data_type.root, dict)
        ]
        return CapabilityResponse(
            worker_id=worker_id,
            model_identity=primary.model_id,
            capabilities=[_wire_capability(capability) for capability in primary.capabilities],
            input_types=input_types,
            output_types=output_types,
            models=[_summary(manifest) for manifest in manifests],
        )

    @app.get("/v1/models", response_model=ModelsResponse)
    async def get_models() -> ModelsResponse:
        return ModelsResponse(worker_id=worker_id, models=[_summary(item) for item in manifests])

    @app.get("/v1/contracts", response_model=ContractsResponse)
    async def get_contracts() -> ContractsResponse:
        return ContractsResponse(worker_id=worker_id, models=list(manifests))

    @app.post("/v1/infer", response_model=InferenceResponse)
    async def post_infer(request: InferenceRequest) -> InferenceResponse:
        manifest = manifest_by_id.get(request.model_id)
        if manifest is None:
            raise HTTPException(status_code=404, detail="unknown model_id")
        if request.operation not in {
            _wire_capability(capability) for capability in manifest.capabilities
        }:
            raise HTTPException(status_code=422, detail="operation is not declared by the model")
        if request.image is not None:
            try:
                decode_image_bytes(request.image, max_bytes=max_image_bytes)
            except ValueError as error:
                raise HTTPException(status_code=422, detail="invalid bounded image input") from error
        cancellation = cancellations.begin(request.request_id)
        if request.cancellation_requested:
            cancellation.set()
        try:
            result = infer(request, cancellation)
            response = await result if inspect.isawaitable(result) else result
            if response.request_id not in (None, request.request_id):
                raise ValueError("inference response request_id mismatch")
            if response.model_identity not in (None, request.model_id):
                raise ValueError("inference response model identity mismatch")
            response.request_id = request.request_id
            response.model_identity = request.model_id
            return response
        except Exception as error:  # converted without request/image/header logging
            return map_exception(request.request_id, request.model_id, error)
        finally:
            cancellations.finish(request.request_id)

    @app.post("/v1/cancel", response_model=CancelResponse)
    async def post_cancel(request: CancelRequest) -> CancelResponse:
        if request.model_id not in manifest_by_id:
            raise HTTPException(status_code=404, detail="unknown model_id")
        return CancelResponse(
            request_id=request.request_id,
            cancelled=cancellations.cancel(request.request_id),
        )

    @app.post("/v1/warmup", response_model=WarmupResponse)
    async def post_warmup(request: WarmupRequest) -> WarmupResponse:
        if request.model_id not in manifest_by_id:
            raise HTTPException(status_code=404, detail="unknown model_id")
        if warmup is None:
            return WarmupResponse(
                request_id=request.request_id,
                model_id=request.model_id,
                ready=False,
                error=WorkerError(
                    code="warmup_unsupported",
                    message="this Worker does not implement optional warmup",
                    retryable=False,
                ),
            )
        started = time.perf_counter()
        result = warmup(request.model_id)
        ready = await result if inspect.isawaitable(result) else result
        return WarmupResponse(
            request_id=request.request_id,
            model_id=request.model_id,
            ready=ready,
            duration_ms=int((time.perf_counter() - started) * 1000),
        )

    return app


def _summary(manifest: ExpertModelManifest) -> WorkerModelSummary:
    return WorkerModelSummary(
        model_id=manifest.model_id,
        display_name=manifest.display_name,
        architecture=manifest.architecture,
        model_version=manifest.model_version,
        checkpoint_sha256=manifest.checkpoint.sha256 if manifest.checkpoint else None,
        capabilities=[_wire_capability(capability) for capability in manifest.capabilities],
        availability=manifest.availability,
    )


def _wire_capability(capability: Capability) -> str:
    if capability == Capability.IMAGE_CLASSIFICATION:
        return "classification"
    return capability.value
