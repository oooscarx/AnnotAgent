use std::path::Path;

use annotagent_core::{
    ArtifactKind, ArtifactRef, ImageArtifact, ImageId, MaskEncoding, ModelImage,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, RunId,
};
use annotagent_model_bundle::{
    ExpectedOutputSummary, ModelBundleSmokeRequest, ModelBundleSmokeTest, OutputTolerances,
    SmokeTestCheck, SmokeTestResult, SmokeTestStatus,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};

use crate::{InstalledModelBundle, ModelCatalogError};

pub struct PreparedBundleSmokeTest {
    pub definition: ModelBundleSmokeTest,
    pub request: PipelineInferenceRequest,
}

pub fn prepare_bundle_smoke_test(
    bundle: &InstalledModelBundle,
    model_id: &str,
) -> Result<PreparedBundleSmokeTest, ModelCatalogError> {
    let reference = &bundle.manifest.test_suite;
    let request = reference
        .input_artifacts
        .iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .filter_map(|path| {
            std::fs::read(bundle.content_root.join(path))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ModelBundleSmokeRequest>(&bytes).ok())
        })
        .collect::<Vec<_>>();
    if request.len() != 1 {
        return provisioning(
            "Bundle smoke suite must contain exactly one valid request JSON input".to_owned(),
        );
    }
    let request = request.into_iter().next().expect("length checked");
    if !reference
        .input_artifacts
        .iter()
        .any(|path| path == &request.image_path)
    {
        return provisioning("Smoke request image is not declared as a test input".to_owned());
    }
    let image_path = bundle.content_root.join(&request.image_path);
    let image_bytes = std::fs::read(&image_path)?;
    if image_bytes.is_empty() || image_bytes.len() > 64 * 1024 * 1024 {
        return provisioning("Smoke test image must be bounded and non-empty".to_owned());
    }
    let expected = read_json::<ExpectedOutputSummary>(
        &bundle.content_root.join(&reference.expected_summary),
        "expected summary",
    )?;
    let tolerances = read_json::<OutputTolerances>(
        &bundle.content_root.join(&reference.tolerances),
        "output tolerances",
    )?;
    validate_definition(&request, &expected, &tolerances)?;
    let image_id = ImageId::new();
    let decoded = image::load_from_memory(&image_bytes).map_err(|error| {
        ModelCatalogError::Provisioning(format!("Smoke test image cannot be decoded: {error}"))
    })?;
    let mime_type = image_mime(&image_path)?.to_owned();
    let mut input_artifacts = request
        .input_artifacts
        .clone()
        .into_iter()
        .filter(|artifact| !matches!(artifact, PipelineArtifact::Image(_)))
        .map(|mut artifact| {
            rebind_smoke_image(&mut artifact, image_id);
            artifact
        })
        .collect::<Vec<_>>();
    input_artifacts.insert(
        0,
        PipelineArtifact::Image(ImageArtifact {
            reference: ArtifactRef {
                artifact_id: format!("model-bundle-smoke-image:{image_id}"),
                source_node: "model_bundle_smoke".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: None,
            },
            image_id,
            width: decoded.width(),
            height: decoded.height(),
            mime_type: mime_type.clone(),
            blob_ref: format!(
                "model-bundle://{}/{}",
                bundle.manifest.id, request.image_path
            ),
            parent: None,
            root_region: None,
        }),
    );
    let request_id = uuid::Uuid::new_v4().to_string();
    let pipeline_request = PipelineInferenceRequest {
        protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
        request_id,
        run_id: RunId::new(),
        image_id,
        node_id: format!("model_bundle_smoke:{}", reference.test_id),
        model_id: model_id.to_owned(),
        operation: request.operation,
        image: Some(ModelImage {
            id: format!("bundle-smoke:{image_id}"),
            mime_type,
            data_base64: STANDARD.encode(image_bytes),
        }),
        input_artifacts,
        parameters: request.parameters.clone(),
        timeout_ms: request.timeout_ms,
    };
    Ok(PreparedBundleSmokeTest {
        definition: ModelBundleSmokeTest {
            test_id: reference.test_id.clone(),
            request,
            expected,
            tolerances,
        },
        request: pipeline_request,
    })
}

fn rebind_smoke_image(artifact: &mut PipelineArtifact, image_id: ImageId) {
    match artifact {
        PipelineArtifact::Image(value) => value.image_id = image_id,
        PipelineArtifact::DetectionSet(value) => value.image_id = image_id,
        PipelineArtifact::BoxPromptSet(value) => value.image_id = image_id,
        PipelineArtifact::PointPromptSet(value) => value.image_id = image_id,
        PipelineArtifact::MaskSet(value) => value.image_id = image_id,
        PipelineArtifact::SemanticMask(value) => value.image_id = image_id,
        PipelineArtifact::PolygonSet(value) => value.image_id = image_id,
        PipelineArtifact::CandidateClusterSet(value) => value.image_id = image_id,
        PipelineArtifact::CropSet(value) => value.image_id = image_id,
        PipelineArtifact::ClassificationSet(value) => value.image_id = image_id,
        PipelineArtifact::AnnotationCandidateSet(value) => value.image_id = image_id,
    }
}

pub fn evaluate_bundle_smoke_response(
    test: &ModelBundleSmokeTest,
    request: &PipelineInferenceRequest,
    response: &PipelineInferenceResponse,
    duration_ms: u64,
    started_at: DateTime<Utc>,
) -> SmokeTestResult {
    let mut checks = Vec::new();
    checks.push(check(
        "request identity",
        response.request_id.as_deref() == Some(request.request_id.as_str()),
        "response is scoped to the fixed smoke request",
    ));
    checks.push(check(
        "inference error",
        response.error.is_none(),
        "plugin returned no typed inference error",
    ));
    checks.push(check(
        "artifact contract",
        !response.artifacts.is_empty()
            && response
                .artifacts
                .iter()
                .all(|artifact| artifact.validate().is_ok()),
        "all output artifacts are non-empty, finite and Core-valid",
    ));
    let kinds = response
        .artifacts
        .iter()
        .map(PipelineArtifact::artifact_type)
        .collect::<std::collections::BTreeSet<_>>();
    checks.push(check(
        "required artifact kinds",
        test.expected
            .required_artifact_kinds
            .iter()
            .all(|kind| kinds.contains(kind)),
        "output contains every Bundle-declared Artifact kind",
    ));
    checks.push(check(
        "artifact count",
        response.artifacts.len() >= test.expected.minimum_artifact_count,
        "output artifact count meets the declared minimum",
    ));
    let item_count: usize = response.artifacts.iter().map(artifact_item_count).sum();
    checks.push(check(
        "item count",
        item_count >= test.expected.minimum_item_count,
        "output item count meets the declared minimum",
    ));
    let coverages = response
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::MaskSet(set) => Some(
                set.masks
                    .iter()
                    .filter_map(|item| mask_coverage(&item.mask))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    if test.expected.require_non_empty_mask {
        checks.push(check(
            "non-empty mask",
            coverages.iter().any(|coverage| *coverage > 0.0),
            "at least one prompted mask has positive area",
        ));
    }
    if let Some(minimum) = test.tolerances.minimum_mask_coverage {
        checks.push(check(
            "minimum mask coverage",
            coverages.iter().any(|coverage| *coverage >= minimum),
            "at least one mask meets the broad minimum coverage tolerance",
        ));
    }
    if let Some(maximum) = test.tolerances.maximum_mask_coverage {
        checks.push(check(
            "maximum mask coverage",
            !coverages.is_empty() && coverages.iter().all(|coverage| *coverage <= maximum),
            "every mask remains within the broad maximum coverage tolerance",
        ));
    }
    checks.push(check(
        "duration tolerance",
        duration_ms <= test.tolerances.maximum_duration_ms,
        "wall-clock inference duration is within the Bundle tolerance",
    ));
    let passed = checks.iter().all(|item| item.passed);
    SmokeTestResult {
        test_id: test.test_id.clone(),
        status: if passed {
            SmokeTestStatus::Passed
        } else {
            SmokeTestStatus::Failed
        },
        checks,
        duration_ms,
        started_at,
        finished_at: Utc::now(),
    }
}

fn validate_definition(
    request: &ModelBundleSmokeRequest,
    expected: &ExpectedOutputSummary,
    tolerances: &OutputTolerances,
) -> Result<(), ModelCatalogError> {
    if expected.required_artifact_kinds.is_empty()
        || expected.minimum_artifact_count == 0
        || expected.minimum_item_count == 0
        || tolerances.maximum_duration_ms == 0
        || request.timeout_ms.is_some_and(|timeout| timeout == 0)
    {
        return provisioning("Smoke expectation and tolerance bounds must be positive".to_owned());
    }
    for coverage in [
        tolerances.minimum_mask_coverage,
        tolerances.maximum_mask_coverage,
    ]
    .into_iter()
    .flatten()
    {
        if !coverage.is_finite() || !(0.0..=1.0).contains(&coverage) {
            return provisioning("Mask coverage tolerance must be finite in [0, 1]".to_owned());
        }
    }
    if tolerances
        .minimum_mask_coverage
        .zip(tolerances.maximum_mask_coverage)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return provisioning("Mask coverage tolerance range is inverted".to_owned());
    }
    Ok(())
}

fn artifact_item_count(artifact: &PipelineArtifact) -> usize {
    match artifact {
        PipelineArtifact::Image(_) | PipelineArtifact::SemanticMask(_) => 1,
        PipelineArtifact::DetectionSet(value) => value.detections.len(),
        PipelineArtifact::BoxPromptSet(value) => value.prompts.len(),
        PipelineArtifact::PointPromptSet(value) => value.prompts.len(),
        PipelineArtifact::MaskSet(value) => value.masks.len(),
        PipelineArtifact::PolygonSet(value) => value.polygons.len(),
        PipelineArtifact::CandidateClusterSet(value) => value.candidates.len(),
        PipelineArtifact::CropSet(value) => value.crops.len(),
        PipelineArtifact::ClassificationSet(value) => value.classifications.len(),
        PipelineArtifact::AnnotationCandidateSet(value) => value.candidates.len(),
    }
}

fn mask_coverage(mask: &MaskEncoding) -> Option<f64> {
    match mask {
        MaskEncoding::Polygon { rings } => {
            let area = rings
                .iter()
                .filter(|ring| ring.len() >= 3)
                .map(|ring| {
                    ring.iter()
                        .zip(ring.iter().cycle().skip(1))
                        .take(ring.len())
                        .map(|(left, right)| {
                            f64::from(left.x()) * f64::from(right.y())
                                - f64::from(right.x()) * f64::from(left.y())
                        })
                        .sum::<f64>()
                        .abs()
                        / 2.0
                })
                .sum::<f64>();
            area.is_finite().then_some(area.clamp(0.0, 1.0))
        }
        MaskEncoding::CocoRle {
            width,
            height,
            counts,
        } => {
            let runs = counts
                .split_ascii_whitespace()
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            let foreground = runs
                .iter()
                .enumerate()
                .filter(|(index, _)| index % 2 == 1)
                .map(|(_, count)| *count)
                .sum::<u64>();
            let pixels = u64::from(*width).saturating_mul(u64::from(*height));
            (pixels > 0).then_some(foreground as f64 / pixels as f64)
        }
    }
}

fn image_mime(path: &Path) -> Result<&'static str, ModelCatalogError> {
    match path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        _ => provisioning("Smoke test image must be PNG or JPEG".to_owned()),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<T, ModelCatalogError> {
    serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|error| ModelCatalogError::Provisioning(format!("invalid {label}: {error}")))
}

fn check(name: &str, passed: bool, detail: &str) -> SmokeTestCheck {
    SmokeTestCheck {
        name: name.to_owned(),
        passed,
        detail: detail.to_owned(),
    }
}

fn provisioning<T>(message: String) -> Result<T, ModelCatalogError> {
    Err(ModelCatalogError::Provisioning(message))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{
        ArtifactKind, ArtifactRef, ArtifactValidationState, MaskArtifactItem, MaskSetArtifact,
        PipelineInferenceResponse,
    };

    use super::*;

    #[test]
    fn smoke_tolerances_reject_empty_and_out_of_range_masks() {
        let image_id = ImageId::new();
        let request = PipelineInferenceRequest {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: "smoke".to_owned(),
            run_id: RunId::new(),
            image_id,
            node_id: "smoke".to_owned(),
            model_id: "fixture".to_owned(),
            operation: annotagent_core::VisionCapability::PromptedSegmentation,
            image: None,
            input_artifacts: Vec::new(),
            parameters: BTreeMap::new(),
            timeout_ms: Some(1_000),
        };
        let test = ModelBundleSmokeTest {
            test_id: "fixture".to_owned(),
            request: ModelBundleSmokeRequest {
                image_path: "tests/input.png".to_owned(),
                operation: annotagent_core::VisionCapability::PromptedSegmentation,
                input_artifacts: Vec::new(),
                parameters: BTreeMap::new(),
                timeout_ms: Some(1_000),
            },
            expected: ExpectedOutputSummary {
                required_artifact_kinds: std::collections::BTreeSet::from([ArtifactKind::MaskSet]),
                minimum_artifact_count: 1,
                minimum_item_count: 1,
                require_non_empty_mask: true,
            },
            tolerances: OutputTolerances {
                maximum_duration_ms: 1_000,
                minimum_mask_coverage: Some(0.1),
                maximum_mask_coverage: Some(0.8),
            },
        };
        let response = |counts: &str| PipelineInferenceResponse {
            request_id: Some("smoke".to_owned()),
            model_identity: Some("fixture".to_owned()),
            artifacts: vec![PipelineArtifact::MaskSet(MaskSetArtifact {
                reference: ArtifactRef {
                    artifact_id: "masks".to_owned(),
                    source_node: "smoke".to_owned(),
                    port: "masks".to_owned(),
                    artifact_type: ArtifactKind::MaskSet,
                    item_id: None,
                },
                image_id,
                model_binding: "fixture".to_owned(),
                source_prompts: ArtifactRef {
                    artifact_id: "prompts".to_owned(),
                    source_node: "smoke".to_owned(),
                    port: "prompts".to_owned(),
                    artifact_type: ArtifactKind::BoxPromptSet,
                    item_id: None,
                },
                validation_state: ArtifactValidationState::Unvalidated,
                masks: vec![MaskArtifactItem {
                    mask_id: "mask".to_owned(),
                    prompt: ArtifactRef {
                        artifact_id: "prompts".to_owned(),
                        source_node: "smoke".to_owned(),
                        port: "prompts".to_owned(),
                        artifact_type: ArtifactKind::BoxPromptSet,
                        item_id: Some("box".to_owned()),
                    },
                    mask: MaskEncoding::CocoRle {
                        width: 10,
                        height: 10,
                        counts: counts.to_owned(),
                    },
                    score: annotagent_core::DetectionScore::relative(0.9).expect("score"),
                    attributes: BTreeMap::new(),
                }],
                metadata: BTreeMap::new(),
            })],
            ..PipelineInferenceResponse::default()
        };
        assert_eq!(
            evaluate_bundle_smoke_response(&test, &request, &response("20 30 50"), 10, Utc::now())
                .status,
            SmokeTestStatus::Passed
        );
        assert_eq!(
            evaluate_bundle_smoke_response(&test, &request, &response("100 0"), 10, Utc::now())
                .status,
            SmokeTestStatus::Failed
        );
    }
}
