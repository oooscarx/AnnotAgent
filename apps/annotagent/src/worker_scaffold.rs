use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

pub struct ScaffoldRequest<'a> {
    pub output_root: &'a Path,
    pub name: Option<&'a str>,
    pub capability: Option<&'a str>,
    pub preset: Option<&'a str>,
    pub language: &'a str,
}

#[derive(Clone, Copy)]
struct ScaffoldSpec {
    capabilities: &'static [&'static str],
    output: &'static str,
    geometry: &'static str,
    score: &'static str,
    prompts: &'static [&'static str],
    architecture: Option<&'static str>,
}

pub fn scaffold(request: &ScaffoldRequest<'_>) -> Result<PathBuf> {
    if request.language != "python" {
        bail!("only --language python is available in this release");
    }
    let spec = request
        .preset
        .map(preset)
        .transpose()?
        .or_else(|| request.capability.and_then(generic_capability))
        .with_context(|| "--capability is required when --preset is omitted")?;
    if request.preset.is_none()
        && request
            .capability
            .is_some_and(|capability| generic_capability(capability).is_none())
    {
        bail!("unsupported Worker scaffold capability");
    }
    let default_name = request.preset.map(|preset| format!("{preset}-worker"));
    let name = request
        .name
        .map(str::to_owned)
        .or(default_name)
        .with_context(|| "--name is required when --preset is omitted")?;
    validate_slug(&name)?;
    let target = request.output_root.join(&name);
    if target.exists() {
        bail!(
            "{} already exists; refusing to overwrite it",
            target.display()
        );
    }
    std::fs::create_dir_all(target.join("tests"))
        .with_context(|| format!("cannot create {}", target.display()))?;
    write(&target.join("manifest.yaml"), &manifest(&name, spec))?;
    write(&target.join("app.py"), APP_TEMPLATE)?;
    write(&target.join("model.py"), MODEL_TEMPLATE)?;
    write(
        &target.join("requirements.txt"),
        "annotagent-vision-worker>=0.1.0,<0.2\n",
    )?;
    write(&target.join("tests/test_contract.py"), TEST_TEMPLATE)?;
    write(
        &target.join("README.md"),
        &format!(
            "# {name}\n\nGenerated AnnotAgent Vision Worker. Complete immutable model, checkpoint, dataset and license identity; implement explicit local loading in `model.py`; run `python -m pytest`; then use AnnotAgent's sample test. No weights are downloaded and this Worker starts unavailable.\n"
        ),
    )?;
    Ok(target)
}

fn write(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("cannot write {}", path.display()))
}

fn validate_slug(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 63
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        bail!("Worker name must be a lowercase slug of at most 63 characters");
    }
    Ok(())
}

fn preset(value: &str) -> Result<ScaffoldSpec> {
    match value {
        "sam2" => Ok(ScaffoldSpec {
            capabilities: &["prompted_segmentation"],
            output: "instance_mask",
            geometry: "mask_refined_geometry",
            score: "relative_confidence",
            prompts: &["box", "point"],
            architecture: Some("sam2"),
        }),
        "yolo" => Ok(detection_spec("yolo")),
        "rfdetr" => Ok(detection_spec("rf-detr")),
        "locate-anything" => Ok(ScaffoldSpec {
            capabilities: &["open_vocabulary_detection", "phrase_grounding"],
            output: "detection_set",
            geometry: "predicted_geometry",
            score: "not_provided",
            prompts: &["text"],
            architecture: Some("locate-anything"),
        }),
        "pidnet" => Ok(ScaffoldSpec {
            capabilities: &["semantic_segmentation"],
            output: "semantic_mask",
            geometry: "predicted_geometry",
            score: "relative_confidence",
            prompts: &[],
            architecture: Some("pidnet"),
        }),
        "grounding-dino" => Ok(ScaffoldSpec {
            capabilities: &["open_vocabulary_detection", "phrase_grounding"],
            output: "detection_set",
            geometry: "predicted_geometry",
            score: "relative_confidence",
            prompts: &["text"],
            architecture: Some("grounding-dino"),
        }),
        _ => bail!(
            "unknown preset {value:?}; choose sam2, yolo, rfdetr, locate-anything, pidnet, or grounding-dino"
        ),
    }
}

const fn detection_spec(architecture: &'static str) -> ScaffoldSpec {
    ScaffoldSpec {
        capabilities: &["object_detection"],
        output: "detection_set",
        geometry: "predicted_geometry",
        score: "relative_confidence",
        prompts: &[],
        architecture: Some(architecture),
    }
}

fn generic_capability(value: &str) -> Option<ScaffoldSpec> {
    match value {
        "object_detection" => Some(ScaffoldSpec {
            architecture: None,
            ..detection_spec("custom")
        }),
        "open_vocabulary_detection" => Some(ScaffoldSpec {
            capabilities: &["open_vocabulary_detection"],
            output: "detection_set",
            geometry: "predicted_geometry",
            score: "not_provided",
            prompts: &["text"],
            architecture: None,
        }),
        "image_classification" => Some(ScaffoldSpec {
            capabilities: &["image_classification"],
            output: "classification_set",
            geometry: "not_applicable",
            score: "relative_confidence",
            prompts: &[],
            architecture: None,
        }),
        "semantic_segmentation" => Some(ScaffoldSpec {
            capabilities: &["semantic_segmentation"],
            output: "semantic_mask",
            geometry: "predicted_geometry",
            score: "relative_confidence",
            prompts: &[],
            architecture: None,
        }),
        "prompted_segmentation" => Some(ScaffoldSpec {
            capabilities: &["prompted_segmentation"],
            output: "instance_mask",
            geometry: "mask_refined_geometry",
            score: "relative_confidence",
            prompts: &["box", "point"],
            architecture: None,
        }),
        "instance_segmentation" => Some(ScaffoldSpec {
            capabilities: &["instance_segmentation"],
            output: "instance_mask",
            geometry: "predicted_geometry",
            score: "relative_confidence",
            prompts: &[],
            architecture: None,
        }),
        "keypoint_detection" => Some(ScaffoldSpec {
            capabilities: &["keypoint_detection"],
            output: "keypoints",
            geometry: "predicted_geometry",
            score: "relative_confidence",
            prompts: &[],
            architecture: None,
        }),
        _ => None,
    }
}

fn manifest(name: &str, spec: ScaffoldSpec) -> String {
    let capabilities = spec
        .capabilities
        .iter()
        .map(|capability| format!("  - {capability}"))
        .collect::<Vec<_>>()
        .join("\n");
    let prompts = if spec.prompts.is_empty() {
        "[]".to_owned()
    } else {
        let mut prompts = String::new();
        for prompt in spec.prompts {
            write!(
                prompts,
                "\n  - kind: {prompt}\n    required: false\n    multiple: true"
            )
            .expect("writing to a String cannot fail");
        }
        prompts
    };
    let architecture = spec.architecture.unwrap_or("null");
    let availability = if spec.architecture.is_some() {
        "missing_weights"
    } else {
        "unconfigured"
    };
    format!(
        "schema_version: '1'\nmodel_id: {name}-model\ndisplay_name: {name}\narchitecture: {architecture}\nmodel_version: unconfigured\nconnection:\n  kind: vision_worker_model\n  worker_id: {name}\n  worker_model_id: {name}-model\ncapabilities:\n{capabilities}\ninput_contracts:\n  - name: image\n    data_type: {{artifact: image}}\n    required: true\n    multiple: false\noutput_contracts:\n  - name: outputs\n    data_type: {{artifact: {output}}}\n    required: true\n    multiple: true\nprompt_contracts:{prompts}\nscore_semantics: {score}\ngeometry_semantics: {geometry}\nlabel_space: null\ncheckpoint: null\nruntime_requirements:\n  devices: []\n  minimum_gpu_memory_mb: null\n  dependencies: []\n  supports_batch: false\nlicense:\n  code_license: null\n  weight_license: null\n  source_url: null\n  commercial_use: unknown\n  redistribution: unknown\n  usage_notes:\n    - Complete and verify license metadata before registration.\n  verified_from_official_source: false\navailability: {availability}\navailability_evidence:\n  health_passed: false\n  protocol_compatible: true\n  contracts_validated: true\n  sample_conversion_passed: false\n  weights_ready: false\n  checked_at: null\n  detail: Generated adapter template; no checkpoint has been configured or tested.\nmetadata:\n  generated_by: annotagent worker scaffold\n",
        output = spec.output,
        score = spec.score,
        geometry = spec.geometry,
    )
}

const APP_TEMPLATE: &str = r#"from pathlib import Path

import uvicorn

from annotagent_vision_worker import create_worker_app, load_manifest
from model import infer, warmup

MANIFEST = load_manifest(Path(__file__).with_name("manifest.yaml"))
app = create_worker_app([MANIFEST], infer, warmup=warmup)

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8790)
"#;

const MODEL_TEMPLATE: &str = r#"from threading import Event

from annotagent_vision_worker import InferenceRequest, InferenceResponse, inference_error


def warmup(model_id: str) -> bool:
    # Load only explicitly configured local weights. Never download weights implicitly.
    return False


def infer(request: InferenceRequest, cancellation: Event) -> InferenceResponse:
    if cancellation.is_set():
        return inference_error(request.request_id, request.model_id, "cancelled", "request cancelled")
    return inference_error(request.request_id, request.model_id, "weights_unavailable", "configure local weights and implement model.py")
"#;

const TEST_TEMPLATE: &str = r"from app import app
from annotagent_vision_worker import assert_app_conformance


def test_worker_contract() -> None:
    assert_app_conformance(app)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sam_preset_is_unavailable_and_contains_explicit_mask_contract() {
        let directory = tempfile::tempdir().expect("temporary worker root");
        let target = scaffold(&ScaffoldRequest {
            output_root: directory.path(),
            name: None,
            capability: None,
            preset: Some("sam2"),
            language: "python",
        })
        .expect("SAM scaffold");
        let manifest = std::fs::read_to_string(target.join("manifest.yaml")).expect("manifest");
        assert!(manifest.contains("prompted_segmentation"));
        assert!(manifest.contains("instance_mask"));
        assert!(manifest.contains("availability: missing_weights"));
        assert!(target.join("tests/test_contract.py").is_file());
    }

    #[test]
    fn scaffold_refuses_path_traversal_and_overwrite() {
        let directory = tempfile::tempdir().expect("temporary worker root");
        let invalid = scaffold(&ScaffoldRequest {
            output_root: directory.path(),
            name: Some("../escape"),
            capability: Some("object_detection"),
            preset: None,
            language: "python",
        });
        assert!(invalid.is_err());
        let request = ScaffoldRequest {
            output_root: directory.path(),
            name: Some("test-worker"),
            capability: Some("object_detection"),
            preset: None,
            language: "python",
        };
        scaffold(&request).expect("first scaffold");
        assert!(scaffold(&request).is_err());
    }
}
