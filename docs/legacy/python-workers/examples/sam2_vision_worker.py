#!/usr/bin/env python3
"""Local SAM2.1 worker for AnnotAgent HTTP Vision Protocol v1."""

from __future__ import annotations

import base64
import io
import json
import os
import threading
import time
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

MAX_IMAGE_BYTES = 21_000_000
MODEL_PATH = os.environ.get("ANNOTAGENT_SAM_MODEL_PATH", "")
MODEL_CONFIG = os.environ.get(
    "ANNOTAGENT_SAM_MODEL_CONFIG", "configs/sam2.1/sam2.1_hiera_t.yaml"
)
MODEL_IDENTITY = os.environ.get("ANNOTAGENT_SAM_MODEL", "sam2.1-hiera-tiny")
MODEL_VERSION = os.environ.get("ANNOTAGENT_SAM_MODEL_VERSION", "unconfigured")
CHECKPOINT_SHA256 = os.environ.get("ANNOTAGENT_SAM_CHECKPOINT_SHA256", "") or None
TRAINING_DATASET_VERSION = os.environ.get("ANNOTAGENT_SAM_TRAINING_DATASET_VERSION", "") or None
WEIGHT_LICENSE = os.environ.get("ANNOTAGENT_SAM_WEIGHT_LICENSE", "") or None
PREDICTOR: Any = None
DEVICE = "unknown"
LOAD_ERROR: str | None = None
PREDICTOR_LOCK = threading.Lock()


def load_model() -> None:
    global PREDICTOR, DEVICE, LOAD_ERROR
    try:
        import torch
        from sam2.build_sam import build_sam2
        from sam2.sam2_image_predictor import SAM2ImagePredictor

        if not MODEL_PATH:
            raise RuntimeError("ANNOTAGENT_SAM_MODEL_PATH is required")
        checkpoint = Path(MODEL_PATH).expanduser().resolve()
        if not checkpoint.is_file():
            raise RuntimeError(f"checkpoint not found: {checkpoint}")
        if torch.cuda.is_available():
            DEVICE = "cuda"
        elif torch.backends.mps.is_available():
            DEVICE = "mps"
        else:
            DEVICE = "cpu"
        model = build_sam2(MODEL_CONFIG, str(checkpoint), device=DEVICE, apply_postprocessing=True)
        PREDICTOR = SAM2ImagePredictor(model)
    except Exception as exc:  # surfaced through health; no request/image data is logged
        LOAD_ERROR = f"{type(exc).__name__}: {exc}"[:500]


def response(error: dict[str, Any] | None = None, **extra: Any) -> dict[str, Any]:
    payload = {
        "protocol_version": 1,
        "model_identity": MODEL_IDENTITY,
        "artifacts": [],
        "request_id": None,
        "metadata": {"worker": "annotagent-sam2", "device": DEVICE},
        "usage": {},
        "warnings": [],
        "timings": {},
        "error": error,
    }
    payload.update(extra)
    return payload


def decode_image(model_image: dict[str, Any]):
    from PIL import Image
    import numpy as np

    encoded = model_image.get("data_base64", "")
    if len(encoded) > MAX_IMAGE_BYTES * 4 // 3 + 8:
        raise ValueError("inline image exceeds worker size limit")
    raw = base64.b64decode(encoded, validate=True)
    if len(raw) > MAX_IMAGE_BYTES:
        raise ValueError("decoded image exceeds worker size limit")
    with Image.open(io.BytesIO(raw)) as image:
        return np.asarray(image.convert("RGB"))


def get_box(request: dict[str, Any]) -> list[float]:
    box = request.get("parameters", {}).get("box_prompt")
    if box is None:
        for artifact in request.get("input_artifacts", []):
            value = artifact.get("value", {})
            if value.get("kind") == "bounding_box":
                box = value.get("rect")
                break
    if not isinstance(box, list) or len(box) != 4:
        raise ValueError("prompted segmentation requires normalized box_prompt [x,y,w,h]")
    values = [float(item) for item in box]
    x, y, width, height = values
    if min(values) < 0 or width <= 0 or height <= 0 or x + width > 1.00001 or y + height > 1.00001:
        raise ValueError("box_prompt must be a valid normalized rectangle")
    return values


def get_boxes(request: dict[str, Any]) -> list[list[float]]:
    pipeline_prompts = pipeline_prompt_context(request)
    if pipeline_prompts is not None:
        return [item[1] for item in pipeline_prompts[1]]
    prompts = request.get("parameters", {}).get("box_prompts")
    if prompts is None:
        return [get_box(request)]
    if not isinstance(prompts, list) or not 1 <= len(prompts) <= 8:
        raise ValueError("box_prompts must contain between one and eight boxes")
    boxes: list[list[float]] = []
    for prompt in prompts:
        if not isinstance(prompt, list) or len(prompt) != 4:
            raise ValueError("each box prompt must be normalized [x,y,w,h]")
        values = [float(item) for item in prompt]
        x, y, width, height = values
        if min(values) < 0 or width <= 0 or height <= 0 or x + width > 1.00001 or y + height > 1.00001:
            raise ValueError("box prompt must be a valid normalized rectangle")
        boxes.append(values)
    return boxes


def pipeline_prompt_context(
    request: dict[str, Any],
) -> tuple[dict[str, Any], list[tuple[str, list[float]]]] | None:
    for envelope in request.get("input_artifacts", []):
        if envelope.get("kind") != "box_prompt_set":
            continue
        artifact = envelope.get("artifact", {})
        reference = artifact.get("reference")
        prompts = artifact.get("prompts")
        if not isinstance(reference, dict) or not isinstance(prompts, list) or not prompts:
            raise ValueError("BoxPromptSet must include a set reference and at least one prompt")
        parsed: list[tuple[str, list[float]]] = []
        for prompt in prompts:
            prompt_id = prompt.get("id")
            bbox = prompt.get("bbox")
            if not isinstance(prompt_id, str) or not prompt_id or not isinstance(bbox, list) or len(bbox) != 4:
                raise ValueError("BoxPromptSet prompts require id and normalized bbox")
            values = [float(item) for item in bbox]
            x, y, width, height = values
            if min(values) < 0 or width <= 0 or height <= 0 or x + width > 1.00001 or y + height > 1.00001:
                raise ValueError("BoxPromptSet contains an invalid normalized rectangle")
            parsed.append((prompt_id, values))
        return reference, parsed
    return None


def model_availability() -> str:
    return "unknown" if PREDICTOR is not None else "missing_weights"


def model_summary() -> dict[str, Any]:
    return {
        "model_id": MODEL_IDENTITY,
        "display_name": "SAM 2 prompted segmentation",
        "architecture": "sam2.1",
        "model_version": MODEL_VERSION,
        "checkpoint_sha256": CHECKPOINT_SHA256,
        "capabilities": ["prompted_segmentation"],
        "availability": model_availability(),
    }


def model_manifest() -> dict[str, Any]:
    weights_ready = PREDICTOR is not None and CHECKPOINT_SHA256 is not None and WEIGHT_LICENSE is not None
    return {
        "schema_version": "1",
        "model_id": MODEL_IDENTITY,
        "display_name": "SAM 2 prompted segmentation",
        "architecture": "sam2.1",
        "model_version": MODEL_VERSION,
        "connection": {
            "kind": "vision_worker_model",
            "worker_id": "annotagent-sam2",
            "worker_model_id": MODEL_IDENTITY,
        },
        "capabilities": ["prompted_segmentation"],
        "input_contracts": [
            {"name": "image", "data_type": {"artifact": "image"}, "required": True, "multiple": False},
            {"name": "box_prompts", "data_type": {"artifact": "box_prompt_set"}, "required": False, "multiple": True},
            {"name": "point_prompts", "data_type": {"artifact": "point_prompt_set"}, "required": False, "multiple": True},
        ],
        "output_contracts": [
            {"name": "masks", "data_type": {"artifact": "mask_set"}, "required": True, "multiple": True},
        ],
        "prompt_contracts": [
            {"kind": "box", "required": False, "multiple": True},
            {"kind": "point", "required": False, "multiple": True},
        ],
        "score_semantics": "relative_confidence",
        "geometry_semantics": "mask_refined_geometry",
        "label_space": None,
        "checkpoint": None if CHECKPOINT_SHA256 is None else {
            "sha256": CHECKPOINT_SHA256,
            "source": MODEL_PATH or None,
            "training_dataset_version": TRAINING_DATASET_VERSION,
        },
        "runtime_requirements": {
            "devices": ["cuda", "mps", "cpu"],
            "minimum_gpu_memory_mb": None,
            "dependencies": ["sam2", "torch", "numpy", "Pillow"],
            "supports_batch": False,
        },
        "license": {
            "code_license": "Apache-2.0",
            "weight_license": WEIGHT_LICENSE,
            "source_url": "https://github.com/facebookresearch/sam2",
            "commercial_use": "unknown",
            "redistribution": "unknown",
            "usage_notes": ["Verify the concrete checkpoint license before use."],
            "verified_from_official_source": False,
        },
        "availability": "unknown" if weights_ready else "missing_weights",
        "availability_evidence": {
            "health_passed": PREDICTOR is not None,
            "protocol_compatible": True,
            "contracts_validated": True,
            "sample_conversion_passed": False,
            "weights_ready": weights_ready,
            "checked_at": datetime.now(timezone.utc).isoformat(),
            "detail": "AnnotAgent must run a selected-image sample conversion before registration.",
        },
        "metadata": {"device": DEVICE},
    }


def uncompressed_coco_rle(mask) -> str:
    # COCO uses column-major order and alternating background/foreground run lengths.
    flat = mask.astype("uint8").flatten(order="F")
    counts: list[int] = []
    previous = 0
    run = 0
    for value in flat:
        current = int(value)
        if current == previous:
            run += 1
        else:
            counts.append(run)
            run = 1
            previous = current
    counts.append(run)
    return " ".join(str(item) for item in counts)


def infer(request: dict[str, Any]) -> dict[str, Any]:
    if PREDICTOR is None:
        return response({"code": "model_unavailable", "message": LOAD_ERROR or "SAM2 is not loaded", "retryable": False})
    if request.get("protocol_version", 1) != 1:
        return response({"code": "protocol_version_mismatch", "message": "only protocol v1 is supported", "retryable": False})
    if request.get("operation") != "prompted_segmentation":
        return response({"code": "unsupported_operation", "message": "worker only supports prompted_segmentation", "retryable": False})
    started = time.perf_counter()
    try:
        import numpy as np

        image = decode_image(request["image"])
        normalized_boxes = get_boxes(request)
        height, width = image.shape[:2]
        inference_started = time.perf_counter()
        candidates: list[tuple[int, int, list[float], Any, float]] = []
        with PREDICTOR_LOCK:
            PREDICTOR.set_image(image)
            for prompt_index, normalized_box in enumerate(normalized_boxes):
                x, y, box_width, box_height = normalized_box
                pixel_box = np.asarray(
                    [x * width, y * height, (x + box_width) * width, (y + box_height) * height],
                    dtype=np.float32,
                )
                masks, scores, _ = PREDICTOR.predict(box=pixel_box, multimask_output=True)
                for mask_index, (mask, score) in enumerate(zip(masks, scores, strict=True)):
                    if mask.any():
                        candidates.append(
                            (prompt_index, mask_index, normalized_box, mask.astype(bool), float(score))
                        )
        inference_ms = int((time.perf_counter() - inference_started) * 1000)
        if not candidates:
            raise ValueError("SAM2 returned an empty mask")
        pipeline_prompts = pipeline_prompt_context(request)
        if pipeline_prompts is not None:
            source_prompts, prompt_items = pipeline_prompts
            masks = []
            for prompt_index, mask_index, normalized_box, mask, score in candidates:
                prompt_id = prompt_items[prompt_index][0]
                masks.append({
                    "mask_id": f"sam-mask:{prompt_id}:{mask_index}",
                    "prompt": {**source_prompts, "item_id": prompt_id},
                    "mask": {
                        "encoding": "coco_rle",
                        "width": width,
                        "height": height,
                        "counts": uncompressed_coco_rle(mask),
                    },
                    "score": {"value": score, "semantics": "relative_confidence"},
                    "attributes": {
                        "prompt_bbox": normalized_box,
                        "prompt_index": prompt_index,
                        "mask_index": mask_index,
                        "mask_area_pixels": int(mask.sum()),
                        "device": DEVICE,
                    },
                })
            artifacts = [{
                "kind": "mask_set",
                "artifact": {
                    "reference": {
                        "artifact_id": f"sam-mask-set:{uuid.uuid4()}",
                        "source_node": request["node_id"],
                        "port": "masks",
                        "artifact_type": "mask_set",
                        "item_id": None,
                    },
                    "image_id": request["image_id"],
                    "model_binding": request["model_id"],
                    "source_prompts": source_prompts,
                    "validation_state": "unvalidated",
                    "masks": masks,
                    "metadata": {"worker": "annotagent-sam2", "device": DEVICE},
                },
            }]
        else:
            input_ids = [item.get("id") for item in request.get("input_artifacts", []) if item.get("id")]
            artifacts = []
            for prompt_index, mask_index, normalized_box, mask, score in candidates:
                ys, xs = np.where(mask)
                tight = [
                    float(xs.min() / width),
                    float(ys.min() / height),
                    float((xs.max() - xs.min() + 1) / width),
                    float((ys.max() - ys.min() + 1) / height),
                ]
                artifacts.append({
                    "id": str(uuid.uuid4()),
                    "image_id": request["image_id"],
                    "task_id": request.get("task_id"),
                    "label": "ball",
                    "role": "candidate",
                    "value": {
                        "kind": "instance_mask",
                        "mask": {
                            "encoding": "coco_rle",
                            "width": width,
                            "height": height,
                            "counts": uncompressed_coco_rle(mask),
                        },
                    },
                    "source_node": request["node_id"],
                    "confidence": score,
                    "metadata": {
                        "tight_bbox": tight,
                        "prompt_bbox": normalized_box,
                        "prompt_index": prompt_index,
                        "mask_index": mask_index,
                        "mask_area_pixels": int(mask.sum()),
                        "device": DEVICE,
                    },
                    "validation_state": "unvalidated",
                    "provenance": {
                        "provider": "sam2_http_worker",
                        "model": MODEL_IDENTITY,
                        "request_id": request.get("request_id"),
                        "input_artifact_ids": input_ids,
                    },
                    "revision": 1,
                    "replaces_artifact_id": None,
                    "created_at": datetime.now(timezone.utc).isoformat(),
                })
        total_ms = int((time.perf_counter() - started) * 1000)
        return response(
            artifacts=artifacts,
            request_id=request.get("request_id"),
            timings={"inference_ms": inference_ms, "total_ms": total_ms},
            usage={"source": DEVICE, "compute_milliseconds": inference_ms},
        )
    except Exception as exc:
        return response({"code": "inference_failed", "message": f"{type(exc).__name__}: {exc}"[:500], "retryable": False})


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: Any) -> None:
        print(f"sam2-worker {self.address_string()} {format % args}")

    def send_json(self, status: int, payload: dict[str, Any]) -> None:
        encoded = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self) -> None:
        if self.path == "/health":
            status = "healthy" if PREDICTOR is not None else "unavailable"
            self.send_json(200 if PREDICTOR is not None else 503, {"status": status, "detail": LOAD_ERROR, "checked_at": datetime.now(timezone.utc).isoformat()})
        elif self.path == "/v1/capabilities":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": "annotagent-sam2",
                "model_identity": MODEL_IDENTITY,
                "capabilities": ["prompted_segmentation"],
                "input_types": ["image", {"artifact": "box_prompt_set"}, {"artifact": "point_prompt_set"}],
                "output_types": ["mask_set"],
                "limits": {"max_images": 1, "max_input_artifacts": 8, "max_request_bytes": MAX_IMAGE_BYTES, "timeout_seconds": 120},
                "models": [model_summary()],
            })
        elif self.path == "/v1/models":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": "annotagent-sam2",
                "models": [model_summary()],
            })
        elif self.path == "/v1/contracts":
            self.send_json(200, {
                "protocol_version": 1,
                "worker_id": "annotagent-sam2",
                "models": [model_manifest()],
            })
        else:
            self.send_json(404, {"error": "not_found"})

    def do_POST(self) -> None:
        if self.path != "/v1/infer":
            self.send_json(404, {"error": "not_found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > MAX_IMAGE_BYTES * 2:
                raise ValueError("request body has invalid size")
            request = json.loads(self.rfile.read(length))
            result = infer(request)
            self.send_json(200 if result.get("error") is None else 422, result)
        except Exception as exc:
            self.send_json(400, response({"code": "invalid_request", "message": f"{type(exc).__name__}: {exc}"[:500], "retryable": False}))


if __name__ == "__main__":
    load_model()
    host = os.environ.get("ANNOTAGENT_SAM_HOST", "127.0.0.1")
    port = int(os.environ.get("ANNOTAGENT_SAM_PORT", "8790"))
    print(f"SAM2 worker: http://{host}:{port} model={MODEL_IDENTITY} device={DEVICE}")
    if LOAD_ERROR:
        print(f"SAM2 load error: {LOAD_ERROR}")
    ThreadingHTTPServer((host, port), Handler).serve_forever()
