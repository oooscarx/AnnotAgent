"""Safe Artifact construction and serialization helpers."""

from __future__ import annotations

from datetime import datetime, timezone
from math import isfinite
from typing import Any
from uuid import UUID, uuid4

MAX_MASK_PIXELS = 100_000_000
MAX_RLE_COUNTS_BYTES = 16_000_000


def polygon_mask_item(
    *,
    mask_id: str,
    prompt_ref: dict[str, Any],
    rings: list[list[tuple[float, float]]],
    confidence: float | None = None,
) -> dict[str, Any]:
    if not mask_id.strip() or not rings or any(len(ring) < 3 for ring in rings):
        raise ValueError("polygon mask requires an id and rings with at least three points")
    serialized_rings: list[list[dict[str, float]]] = []
    for ring in rings:
        serialized_ring = []
        for x, y in ring:
            if not all(isfinite(value) and 0 <= value <= 1 for value in (x, y)):
                raise ValueError("polygon mask points must be normalized and finite")
            serialized_ring.append({"x": x, "y": y})
        serialized_rings.append(serialized_ring)
    if confidence is not None and (not isfinite(confidence) or not 0 <= confidence <= 1):
        raise ValueError("mask confidence must be finite and within [0,1]")
    _validate_item_ref(prompt_ref, {"box_prompt_set", "point_prompt_set"})
    return {
        "mask_id": mask_id,
        "prompt": prompt_ref,
        "mask": {"encoding": "polygon", "rings": serialized_rings},
        "score": {
            "value": confidence,
            "semantics": "relative_confidence" if confidence is not None else "not_provided",
        },
        "attributes": {},
    }


def mask_set_artifact(
    *,
    image_id: UUID,
    source_node: str,
    model_id: str,
    source_prompts: dict[str, Any],
    masks: list[dict[str, Any]],
    artifact_id: str | None = None,
) -> dict[str, Any]:
    _validate_set_ref(source_prompts, {"box_prompt_set", "point_prompt_set"})
    if not source_node.strip() or not model_id.strip():
        raise ValueError("MaskSet requires source_node and model_id")
    ids = [mask.get("mask_id") for mask in masks]
    if any(not isinstance(item, str) or not item.strip() for item in ids) or len(ids) != len(set(ids)):
        raise ValueError("MaskSet mask ids must be non-empty and unique")
    reference = {
        "artifact_id": artifact_id or f"mask-set:{uuid4()}",
        "source_node": source_node,
        "port": "masks",
        "artifact_type": "mask_set",
        "item_id": None,
    }
    return {
        "kind": "mask_set",
        "artifact": {
            "reference": reference,
            "image_id": str(image_id),
            "model_binding": model_id,
            "source_prompts": source_prompts,
            "validation_state": "unvalidated",
            "masks": masks,
            "metadata": {},
        },
    }


def _validate_set_ref(reference: dict[str, Any], kinds: set[str]) -> None:
    if (
        reference.get("artifact_type") not in kinds
        or not reference.get("artifact_id")
        or not reference.get("source_node")
        or not reference.get("port")
        or reference.get("item_id") is not None
    ):
        raise ValueError("reference must identify a supported prompt set")


def _validate_item_ref(reference: dict[str, Any], kinds: set[str]) -> None:
    if reference.get("item_id") in (None, ""):
        raise ValueError("reference must identify a prompt set item")
    itemless = dict(reference)
    itemless["item_id"] = None
    _validate_set_ref(itemless, kinds)


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
