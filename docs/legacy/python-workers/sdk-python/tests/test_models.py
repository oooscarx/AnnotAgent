from __future__ import annotations

from uuid import uuid4

import pytest
from pydantic import ValidationError

from annotagent_vision_worker.models import ExpertModelManifest


def manifest_value() -> dict[str, object]:
    return {
        "schema_version": "1",
        "model_id": "test-segmenter",
        "display_name": "Test Segmenter",
        "architecture": "test",
        "model_version": "1",
        "connection": {
            "kind": "vision_worker_model",
            "worker_id": "test-worker",
            "worker_model_id": "test-segmenter",
        },
        "capabilities": ["prompted_segmentation"],
        "input_contracts": [
            {
                "name": "image",
                "data_type": {"artifact": "image"},
                "required": True,
                "multiple": False,
            },
            {
                "name": "box_prompts",
                "data_type": {"artifact": "box_prompt_set"},
                "required": True,
                "multiple": True,
            },
        ],
        "output_contracts": [
            {
                "name": "masks",
                "data_type": {"artifact": "mask_set"},
                "required": True,
                "multiple": True,
            }
        ],
        "prompt_contracts": [{"kind": "box", "required": True, "multiple": True}],
        "score_semantics": "relative_confidence",
        "geometry_semantics": "mask_refined_geometry",
        "label_space": None,
        "checkpoint": {"sha256": "a" * 64, "source": None, "training_dataset_version": None},
        "runtime_requirements": {
            "devices": ["cpu"],
            "minimum_gpu_memory_mb": None,
            "dependencies": [],
            "supports_batch": False,
        },
        "license": {
            "code_license": "MIT",
            "weight_license": None,
            "source_url": None,
            "commercial_use": "unknown",
            "redistribution": "unknown",
            "usage_notes": [],
            "verified_from_official_source": False,
        },
        "availability": "unknown",
        "availability_evidence": {
            "health_passed": False,
            "protocol_compatible": True,
            "contracts_validated": True,
            "sample_conversion_passed": False,
            "weights_ready": False,
            "checked_at": None,
            "detail": None,
        },
        "metadata": {"fixture": str(uuid4())},
    }


def test_prompted_segmentation_contract_is_strict() -> None:
    manifest = ExpertModelManifest.model_validate(manifest_value())
    assert manifest.model_id == "test-segmenter"
    invalid = manifest_value()
    invalid["prompt_contracts"] = []
    with pytest.raises(ValidationError, match="box or point"):
        ExpertModelManifest.model_validate(invalid)


def test_available_requires_complete_evidence() -> None:
    invalid = manifest_value()
    invalid["availability"] = "available"
    with pytest.raises(ValidationError, match="complete registration evidence"):
        ExpertModelManifest.model_validate(invalid)
