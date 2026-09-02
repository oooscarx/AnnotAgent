from __future__ import annotations

import importlib.util
from pathlib import Path

from annotagent_vision_worker.models import ExpertModelManifest


def load_reference_worker():
    path = Path(__file__).parents[3] / "examples" / "sam2_vision_worker.py"
    spec = importlib.util.spec_from_file_location("annotagent_sam_reference", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_sam_reference_exposes_a_valid_missing_weights_contract() -> None:
    worker = load_reference_worker()
    manifest = ExpertModelManifest.model_validate(worker.model_manifest())
    assert manifest.capabilities == {"prompted_segmentation"}
    assert manifest.availability == "missing_weights"
    assert manifest.availability_evidence.sample_conversion_passed is False
    assert worker.model_summary()["availability"] == "missing_weights"


def test_sam_reference_reads_exact_pipeline_prompt_identity() -> None:
    worker = load_reference_worker()
    reference = {
        "artifact_id": "box-prompts-1",
        "source_node": "prompt-conversion",
        "port": "prompts",
        "artifact_type": "box_prompt_set",
        "item_id": None,
    }
    context = worker.pipeline_prompt_context(
        {
            "input_artifacts": [
                {
                    "kind": "box_prompt_set",
                    "artifact": {
                        "reference": reference,
                        "prompts": [
                            {
                                "id": "box-prompt:ball-1",
                                "bbox": [0.2, 0.3, 0.1, 0.1],
                            }
                        ],
                    },
                }
            ]
        }
    )
    assert context == (reference, [("box-prompt:ball-1", [0.2, 0.3, 0.1, 0.1])])

