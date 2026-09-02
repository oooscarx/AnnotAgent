"""Coordinate normalization helpers shared by Worker adapters."""

from __future__ import annotations

from math import isfinite
from typing import Iterable


def validate_normalized_xyxy(values: Iterable[float]) -> tuple[float, float, float, float]:
    coordinates = tuple(float(value) for value in values)
    if len(coordinates) != 4:
        raise ValueError("xyxy coordinates require exactly four numbers")
    x1, y1, x2, y2 = coordinates
    if not all(isfinite(value) for value in coordinates):
        raise ValueError("coordinates must be finite")
    if not (0.0 <= x1 < x2 <= 1.0 and 0.0 <= y1 < y2 <= 1.0):
        raise ValueError("coordinates must be ordered and normalized")
    return x1, y1, x2, y2


def pixel_xyxy_to_normalized(
    values: Iterable[float], image_width: int, image_height: int
) -> tuple[float, float, float, float]:
    if image_width <= 0 or image_height <= 0:
        raise ValueError("image dimensions must be positive")
    x1, y1, x2, y2 = (float(value) for value in values)
    return validate_normalized_xyxy(
        (x1 / image_width, y1 / image_height, x2 / image_width, y2 / image_height)
    )


def normalized_xyxy_to_xywh(values: Iterable[float]) -> tuple[float, float, float, float]:
    x1, y1, x2, y2 = validate_normalized_xyxy(values)
    return x1, y1, x2 - x1, y2 - y1
