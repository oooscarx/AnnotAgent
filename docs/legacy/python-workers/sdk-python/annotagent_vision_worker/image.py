"""Bounded inline image decoding. Filesystem paths are deliberately unsupported."""

from __future__ import annotations

import base64
from io import BytesIO

from PIL import Image

from .models import ModelImage

DEFAULT_MAX_IMAGE_BYTES = 32_000_000
DEFAULT_MAX_IMAGE_PIXELS = 100_000_000


def decode_image_bytes(image: ModelImage, max_bytes: int = DEFAULT_MAX_IMAGE_BYTES) -> bytes:
    maximum_encoded = (max_bytes * 4 // 3) + 8
    if len(image.data_base64) > maximum_encoded:
        raise ValueError("encoded image exceeds the Worker limit")
    try:
        decoded = base64.b64decode(image.data_base64, validate=True)
    except ValueError as error:
        raise ValueError("image data is not valid base64") from error
    if not decoded or len(decoded) > max_bytes:
        raise ValueError("decoded image is empty or exceeds the Worker limit")
    return decoded


def decode_pil_image(
    image: ModelImage,
    max_bytes: int = DEFAULT_MAX_IMAGE_BYTES,
    max_pixels: int = DEFAULT_MAX_IMAGE_PIXELS,
) -> Image.Image:
    decoded = decode_image_bytes(image, max_bytes=max_bytes)
    try:
        with Image.open(BytesIO(decoded)) as source:
            width, height = source.size
            if width <= 0 or height <= 0 or width * height > max_pixels:
                raise ValueError("image dimensions exceed the Worker limit")
            expected = "PNG" if image.mime_type == "image/png" else "JPEG"
            if source.format != expected:
                raise ValueError("declared image media type does not match decoded bytes")
            source.load()
            return source.convert("RGB")
    except (OSError, Image.DecompressionBombError) as error:
        raise ValueError("image cannot be decoded safely") from error
