"""Stable error mapping for Worker adapters."""

from __future__ import annotations

from .models import InferenceResponse, WorkerError


def inference_error(
    request_id: str | None,
    model_id: str | None,
    code: str,
    message: str,
    *,
    retryable: bool = False,
) -> InferenceResponse:
    return InferenceResponse(
        model_identity=model_id,
        request_id=request_id,
        error=WorkerError(
            code=code,
            message=message[:1000] or "Worker error",
            retryable=retryable,
        ),
    )


def map_exception(request_id: str | None, model_id: str | None, error: Exception) -> InferenceResponse:
    if isinstance(error, (ValueError, KeyError, TypeError)):
        return inference_error(
            request_id,
            model_id,
            "invalid_request",
            f"{type(error).__name__}: {error}",
        )
    return inference_error(
        request_id,
        model_id,
        "inference_failed",
        f"{type(error).__name__}: {error}",
        retryable=True,
    )
