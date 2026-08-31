#!/usr/bin/env python3
"""Deterministic multi-model Vision Protocol fixture for browser release tests.

This process proves discovery and typed conversion only. It is deliberately named as a fixture,
contains no model code or weights, and must never be presented as real model quality evidence.
"""

from __future__ import annotations

import json
import os
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

WORKER_ID = "annotagent-e2e-contract-fixture"
PORT = int(os.environ.get("ANNOTAGENT_E2E_WORKER_PORT", "8796"))
CHECKPOINTS = {
    "sam2.1-hiera-tiny": "a" * 64,
    "yolo-specialist": "b" * 64,
    "rfdetr-specialist": "c" * 64,
    "locate-anything": "d" * 64,
    "e2e-generic-detector": "e" * 64,
}
CAPABILITIES = {
    "sam2.1-hiera-tiny": ["prompted_segmentation"],
    "yolo-specialist": ["object_detection"],
    "rfdetr-specialist": ["object_detection"],
    "locate-anything": ["open_vocabulary_detection", "phrase_grounding"],
    "e2e-generic-detector": ["object_detection"],
}


def checked_at() -> str:
    return datetime.now(timezone.utc).isoformat()


def model_summary(model_id: str) -> dict[str, Any]:
    return {
        "model_id": model_id,
        "display_name": f"E2E contract fixture · {model_id}",
        "architecture": "deterministic-contract-fixture",
        "model_version": "e2e-contract-v1",
        "checkpoint_sha256": CHECKPOINTS[model_id],
        "capabilities": CAPABILITIES[model_id],
        "availability": "unknown",
    }


def artifact_contract(name: str, kind: str, required: bool = True) -> dict[str, Any]:
    return {
        "name": name,
        "data_type": {"artifact": kind},
        "required": required,
        "multiple": False,
    }


def model_manifest(model_id: str) -> dict[str, Any]:
    segmentation = model_id == "sam2.1-hiera-tiny"
    capabilities = CAPABILITIES[model_id]
    inputs = [artifact_contract("image", "image")]
    prompts: list[dict[str, Any]] = []
    if segmentation:
        inputs.append(artifact_contract("box_prompts", "box_prompt_set"))
        prompts = [{"kind": "box", "required": True, "multiple": True}]
    return {
        "schema_version": "1",
        "model_id": model_id,
        "display_name": f"E2E contract fixture · {model_id}",
        "architecture": "deterministic-contract-fixture",
        "model_version": "e2e-contract-v1",
        "connection": {
            "kind": "vision_worker_model",
            "worker_id": WORKER_ID,
            "worker_model_id": model_id,
        },
        "capabilities": capabilities,
        "input_contracts": inputs,
        "output_contracts": [
            artifact_contract("masks" if segmentation else "detections", "mask_set" if segmentation else "detection_set")
        ],
        "prompt_contracts": prompts,
        "score_semantics": "relative_confidence",
        "geometry_semantics": "mask_refined_geometry" if segmentation else "predicted_geometry",
        "label_space": None if segmentation or model_id == "locate-anything" else ["football"],
        "checkpoint": {
            "sha256": CHECKPOINTS[model_id],
            "source": "deterministic browser-test fixture",
            "training_dataset_version": "synthetic-e2e-v1",
        },
        "runtime_requirements": {
            "devices": ["cpu"],
            "minimum_gpu_memory_mb": None,
            "dependencies": [],
            "supports_batch": False,
        },
        "license": {
            "code_license": "test-only",
            "weight_license": "test-only deterministic fixture",
            "source_url": None,
            "commercial_use": "restricted",
            "redistribution": "restricted",
            "usage_notes": ["Protocol fixture only; not real model accuracy."],
            "verified_from_official_source": False,
        },
        "availability": "unknown",
        "availability_evidence": {
            "health_passed": True,
            "protocol_compatible": True,
            "contracts_validated": True,
            "sample_conversion_passed": False,
            "weights_ready": True,
            "checked_at": checked_at(),
            "detail": "Browser fixture requires AnnotAgent selected-image conversion.",
        },
        "metadata": {"fixture": True, "real_model": False},
    }


def mask_response(request: dict[str, Any]) -> dict[str, Any]:
    prompt_set = next(
        envelope["artifact"]
        for envelope in request.get("input_artifacts", [])
        if envelope.get("kind") == "box_prompt_set"
    )
    prompt = prompt_set["prompts"][0]
    source_prompts = prompt_set["reference"]
    mask_set = {
        "reference": {
            "artifact_id": f"e2e-mask-set:{uuid.uuid4()}",
            "source_node": request["node_id"],
            "port": "masks",
            "artifact_type": "mask_set",
            "item_id": None,
        },
        "image_id": request["image_id"],
        "model_binding": request["model_id"],
        "source_prompts": source_prompts,
        "validation_state": "unvalidated",
        "masks": [
            {
                "mask_id": "e2e-mask",
                "prompt": {**source_prompts, "item_id": prompt["id"]},
                "mask": {
                    "encoding": "coco_rle",
                    "width": 4,
                    "height": 4,
                    "counts": "5 2 2 2 5",
                },
                "score": {"value": 0.9, "semantics": "relative_confidence"},
                "attributes": {"fixture": True},
            }
        ],
        "metadata": {"fixture": True, "real_model": False},
    }
    return {
        "protocol_version": 1,
        "request_id": request["request_id"],
        "model_identity": request["model_id"],
        "artifacts": [{"kind": "mask_set", "artifact": mask_set}],
        "metadata": {"fixture": True},
        "usage": {"source": "deterministic_fixture", "compute_milliseconds": 1},
        "timings": {"total_ms": 1},
        "warnings": ["Deterministic contract fixture; no real model inference."],
        "error": None,
    }


def detection_response(request: dict[str, Any]) -> dict[str, Any]:
    artifact = {
        "id": str(uuid.uuid4()),
        "image_id": request["image_id"],
        "task_id": request["task_id"],
        "label": "football",
        "role": "candidate",
        "value": {"kind": "bounding_box", "rect": [0.35, 0.35, 0.2, 0.2]},
        "source_node": request["node_id"],
        "confidence": 0.9,
        "metadata": {"fixture": True, "real_model": False},
        "validation_state": "unvalidated",
        "provenance": {
            "provider": "e2e_contract_fixture",
            "model": request["model_id"],
            "tool": None,
            "request_id": request["request_id"],
            "model_digest": CHECKPOINTS[request["model_id"]],
            "input_artifact_ids": [],
        },
        "revision": 1,
        "replaces_artifact_id": None,
        "created_at": checked_at(),
    }
    return {
        "protocol_version": 1,
        "model_identity": request["model_id"],
        "artifacts": [artifact],
        "request_id": request["request_id"],
        "metadata": {"fixture": True},
        "usage": {"source": "deterministic_fixture", "compute_milliseconds": 1},
        "warnings": ["Deterministic contract fixture; no real model inference."],
        "timings": {"total_ms": 1},
        "error": None,
    }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: Any) -> None:
        return

    def send_json(self, status: int, payload: dict[str, Any]) -> None:
        encoded = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/health":
            self.send_json(200, {"status": "healthy", "detail": "deterministic contract fixture", "checked_at": checked_at()})
        elif self.path == "/v1/capabilities":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": WORKER_ID,
                "model_identity": "e2e-multi-model-contract-fixture",
                "capabilities": sorted({item for values in CAPABILITIES.values() for item in values}),
                "input_types": ["image", {"artifact": "box_prompt_set"}],
                "output_types": ["bounding_box", "detection_set", "mask_set"],
                "limits": {"max_images": 1, "max_input_artifacts": 8, "max_request_bytes": 20_000_000, "timeout_seconds": 10},
                "models": [model_summary(model_id) for model_id in CAPABILITIES],
            })
        elif self.path == "/v1/models":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": WORKER_ID,
                "models": [model_summary(model_id) for model_id in CAPABILITIES],
            })
        elif self.path == "/v1/contracts":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": WORKER_ID,
                "models": [model_manifest(model_id) for model_id in CAPABILITIES],
            })
        else:
            self.send_json(404, {"error": "not_found"})

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/v1/infer":
            self.send_json(404, {"error": "not_found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            request = json.loads(self.rfile.read(length))
            response = mask_response(request) if request.get("operation") == "prompted_segmentation" else detection_response(request)
            self.send_json(200, response)
        except Exception as error:
            self.send_json(400, {"error": f"invalid fixture request: {type(error).__name__}: {error}"})


if __name__ == "__main__":
    print(f"AnnotAgent deterministic Expert Vision fixture: http://127.0.0.1:{PORT}")
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
