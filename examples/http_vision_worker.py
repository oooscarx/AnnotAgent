#!/usr/bin/env python3
"""Reference AnnotAgent Vision Worker.

Without ANNOTAGENT_MODEL_PATH this is an explicitly labelled protocol fixture. When a local
Ultralytics-compatible weights path is configured, object_detection requests run real inference.
The worker never calls itself "real" in fixture mode.
"""

from __future__ import annotations

import base64
import json
import os
import time
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
from pathlib import Path
from typing import Any

PROTOCOL_VERSION = 1
MODEL_PATH = os.environ.get("ANNOTAGENT_MODEL_PATH")
MODEL: Any | None = None
MODEL_ERROR: str | None = None

if MODEL_PATH:
    try:
        resolved = Path(MODEL_PATH).expanduser().resolve(strict=True)
        from ultralytics import YOLO  # type: ignore[import-not-found]

        MODEL = YOLO(str(resolved))
    except Exception as error:  # surfaced by /health; never silently becomes real inference
        MODEL_ERROR = f"{type(error).__name__}: {error}"


def response_artifact(request: dict[str, Any], label: str, box: list[float], confidence: float) -> dict[str, Any]:
    return {
        "id": str(uuid.uuid4()),
        "image_id": request["image_id"],
        "task_id": request["task_id"],
        "label": label,
        "role": "candidate",
        "value": {"kind": "bounding_box", "rect": box},
        "source_node": request["node_id"],
        "confidence": confidence,
        "metadata": {},
        "validation_state": "unvalidated",
        "provenance": {
            "provider": "reference_http_worker",
            "model": Path(MODEL_PATH).name if MODEL_PATH else "fixture",
            "tool": None,
            "request_id": request["request_id"],
            "model_digest": None,
            "input_artifact_ids": [],
        },
        "revision": 1,
        "replaces_artifact_id": None,
        "created_at": datetime.now(timezone.utc).isoformat(),
    }


def infer(request: dict[str, Any]) -> dict[str, Any]:
    started = time.perf_counter()
    if request.get("protocol_version") != PROTOCOL_VERSION:
        return wire_error(request, "protocol_version_mismatch", "unsupported protocol version", False)
    if request.get("operation") != "object_detection":
        return wire_error(request, "unsupported_operation", "reference real adapter supports object_detection", False)
    if MODEL is None:
        detail = MODEL_ERROR or "ANNOTAGENT_MODEL_PATH is not configured; worker is in fixture mode"
        return wire_error(request, "weights_unavailable", detail, False)
    image = request.get("image")
    if not image or not image.get("data_base64"):
        return wire_error(request, "image_required", "inline bounded image input is required", False)
    try:
        from PIL import Image  # type: ignore[import-not-found]

        decoded = Image.open(BytesIO(base64.b64decode(image["data_base64"], validate=True)))
        prediction = MODEL.predict(decoded, verbose=False)[0]
        width, height = decoded.size
        names = prediction.names
        artifacts = []
        for box in prediction.boxes:
            x1, y1, x2, y2 = [float(value) for value in box.xyxy[0].tolist()]
            label = str(names[int(box.cls[0])])
            artifacts.append(
                response_artifact(
                    request,
                    label,
                    [x1 / width, y1 / height, (x2 - x1) / width, (y2 - y1) / height],
                    float(box.conf[0]),
                )
            )
    except Exception as error:
        return wire_error(request, "inference_failed", f"{type(error).__name__}: {error}", True)
    elapsed = int((time.perf_counter() - started) * 1000)
    return {
        "protocol_version": PROTOCOL_VERSION,
        "model_identity": f"ultralytics:{Path(MODEL_PATH).name}",
        "artifacts": artifacts,
        "request_id": request["request_id"],
        "metadata": {"mode": "real_local_weights"},
        "usage": {"source": "actual", "compute_milliseconds": elapsed},
        "warnings": [],
        "timings": {"inference_ms": elapsed, "total_ms": elapsed},
        "error": None,
    }


def wire_error(request: dict[str, Any], code: str, message: str, retryable: bool) -> dict[str, Any]:
    return {
        "protocol_version": PROTOCOL_VERSION,
        "model_identity": Path(MODEL_PATH).name if MODEL_PATH else "fixture-no-weights",
        "artifacts": [],
        "request_id": request.get("request_id"),
        "metadata": {"mode": "real_local_weights" if MODEL else "fixture"},
        "usage": {},
        "warnings": [],
        "timings": {},
        "error": {"code": code, "message": message, "retryable": retryable},
    }


class Handler(BaseHTTPRequestHandler):
    def send_json(self, status: int, value: dict[str, Any]) -> None:
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path == "/health":
            status = "healthy" if MODEL is not None else ("unavailable" if MODEL_PATH else "degraded")
            self.send_json(200, {"status": status, "detail": MODEL_ERROR or "fixture mode", "checked_at": None})
        elif self.path == "/v1/capabilities":
            self.send_json(
                200,
                {
                    "protocol_version": PROTOCOL_VERSION,
                    "worker_id": "annotagent-reference-worker",
                    "model_identity": Path(MODEL_PATH).name if MODEL_PATH else "fixture-no-weights",
                    "capabilities": ["object_detection"],
                    "input_types": ["image"],
                    "output_types": ["bounding_box"],
                    "limits": {"max_images": 1, "max_request_bytes": 20_000_000},
                },
            )
        else:
            self.send_json(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path != "/v1/infer":
            self.send_json(404, {"error": "not found"})
            return
        length = min(int(self.headers.get("Content-Length", "0")), 20_000_000)
        try:
            request = json.loads(self.rfile.read(length))
            self.send_json(200, infer(request))
        except Exception as error:
            self.send_json(400, {"error": f"invalid request: {error}"})

    def log_message(self, format: str, *args: Any) -> None:
        return  # Avoid accidental request/header logging; product traces live in Rust.


if __name__ == "__main__":
    host = os.environ.get("ANNOTAGENT_WORKER_HOST", "127.0.0.1")
    port = int(os.environ.get("ANNOTAGENT_WORKER_PORT", "8790"))
    print(f"AnnotAgent reference worker listening on http://{host}:{port}; mode={'real' if MODEL else 'fixture'}")
    ThreadingHTTPServer((host, port), Handler).serve_forever()
