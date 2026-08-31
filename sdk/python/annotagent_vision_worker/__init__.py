"""Public AnnotAgent Vision Worker SDK surface."""

from .app import create_worker_app
from .artifacts import bounding_box_artifact, instance_mask_artifact
from .cancellation import CancellationRegistry
from .conformance import assert_app_conformance
from .coordinates import (
    normalized_xyxy_to_xywh,
    pixel_xyxy_to_normalized,
    validate_normalized_xyxy,
)
from .errors import inference_error, map_exception
from .image import decode_image_bytes, decode_pil_image
from .manifest import load_manifest
from .models import *  # noqa: F403 - SDK intentionally re-exports its wire contracts

__all__ = [
    "CancellationRegistry",
    "assert_app_conformance",
    "bounding_box_artifact",
    "create_worker_app",
    "decode_image_bytes",
    "decode_pil_image",
    "inference_error",
    "instance_mask_artifact",
    "load_manifest",
    "map_exception",
    "normalized_xyxy_to_xywh",
    "pixel_xyxy_to_normalized",
    "validate_normalized_xyxy",
]
