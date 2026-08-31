"""Safe Artifact construction and serialization helpers."""

from __future__ import annotations

from datetime import datetime, timezone
from math import isfinite
from typing import Any
from uuid import UUID, uuid4

MAX_MASK_PIXELS = 100_000_000
MAX_RLE_COUNTS_BYTES = 16_000_000


def bounding_box_artifact(
    *,
    image_id: UUID,
    task_id: str,
    source_node: str,
    rect_xywh: tuple[float, float, float, float],
    model_id: str,
    request_id: str,
    label: str | None = None,
    confidence: float | None = None,
    input_artifact_ids: list[UUID] | None = None,
) -> dict[str, Any]:
    x, y, width, height = rect_xywh
    if not all(isfinite(value) for value in rect_xywh):
        raise ValueError("bounding box values must be finite")
    if x < 0 or y < 0 or width <= 0 or height <= 0 or x + width > 1 or y + height > 1:
        raise ValueError("bounding box must be normalized xywh")
    if confidence is not None and (not isfinite(confidence) or not 0 <= confidence <= 1):
        raise ValueError("confidence must be finite and within [0,1]")
    return _artifact(
        image_id=image_id,
        task_id=task_id,
        source_node=source_node,
        model_id=model_id,
        request_id=request_id,
        label=label,
        confidence=confidence,
        input_artifact_ids=input_artifact_ids,
        value={"kind": "bounding_box", "rect": [x, y, width, height]},
    )


def instance_mask_artifact(
    *,
    image_id: UUID,
    task_id: str,
    source_node: str,
    width: int,
    height: int,
    counts: str,
    model_id: str,
    request_id: str,
    label: str | None = None,
    confidence: float | None = None,
    input_artifact_ids: list[UUID] | None = None,
) -> dict[str, Any]:
    if width <= 0 or height <= 0 or width * height > MAX_MASK_PIXELS:
        raise ValueError("mask dimensions are invalid or exceed the limit")
    if not counts or len(counts.encode()) > MAX_RLE_COUNTS_BYTES:
        raise ValueError("mask RLE is empty or exceeds the limit")
    runs = counts.split()
    if not runs or any(not run.isdecimal() for run in runs):
        raise ValueError("mask RLE must contain non-negative integer runs")
    if sum(int(run) for run in runs) != width * height:
        raise ValueError("mask RLE dimensions do not match its run lengths")
    return _artifact(
        image_id=image_id,
        task_id=task_id,
        source_node=source_node,
        model_id=model_id,
        request_id=request_id,
        label=label,
        confidence=confidence,
        input_artifact_ids=input_artifact_ids,
        value={
            "kind": "instance_mask",
            "mask": {
                "encoding": "coco_rle",
                "width": width,
                "height": height,
                "counts": counts,
            },
        },
    )


def _artifact(
    *,
    image_id: UUID,
    task_id: str,
    source_node: str,
    model_id: str,
    request_id: str,
    value: dict[str, Any],
    label: str | None,
    confidence: float | None,
    input_artifact_ids: list[UUID] | None,
) -> dict[str, Any]:
    return {
        "id": str(uuid4()),
        "image_id": str(image_id),
        "task_id": task_id,
        "label": label,
        "role": "candidate",
        "value": value,
        "source_node": source_node,
        "confidence": confidence,
        "metadata": {},
        "validation_state": "unvalidated",
        "provenance": {
            "provider": "annotagent_vision_worker",
            "model": model_id,
            "tool": None,
            "request_id": request_id,
            "model_digest": None,
            "input_artifact_ids": [str(item) for item in input_artifact_ids or []],
        },
        "revision": 1,
        "replaces_artifact_id": None,
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
