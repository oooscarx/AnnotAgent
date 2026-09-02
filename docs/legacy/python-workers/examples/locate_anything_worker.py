#!/usr/bin/env python3
"""LocateAnything adapter for AnnotAgent Detection Worker Protocol v1.

The adapter loads only an explicitly configured local model and official worker source. It never
downloads weights and never logs requests, images, headers, model output, or local paths.

Required for real inference:
  ANNOTAGENT_LOCATEANYTHING_MODEL_PATH=/absolute/local/model-directory
  ANNOTAGENT_LOCATEANYTHING_CODE_PATH=/absolute/NVlabs/Eagle/Embodied

The configured code directory must contain the official ``locateanything_worker.py``. Without
both paths the process remains available for health/capability discovery and reports unavailable
model health; use AnnotAgent's Rust Mock Grounding Backend for offline inference tests.
"""

from __future__ import annotations

import base64
import io
import json
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

PROTOCOL_VERSION = 1
WORKER_ID = "annotagent-locate-anything"
MODEL_ID = os.environ.get("ANNOTAGENT_LOCATEANYTHING_MODEL_ID", "locate-anything-local")
MODEL_PATH = os.environ.get("ANNOTAGENT_LOCATEANYTHING_MODEL_PATH", "")
CODE_PATH = os.environ.get("ANNOTAGENT_LOCATEANYTHING_CODE_PATH", "")
HOST = os.environ.get("ANNOTAGENT_LOCATEANYTHING_HOST", "127.0.0.1")
PORT = int(os.environ.get("ANNOTAGENT_LOCATEANYTHING_PORT", "8791"))
MAX_IMAGE_BYTES = 32_000_000
MAX_REQUEST_BYTES = 44_000_000
MAX_RESPONSE_BYTES = 2_000_000
MAX_QUERIES = 100
MAX_QUERY_TEXT = 2_000
MAX_DETECTIONS = 10_000

MODEL: Any | None = None
MODEL_ERROR: str | None = None
DEVICE = "unknown"
MODEL_LOCK = threading.Lock()
ACTIVE_LOCK = threading.Lock()
ACTIVE_REQUESTS: set[str] = set()
CANCELLED_REQUESTS: set[str] = set()


def _safe_exception(error: Exception) -> str:
    """Expose an exception category without leaking paths, prompts, or model output."""
    return type(error).__name__[:100]


def load_model() -> None:
    global MODEL, MODEL_ERROR, DEVICE
    if not MODEL_PATH or not CODE_PATH:
        MODEL_ERROR = "local_model_and_official_code_paths_required"
        return
    try:
        model_path = Path(MODEL_PATH).expanduser().resolve(strict=True)
        code_path = Path(CODE_PATH).expanduser().resolve(strict=True)
        if not model_path.is_dir() or not (code_path / "locateanything_worker.py").is_file():
            raise RuntimeError("invalid local LocateAnything installation")
        sys.path.insert(0, str(code_path))
        import torch  # type: ignore[import-not-found]
        from locateanything_worker import LocateAnythingWorker  # type: ignore[import-not-found]

        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is required by the tracked LocateAnything profile")
        DEVICE = "cuda"
        MODEL = LocateAnythingWorker(str(model_path), device=DEVICE)
    except Exception as error:  # surfaced through bounded health metadata
        MODEL_ERROR = f"model_load_failed:{_safe_exception(error)}"


def _ensure_fields(value: dict[str, Any], allowed: set[str], scope: str) -> None:
    unknown = set(value) - allowed
    if unknown:
        raise ValueError(f"{scope} contains unsupported fields")


def _decode_request_image(value: Any):
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


def _validate_queries(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not 1 <= len(value) <= MAX_QUERIES:
        raise ValueError("queries must contain between 1 and 100 entries")
    result: list[dict[str, Any]] = []
    ids: set[str] = set()
    for query in value:
        if not isinstance(query, dict):
            raise ValueError("query must be an object")
        _ensure_fields(query, {"id", "text", "target_label"}, "query")
        query_id = query.get("id")
        text = query.get("text")
        target = query.get("target_label")
        if (
            not isinstance(query_id, str)
            or not query_id.strip()
            or len(query_id) > 128
            or query_id in ids
            or not isinstance(text, str)
            or not text.strip()
            or len(text) > MAX_QUERY_TEXT
            or (target is not None and (not isinstance(target, str) or not target.strip()))
        ):
            raise ValueError("query identity, text, or target label is invalid")
        ids.add(query_id)
        result.append({"id": query_id, "text": text, "target_label": target})
    return result


def _validate_request(request: Any) -> tuple[str, str, list[dict[str, Any]], dict[str, Any]]:
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
    operation = request.get("operation")
    if operation not in {"open_vocabulary_detection", "phrase_grounding"}:
        raise ValueError("operation is not supported")
    queries = _validate_queries(request.get("queries"))
    target_labels = request.get("target_labels", [])
    if not isinstance(target_labels, list) or any(
        not isinstance(item, str) or not item.strip() for item in target_labels
    ):
        raise ValueError("target labels are invalid")
    options = request.get("options", {})
    if not isinstance(options, dict):
        raise ValueError("options must be an object")
    _ensure_fields(
        options,
        {"confidence_threshold", "iou_threshold", "max_detections", "generation_mode"},
        "options",
    )
    maximum = options.get("max_detections", MAX_DETECTIONS)
    if not isinstance(maximum, int) or isinstance(maximum, bool) or not 1 <= maximum <= MAX_DETECTIONS:
        raise ValueError("max_detections is invalid")
    generation_mode = options.get("generation_mode", "hybrid")
    if generation_mode not in {"fast", "slow", "hybrid"}:
        raise ValueError("generation_mode is invalid")
    return request_id, operation, queries, {"max_detections": maximum, "generation_mode": generation_mode}


def _error_response(request: Any, code: str, retryable: bool) -> dict[str, Any]:
    request_id = request.get("request_id", "invalid") if isinstance(request, dict) else "invalid"
    model_id = request.get("model_id", MODEL_ID) if isinstance(request, dict) else MODEL_ID
    if model_id != MODEL_ID:
        model_id = MODEL_ID
    return {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id if isinstance(request_id, str) else "invalid",
        "model_id": model_id,
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
        request_id, operation, queries, options = _validate_request(request)
        image = _decode_request_image(request["image"])
    except Exception as error:
        return _error_response(request, f"invalid_request:{_safe_exception(error)}", False)
    if MODEL is None:
        return _error_response(request, "model_unavailable", False)
    with ACTIVE_LOCK:
        ACTIVE_REQUESTS.add(request_id)
        CANCELLED_REQUESTS.discard(request_id)
    detections: list[dict[str, Any]] = []
    try:
        width, height = image.size
        with MODEL_LOCK:
            for query in queries:
                if _cancelled(request_id):
                    return _error_response(request, "cancelled", False)
                if operation == "open_vocabulary_detection":
                    result = MODEL.detect(
                        image,
                        [query["text"]],
                        generation_mode=options["generation_mode"],
                        verbose=False,
                    )
                else:
                    result = MODEL.ground_multi(
                        image,
                        query["text"],
                        generation_mode=options["generation_mode"],
                        verbose=False,
                    )
                answer = result.get("answer", "")
                if not isinstance(answer, str):
                    raise ValueError("model answer is not text")
                for box in MODEL.parse_boxes(answer, width, height):
                    x1 = float(box["x1"]) / width
                    y1 = float(box["y1"]) / height
                    x2 = float(box["x2"]) / width
                    y2 = float(box["y2"]) / height
                    if not (0 <= x1 < x2 <= 1 and 0 <= y1 < y2 <= 1):
                        raise ValueError("model returned invalid box geometry")
                    detections.append(
                        {
                            "detection_id": f"{query['id']}:{len(detections)}",
                            "query_id": query["id"],
                            "model_label": None,
                            "target_label": query["target_label"],
                            "bbox_xyxy_normalized": [x1, y1, x2, y2],
                            "score": None,
                            "score_semantics": "not_provided",
                        }
                    )
                    if len(detections) >= options["max_detections"]:
                        break
                if len(detections) >= options["max_detections"]:
                    break
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
                    "capabilities": ["open_vocabulary_detection", "phrase_grounding"],
                    "score_semantics": "not_provided",
                    "supports_visual_prompt": False,
                    "supports_batch": False,
                    "label_space": [],
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
        length_text = self.headers.get("Content-Length", "")
        try:
            length = int(length_text)
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
                valid = (
                    request.get("protocol_version") == PROTOCOL_VERSION
                    and request.get("model_id") == MODEL_ID
                    and isinstance(request_id, str)
                    and bool(request_id)
                )
                if not valid:
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
        f"AnnotAgent LocateAnything worker listening on http://{HOST}:{PORT}; "
        f"status={'healthy' if MODEL is not None else 'unavailable'}"
    )
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
