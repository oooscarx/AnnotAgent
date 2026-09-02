from __future__ import annotations

from pathlib import Path

import pytest

from annotagent_vision_worker import load_manifest
from annotagent_vision_worker.scaffold import PRESETS, scaffold_worker


@pytest.mark.parametrize("preset", sorted(PRESETS))
def test_every_preset_generates_a_valid_unavailable_worker(tmp_path: Path, preset: str) -> None:
    target = scaffold_worker(tmp_path, name=f"test-{preset}", preset=preset)
    manifest = load_manifest(target / "manifest.yaml")
    assert manifest.availability.value in {"missing_weights", "unconfigured"}
    assert not manifest.availability_evidence.sample_conversion_passed
    assert (target / "app.py").is_file()
    assert (target / "tests" / "test_contract.py").is_file()


def test_scaffold_refuses_traversal_and_overwrite(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="lowercase slug"):
        scaffold_worker(tmp_path, name="../escape", capability="object_detection")
    scaffold_worker(tmp_path, name="safe-worker", capability="object_detection")
    with pytest.raises(FileExistsError, match="refusing to overwrite"):
        scaffold_worker(tmp_path, name="safe-worker", capability="object_detection")
