from __future__ import annotations

from uuid import uuid4

import pytest

from annotagent_vision_worker import (
    bounding_box_artifact,
    instance_mask_artifact,
    normalized_xyxy_to_xywh,
    pixel_xyxy_to_normalized,
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
