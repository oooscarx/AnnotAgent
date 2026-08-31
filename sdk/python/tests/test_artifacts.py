from __future__ import annotations

from uuid import uuid4

import pytest

from annotagent_vision_worker import (
    bounding_box_artifact,
    instance_mask_artifact,
    mask_set_artifact,
    normalized_xyxy_to_xywh,
    pixel_xyxy_to_normalized,
    polygon_mask_item,
)


def test_coordinates_and_bbox_are_normalized() -> None:
    normalized = pixel_xyxy_to_normalized((10, 20, 60, 80), 100, 100)
    assert normalized == (0.1, 0.2, 0.6, 0.8)
    rect = normalized_xyxy_to_xywh(normalized)
    artifact = bounding_box_artifact(
        image_id=uuid4(),
        task_id="objects",
        source_node="detector",
        rect_xywh=rect,
        model_id="test",
        request_id="request",
    )
    assert artifact["value"]["rect"] == [0.1, 0.2, 0.5, 0.6000000000000001]
    with pytest.raises(ValueError, match="ordered"):
        normalized_xyxy_to_xywh((0.5, 0.5, 0.4, 0.7))


def test_mask_rle_is_dimension_checked() -> None:
    artifact = instance_mask_artifact(
        image_id=uuid4(),
        task_id="objects",
        source_node="segmenter",
        width=2,
        height=2,
        counts="1 2 1",
        model_id="test",
        request_id="request",
    )
    assert artifact["value"]["mask"]["counts"] == "1 2 1"
    with pytest.raises(ValueError, match="dimensions"):
        instance_mask_artifact(
            image_id=uuid4(),
            task_id="objects",
            source_node="segmenter",
            width=2,
            height=2,
            counts="1 1",
            model_id="test",
            request_id="request",
        )


def test_prompted_mask_set_keeps_exact_prompt_reference() -> None:
    image_id = uuid4()
    prompt_set = {
        "artifact_id": "prompts-1",
        "source_node": "prompt-conversion",
        "port": "prompts",
        "artifact_type": "box_prompt_set",
        "item_id": None,
    }
    prompt = {**prompt_set, "item_id": "box-prompt:ball-1"}
    item = polygon_mask_item(
        mask_id="mask-1",
        prompt_ref=prompt,
        rings=[[(0.2, 0.2), (0.4, 0.2), (0.4, 0.4), (0.2, 0.4)]],
        confidence=0.95,
    )
    artifact = mask_set_artifact(
        image_id=image_id,
        source_node="segment",
        model_id="sam2.1",
        source_prompts=prompt_set,
        masks=[item],
        artifact_id="masks-1",
    )
    assert artifact["kind"] == "mask_set"
    assert artifact["artifact"]["masks"][0]["prompt"]["item_id"] == "box-prompt:ball-1"
