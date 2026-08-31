from __future__ import annotations

import base64
from threading import Event
from uuid import uuid4

from fastapi.testclient import TestClient

from annotagent_vision_worker import (
    InferenceRequest,
    InferenceResponse,
    assert_app_conformance,
    create_worker_app,
)
from annotagent_vision_worker.models import ExpertModelManifest

from test_models import manifest_value

PNG_1X1 = base64.b64encode(
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
    b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\rIDAT\x08\xd7c\xf8\xcf\xc0"
    b"\xf0\x1f\x00\x05\x00\x01\xff\x89\x99=\x1d\x00\x00\x00\x00IEND\xaeB`\x82"
).decode()


def infer(request: InferenceRequest, cancellation: Event) -> InferenceResponse:
    assert not cancellation.is_set()
    return InferenceResponse(request_id=request.request_id, model_identity=request.model_id)


def test_discovery_and_conformance_are_consistent() -> None:
    manifest = ExpertModelManifest.model_validate(manifest_value())
    app = create_worker_app([manifest], infer)
    assert_app_conformance(app)
    client = TestClient(app)
    assert client.get("/health").json()["status"] == "degraded"
    assert client.get("/v1/models").json()["models"][0]["availability"] == "unknown"
    assert client.get("/v1/contracts").json()["models"][0]["model_id"] == "test-segmenter"


def test_inference_rejects_bad_image_and_preserves_scope() -> None:
    manifest = ExpertModelManifest.model_validate(manifest_value())
    client = TestClient(create_worker_app([manifest], infer))
    request = {
        "protocol_version": 1,
        "request_id": "request-1",
        "operation": "prompted_segmentation",
        "run_id": str(uuid4()),
        "image_id": str(uuid4()),
        "task_id": "objects",
        "node_id": "segment",
        "model_id": "test-segmenter",
        "image": {"id": "image", "mime_type": "image/png", "data_base64": PNG_1X1},
        "input_artifacts": [],
        "prompt": None,
        "parameters": {},
        "timeout_ms": 1000,
        "cancellation_requested": False,
    }
    response = client.post("/v1/infer", json=request)
    assert response.status_code == 200
    assert response.json()["request_id"] == "request-1"
    assert response.json()["model_identity"] == "test-segmenter"
    request["image"]["data_base64"] = "not-base64"
    assert client.post("/v1/infer", json=request).status_code == 422
