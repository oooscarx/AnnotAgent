"""Generate a Worker adapter without modifying AnnotAgent Core."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

import yaml


@dataclass(frozen=True)
class Preset:
    capability: tuple[str, ...]
    output: str
    geometry: str
    score: str = "relative_confidence"
    prompts: tuple[str, ...] = ()
    architecture: str | None = None


PRESETS: dict[str, Preset] = {
    "sam2": Preset(
        ("prompted_segmentation",),
        "mask_set",
        "mask_refined_geometry",
        prompts=("box", "point"),
        architecture="sam2",
    ),
    "yolo": Preset(("object_detection",), "detection_set", "predicted_geometry", architecture="yolo"),
    "rfdetr": Preset(("object_detection",), "detection_set", "predicted_geometry", architecture="rf-detr"),
    "locate-anything": Preset(
        ("open_vocabulary_detection", "phrase_grounding"),
        "detection_set",
        "predicted_geometry",
        score="not_provided",
        prompts=("text",),
        architecture="locate-anything",
    ),
    "pidnet": Preset(
        ("semantic_segmentation",),
        "semantic_mask",
        "predicted_geometry",
        architecture="pidnet",
    ),
    "grounding-dino": Preset(
        ("open_vocabulary_detection", "phrase_grounding"),
        "detection_set",
        "predicted_geometry",
        prompts=("text",),
        architecture="grounding-dino",
    ),
}


def scaffold_worker(
    output_root: str | Path,
    *,
    name: str,
    capability: str | None = None,
    preset: str | None = None,
) -> Path:
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,62}", name):
        raise ValueError("Worker name must be a lowercase slug of at most 63 characters")
    if preset is not None and preset not in PRESETS:
        raise ValueError(f"unknown preset {preset!r}")
    selected = PRESETS.get(preset) if preset else _generic_preset(capability)
    if selected is None:
        raise ValueError("--capability is required when --preset is omitted")
    root = Path(output_root).expanduser().resolve()
    target = root / name
    if target.exists():
        raise FileExistsError(f"refusing to overwrite {target}")
    target.mkdir(parents=True)
    (target / "tests").mkdir()
    manifest = _manifest(name, selected)
    (target / "manifest.yaml").write_text(
        yaml.safe_dump(manifest, sort_keys=False), encoding="utf-8"
    )
    (target / "app.py").write_text(_app_template(), encoding="utf-8")
    (target / "model.py").write_text(_model_template(), encoding="utf-8")
    (target / "requirements.txt").write_text(
        "annotagent-vision-worker>=0.1.0,<0.2\n", encoding="utf-8"
    )
    (target / "tests" / "test_contract.py").write_text(
        _test_template(), encoding="utf-8"
    )
    (target / "README.md").write_text(_readme_template(name, preset), encoding="utf-8")
    return target


def _generic_preset(capability: str | None) -> Preset | None:
    if capability is None:
        return None
    mapping = {
        "object_detection": ("detection_set", "predicted_geometry"),
        "open_vocabulary_detection": ("detection_set", "predicted_geometry"),
        "image_classification": ("classification_set", "not_applicable"),
        "semantic_segmentation": ("semantic_mask", "predicted_geometry"),
        "prompted_segmentation": ("mask_set", "mask_refined_geometry"),
        "instance_segmentation": ("instance_mask", "predicted_geometry"),
        "keypoint_detection": ("keypoints", "predicted_geometry"),
    }
    if capability not in mapping:
        raise ValueError(f"unsupported scaffold capability {capability!r}")
    output, geometry = mapping[capability]
    prompts = ("box", "point") if capability == "prompted_segmentation" else ()
    return Preset((capability,), output, geometry, prompts=prompts)


def _manifest(name: str, preset: Preset) -> dict[str, object]:
    return {
        "schema_version": "1",
        "model_id": f"{name}-model",
        "display_name": name.replace("-", " ").title(),
        "architecture": preset.architecture,
        "model_version": "unconfigured",
        "connection": {
            "kind": "vision_worker_model",
            "worker_id": name,
            "worker_model_id": f"{name}-model",
        },
        "capabilities": list(preset.capability),
        "input_contracts": [
            {
                "name": "image",
                "data_type": {"artifact": "image"},
                "required": True,
                "multiple": False,
            },
            *(
                [
                    {
                        "name": "box_prompts",
                        "data_type": {"artifact": "box_prompt_set"},
                        "required": False,
                        "multiple": True,
                    },
                    {
                        "name": "point_prompts",
                        "data_type": {"artifact": "point_prompt_set"},
                        "required": False,
                        "multiple": True,
                    },
                ]
                if "prompted_segmentation" in preset.capability
                else []
            ),
        ],
        "output_contracts": [
            {
                "name": "outputs",
                "data_type": {"artifact": preset.output},
                "required": True,
                "multiple": True,
            }
        ],
        "prompt_contracts": [
            {"kind": kind, "required": False, "multiple": True} for kind in preset.prompts
        ],
        "score_semantics": preset.score,
        "geometry_semantics": preset.geometry,
        "label_space": None,
        "checkpoint": None,
        "runtime_requirements": {
            "devices": [],
            "minimum_gpu_memory_mb": None,
            "dependencies": [],
            "supports_batch": False,
        },
        "license": {
            "code_license": None,
            "weight_license": None,
            "source_url": None,
            "commercial_use": "unknown",
            "redistribution": "unknown",
            "usage_notes": ["Complete and verify license metadata before registration."],
            "verified_from_official_source": False,
        },
        "availability": "missing_weights" if preset.architecture else "unconfigured",
        "availability_evidence": {
            "health_passed": False,
            "protocol_compatible": True,
            "contracts_validated": True,
            "sample_conversion_passed": False,
            "weights_ready": False,
            "checked_at": None,
            "detail": "Generated adapter template; no checkpoint has been configured or tested.",
        },
        "metadata": {"generated_by": "annotagent-vision-worker scaffold"},
    }


def _app_template() -> str:
    return '''from pathlib import Path

import uvicorn

from annotagent_vision_worker import create_worker_app, load_manifest
from model import infer, warmup

MANIFEST = load_manifest(Path(__file__).with_name("manifest.yaml"))
app = create_worker_app([MANIFEST], infer, warmup=warmup)

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8790)
'''


def _model_template() -> str:
    return '''from threading import Event

from annotagent_vision_worker import InferenceRequest, InferenceResponse, inference_error


def warmup(model_id: str) -> bool:
    # Load only explicitly configured local weights here. Never download weights implicitly.
    return False


def infer(request: InferenceRequest, cancellation: Event) -> InferenceResponse:
    if cancellation.is_set():
        return inference_error(request.request_id, request.model_id, "cancelled", "request cancelled")
    return inference_error(
        request.request_id,
        request.model_id,
        "weights_unavailable",
        "configure local model weights and implement model.py before inference",
    )
'''


def _test_template() -> str:
    return '''from app import app
from annotagent_vision_worker import assert_app_conformance


def test_worker_contract() -> None:
    assert_app_conformance(app)
'''


def _readme_template(name: str, preset: str | None) -> str:
    selected = preset or "custom capability"
    return f"""# {name}

Generated AnnotAgent Vision Worker ({selected}).

1. Complete immutable model/checkpoint/dataset/license identity in `manifest.yaml`.
2. Implement explicit local loading and inference in `model.py`.
3. Run `python -m pytest`.
4. Start `python app.py` and use AnnotAgent's explicit sample test.

The generated Worker does not download weights and starts as unavailable.
"""
