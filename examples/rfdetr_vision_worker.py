#!/usr/bin/env python3
"""RF-DETR adapter for AnnotAgent Detection Worker Protocol v1.

The adapter performs no installation and no model download. Real inference requires an explicitly
configured local checkpoint plus immutable version metadata:

  ANNOTAGENT_RFDETR_CHECKPOINT_PATH=/absolute/checkpoint.pth
  ANNOTAGENT_RFDETR_CHECKPOINT_SHA256=<64 lowercase or uppercase hex characters>
  ANNOTAGENT_RFDETR_ARCHITECTURE=rfdetr-small
  ANNOTAGENT_RFDETR_MODEL_VERSION=1
  ANNOTAGENT_RFDETR_TRAINING_DATASET_VERSION=my-dataset-v1
  ANNOTAGENT_RFDETR_LABEL_SPACE='["football","robot"]'

Without a complete configuration, the process remains available for health/capability discovery
and reports unavailable model health. Requests, image bytes, headers, predictions, and local paths
are never logged.
"""

from __future__ import annotations

import base64
import hashlib
import io
import json
import math
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

PROTOCOL_VERSION = 1
WORKER_ID = "annotagent-rfdetr"
MODEL_ID = os.environ.get("ANNOTAGENT_RFDETR_MODEL_ID", "rfdetr-specialist-local")
CHECKPOINT_PATH = os.environ.get("ANNOTAGENT_RFDETR_CHECKPOINT_PATH", "")
CHECKPOINT_SHA256 = os.environ.get("ANNOTAGENT_RFDETR_CHECKPOINT_SHA256", "")
ARCHITECTURE = os.environ.get("ANNOTAGENT_RFDETR_ARCHITECTURE", "")
MODEL_VERSION = os.environ.get("ANNOTAGENT_RFDETR_MODEL_VERSION", "")
TRAINING_DATASET_VERSION = os.environ.get("ANNOTAGENT_RFDETR_TRAINING_DATASET_VERSION", "")
LABEL_SPACE_JSON = os.environ.get("ANNOTAGENT_RFDETR_LABEL_SPACE", "[]")
HOST = os.environ.get("ANNOTAGENT_RFDETR_HOST", "127.0.0.1")
PORT = int(os.environ.get("ANNOTAGENT_RFDETR_PORT", "8792"))
MAX_IMAGE_BYTES = 32_000_000
MAX_REQUEST_BYTES = 44_000_000
MAX_RESPONSE_BYTES = 4_000_000
MAX_DETECTIONS = 10_000

MODEL: Any | None = None
MODEL_ERROR: str | None = None
DEVICE = "unknown"
MODEL_LOCK = threading.Lock()
ACTIVE_LOCK = threading.Lock()
ACTIVE_REQUESTS: set[str] = set()
CANCELLED_REQUESTS: set[str] = set()


def _safe_exception(error: Exception) -> str:
    return type(error).__name__[:100]


def _configured_label_space() -> list[str]:
    try:
        parsed = json.loads(LABEL_SPACE_JSON)
    except json.JSONDecodeError:
        return []
    if (
        not isinstance(parsed, list)
        or len(parsed) > 10_000
        or any(not isinstance(label, str) or not label.strip() for label in parsed)
        or len(set(parsed)) != len(parsed)
    ):
        return []
    return parsed


LABEL_SPACE = _configured_label_space()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as checkpoint:
        for chunk in iter(lambda: checkpoint.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_model() -> None:
    global MODEL, MODEL_ERROR, DEVICE
    if not all(
        [
            CHECKPOINT_PATH,
            CHECKPOINT_SHA256,
            ARCHITECTURE,
            MODEL_VERSION,
            TRAINING_DATASET_VERSION,
            LABEL_SPACE,
        ]
    ):
        MODEL_ERROR = "checkpoint_and_immutable_metadata_required"
        return
    if len(CHECKPOINT_SHA256) != 64 or any(character not in "0123456789abcdefABCDEF" for character in CHECKPOINT_SHA256):
        MODEL_ERROR = "invalid_checkpoint_sha256"
        return
    try:
        checkpoint = Path(CHECKPOINT_PATH).expanduser().resolve(strict=True)
        if not checkpoint.is_file() or _sha256_file(checkpoint) != CHECKPOINT_SHA256.lower():
            raise RuntimeError("checkpoint identity mismatch")
        import torch  # type: ignore[import-not-found]
        from rfdetr import from_checkpoint  # type: ignore[import-not-found]

        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is required by the tracked RF-DETR profile")
        DEVICE = "cuda"
        # The full existing path prevents rfdetr's loader from downloading a named public model.
        # Safe checkpoint loading remains enabled; unsafe pickle fallback is never requested.
        MODEL = from_checkpoint(str(checkpoint), trust_checkpoint=False, device=DEVICE)
    except Exception as error:
        MODEL = None
        MODEL_ERROR = f"model_load_failed:{_safe_exception(error)}"


def _ensure_fields(value: dict[str, Any], allowed: set[str], scope: str) -> None:
    if set(value) - allowed:
        raise ValueError(f"{scope} contains unsupported fields")


def _decode_image(value: Any):
    if not isinstance(value, dict):
        raise ValueError("image must be an object")
    _ensure_fields(value, {"id", "mime_type", "data_base64"}, "image")
    if not isinstance(value.get("id"), str) or not value["id"]:
        raise ValueError("image id is required")
    if value.get("mime_type") not in {"image/png", "image/jpeg"}:
        raise ValueError("image must be PNG or JPEG")
    encoded = value.get("data_base64")
    if not isinstance(encoded, str) or len(encoded) > MAX_IMAGE_BYTES * 4 // 3 + 8:
        raise ValueError("encoded image exceeds limit")
    raw = base64.b64decode(encoded, validate=True)
    if not raw or len(raw) > MAX_IMAGE_BYTES:
        raise ValueError("decoded image exceeds limit")
    from PIL import Image  # type: ignore[import-not-found]

    with Image.open(io.BytesIO(raw)) as image:
        image.load()
        return image.convert("RGB")


def _optional_score(value: Any, name: str) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{name} must be numeric")
    result = float(value)
    if not math.isfinite(result) or not 0 <= result <= 1:
        raise ValueError(f"{name} must be finite and within [0,1]")
    return result


def _validate_request(request: Any) -> tuple[str, Any, float, float, int]:
    if not isinstance(request, dict):
        raise ValueError("request must be an object")
    _ensure_fields(
        request,
        {
            "protocol_version",
            "request_id",
            "operation",
            "model_id",
            "image",
            "queries",
            "target_labels",
            "options",
            "timeout_ms",
        },
        "request",
    )
    if request.get("protocol_version") != PROTOCOL_VERSION:
        raise ValueError("unsupported protocol version")
    request_id = request.get("request_id")
    if not isinstance(request_id, str) or not request_id.strip() or len(request_id) > 128:
        raise ValueError("request id is invalid")
    if request.get("model_id") != MODEL_ID:
        raise ValueError("model id mismatch")
    if request.get("operation") != "object_detection":
        raise ValueError("operation is not supported")
    if request.get("queries", []) != []:
        raise ValueError("trained detector does not accept text queries")
    labels = request.get("target_labels", [])
    if not isinstance(labels, list) or any(not isinstance(label, str) or not label.strip() for label in labels):
        raise ValueError("target labels are invalid")
    options = request.get("options", {})
    if not isinstance(options, dict):
        raise ValueError("options must be an object")
    _ensure_fields(
        options,
        {"confidence_threshold", "iou_threshold", "max_detections", "generation_mode"},
        "options",
    )
    if options.get("generation_mode") is not None:
        raise ValueError("trained detector does not accept generation_mode")
    threshold = _optional_score(options.get("confidence_threshold"), "confidence_threshold")
    iou_threshold = _optional_score(options.get("iou_threshold"), "iou_threshold")
    maximum = options.get("max_detections", MAX_DETECTIONS)
    if maximum is None:
        maximum = MAX_DETECTIONS
    if isinstance(maximum, bool) or not isinstance(maximum, int) or not 1 <= maximum <= MAX_DETECTIONS:
        raise ValueError("max_detections is invalid")
    image = _decode_image(request.get("image"))
    return request_id, image, threshold or 0.0, iou_threshold if iou_threshold is not None else 1.0, maximum


def _iou(left: list[float], right: list[float]) -> float:
    x1 = max(left[0], right[0])
    y1 = max(left[1], right[1])
    x2 = min(left[2], right[2])
    y2 = min(left[3], right[3])
    intersection = max(0.0, x2 - x1) * max(0.0, y2 - y1)
    left_area = (left[2] - left[0]) * (left[3] - left[1])
    right_area = (right[2] - right[0]) * (right[3] - right[1])
    union = left_area + right_area - intersection
    return 0.0 if union <= 0 else intersection / union


def _class_nms(candidates: list[dict[str, Any]], threshold: float, maximum: int) -> list[dict[str, Any]]:
    ordered = sorted(candidates, key=lambda item: (-item["score"], item["detection_id"]))
    kept: list[dict[str, Any]] = []
    for candidate in ordered:
        if any(
            existing["model_label"] == candidate["model_label"]
            and _iou(existing["bbox_xyxy_normalized"], candidate["bbox_xyxy_normalized"]) > threshold
            for existing in kept
        ):
            continue
        kept.append(candidate)
        if len(kept) >= maximum:
            break
    return kept


def _error_response(request: Any, code: str, retryable: bool) -> dict[str, Any]:
    request_id = request.get("request_id", "invalid") if isinstance(request, dict) else "invalid"
    return {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id if isinstance(request_id, str) else "invalid",
        "model_id": MODEL_ID,
        "detections": [],
        "usage": {},
        "warnings": [],
        "error": {"code": code, "message": code.replace("_", " "), "retryable": retryable},
    }


def _cancelled(request_id: str) -> bool:
    with ACTIVE_LOCK:
        return request_id in CANCELLED_REQUESTS


def infer(request: Any) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        request_id, image, confidence_threshold, iou_threshold, maximum = _validate_request(request)
    except Exception as error:
        return _error_response(request, f"invalid_request:{_safe_exception(error)}", False)
    if MODEL is None:
        return _error_response(request, "model_unavailable", False)
    with ACTIVE_LOCK:
        ACTIVE_REQUESTS.add(request_id)
        CANCELLED_REQUESTS.discard(request_id)
    try:
        if _cancelled(request_id):
            return _error_response(request, "cancelled", False)
        width, height = image.size
        with MODEL_LOCK:
            prediction = MODEL.predict(
                image,
                threshold=confidence_threshold,
                include_source_image=False,
            )
        if _cancelled(request_id):
            return _error_response(request, "cancelled", False)
        boxes = prediction.xyxy.tolist()
        scores = prediction.confidence.tolist()
        class_names = prediction.data.get("class_name")
        if class_names is None:
            raise ValueError("checkpoint predictions did not include class_name")
        class_names = class_names.tolist() if hasattr(class_names, "tolist") else list(class_names)
        if not (len(boxes) == len(scores) == len(class_names)):
            raise ValueError("prediction arrays have inconsistent lengths")
        candidates: list[dict[str, Any]] = []
        for index, (box, score, model_label) in enumerate(zip(boxes, scores, class_names, strict=True)):
            model_label = str(model_label)
            score = float(score)
            if model_label not in LABEL_SPACE:
                raise ValueError("prediction label is outside configured label space")
            if not math.isfinite(score) or not 0 <= score <= 1:
                raise ValueError("prediction score is invalid")
            normalized = [float(box[0]) / width, float(box[1]) / height, float(box[2]) / width, float(box[3]) / height]
            if not all(math.isfinite(value) and 0 <= value <= 1 for value in normalized):
                raise ValueError("prediction coordinates are invalid")
            if normalized[2] <= normalized[0] or normalized[3] <= normalized[1]:
                raise ValueError("prediction box has non-positive area")
            candidates.append(
                {
                    "detection_id": f"detection-{index}",
                    "query_id": None,
                    "model_label": model_label,
                    "target_label": None,
                    "bbox_xyxy_normalized": normalized,
                    "score": score,
                    "score_semantics": "relative_confidence",
                }
            )
        detections = _class_nms(candidates, iou_threshold, maximum)
    except Exception as error:
        return _error_response(request, f"inference_failed:{_safe_exception(error)}", True)
    finally:
        with ACTIVE_LOCK:
            ACTIVE_REQUESTS.discard(request_id)
            CANCELLED_REQUESTS.discard(request_id)
    duration_ms = int((time.perf_counter() - started) * 1_000)
    return {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "model_id": MODEL_ID,
        "detections": detections,
        "usage": {"duration_ms": duration_ms, "device": DEVICE},
        "warnings": [],
        "error": None,
    }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: Any) -> None:
        return

    def send_json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, separators=(",", ":"), allow_nan=False).encode()
        if len(body) > MAX_RESPONSE_BYTES:
            status = 500
            body = json.dumps(_error_response({}, "response_limit_exceeded", False)).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path == "/health":
            self.send_json(
                200,
                {
                    "protocol_version": PROTOCOL_VERSION,
                    "worker_id": WORKER_ID,
                    "model_id": MODEL_ID,
                    "status": "healthy" if MODEL is not None else "unavailable",
                    "detail": "ready" if MODEL is not None else (MODEL_ERROR or "model_not_loaded"),
                },
            )
        elif self.path == "/v1/capabilities":
            self.send_json(
                200,
                {
                    "protocol_version": PROTOCOL_VERSION,
                    "worker_id": WORKER_ID,
                    "model_id": MODEL_ID,
                    "capabilities": ["object_detection"],
                    "score_semantics": "relative_confidence",
                    "supports_visual_prompt": False,
                    "supports_batch": False,
                    "label_space": LABEL_SPACE,
                    "limits": {
                        "max_images": 1,
                        "max_input_artifacts": 0,
                        "max_request_bytes": MAX_REQUEST_BYTES,
                        "timeout_seconds": 120,
                    },
                },
            )
        else:
            self.send_json(404, {"error": "not_found"})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        try:
            length = int(self.headers.get("Content-Length", ""))
            if length <= 0 or length > MAX_REQUEST_BYTES:
                raise ValueError("request body size is invalid")
            request = json.loads(self.rfile.read(length))
        except Exception as error:
            self.send_json(400, _error_response({}, f"invalid_request:{_safe_exception(error)}", False))
            return
        if self.path == "/v1/infer":
            result = infer(request)
            self.send_json(200 if result["error"] is None else 422, result)
            return
        if self.path == "/v1/cancel":
            try:
                _ensure_fields(request, {"protocol_version", "request_id", "model_id"}, "cancel")
                request_id = request["request_id"]
                if (
                    request.get("protocol_version") != PROTOCOL_VERSION
                    or request.get("model_id") != MODEL_ID
                    or not isinstance(request_id, str)
                    or not request_id
                ):
                    raise ValueError("cancel request is invalid")
                with ACTIVE_LOCK:
                    active = request_id in ACTIVE_REQUESTS
                    if active:
                        CANCELLED_REQUESTS.add(request_id)
                self.send_json(
                    200,
                    {
                        "protocol_version": PROTOCOL_VERSION,
                        "request_id": request_id,
                        "cancelled": active,
                    },
                )
            except Exception:
                self.send_json(400, {"error": "invalid_cancel_request"})
            return
        self.send_json(404, {"error": "not_found"})


if __name__ == "__main__":
    load_model()
    print(
        f"AnnotAgent RF-DETR worker listening on http://{HOST}:{PORT}; "
        f"status={'healthy' if MODEL is not None else 'unavailable'}"
    )
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
