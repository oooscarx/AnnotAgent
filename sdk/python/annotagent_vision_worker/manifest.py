"""Load and validate Expert Model Manifests without resolving model code or weights."""

from __future__ import annotations

from pathlib import Path

import yaml

from .models import ExpertModelManifest


def load_manifest(path: str | Path) -> ExpertModelManifest:
    resolved = Path(path).expanduser().resolve(strict=True)
    if not resolved.is_file():
        raise ValueError("manifest path is not a file")
    raw = resolved.read_bytes()
    if len(raw) > 1_000_000:
        raise ValueError("manifest exceeds the size limit")
    value = yaml.safe_load(raw)
    if not isinstance(value, dict):
        raise ValueError("manifest root must be a mapping")
    return ExpertModelManifest.model_validate(value)
