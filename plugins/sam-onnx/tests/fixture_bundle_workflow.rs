use std::{collections::BTreeMap, path::PathBuf};

use annotagent_core::{
    ArtifactKind, ImageId, NodePort, PipelineArtifact, ProjectId, RunId, WorkflowDraftNode,
    WorkflowNodeKind,
};
use annotagent_model_bundle::ModelInstanceStatus;
use annotagent_model_catalog::{
    BindModelInstanceRequest, ModelBundleRegistry, build_builtin_fixture_catalog,
    evaluate_bundle_smoke_response, prepare_bundle_smoke_test,
};
use annotagent_plugin_api::{PluginManifest, PluginRuntimeStatus, Sha256Digest};
use annotagent_plugin_host::{PluginProcessConfig, process_directories};
use annotagent_plugin_registry::run_model_instance_smoke;
use annotagent_runtime::{
    CORE_GEOMETRY_QUALITY_EVALUATION, CORE_MASK_TO_BBOX, CorePipelineRunner, DagNodeContext,
    DagNodeRunner as _,
};
use chrono::Utc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn offline_fixture_runs_bundle_plugin_smoke_geometry_and_removal_lifecycle() {
    let temporary = tempdir().expect("tempdir");
    let fixture = build_builtin_fixture_catalog(&temporary.path().join("catalog"))
        .expect("build fixture Bundle");
    let registry_root = temporary.path().join("registry");
    let mut registry = ModelBundleRegistry::open(&registry_root).expect("registry");
    let installed = registry
        .import_local(&fixture.package_path)
        .expect("install verified fixture");
    let manifest =
        PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
    let instance = registry
        .bind_model_instance(BindModelInstanceRequest {
            plugin: &manifest,
            plugin_package_digest: Sha256Digest::of_bytes(b"fixture-plugin-package"),
            runtime_status: PluginRuntimeStatus::Ready,
            bundle_id: &installed.manifest.id,
            bundle_version: &installed.manifest.version,
            model_id: "sam-vit-b-onnx",
            target: current_target(),
            execution_provider: "cpu",
        })
        .expect("bind fixture Model Instance");
    assert_eq!(instance.status, ModelInstanceStatus::Preparing);
    assert!(instance.contract_inspection.valid);

    let prepared = prepare_bundle_smoke_test(&installed, "sam-vit-b-onnx")
        .expect("prepare fixed smoke vector");
    assert!(matches!(
        prepared.request.input_artifacts.first(),
        Some(PipelineArtifact::Image(_))
    ));
    assert!(
        prepared
            .request
            .input_artifacts
            .iter()
            .all(|artifact| artifact.image_id() == prepared.request.image_id)
    );

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_annotagent-plugin-sam-onnx"));
    let (state_dir, cache_dir, temporary_dir) =
        process_directories(&temporary.path().join("plugin-process"));
    let model_files = installed
        .manifest
        .files
        .iter()
        .map(|file| {
            (
                file.role.as_str().to_owned(),
                installed.content_root.join(&file.path),
            )
        })
        .collect();
    let started_at = Utc::now();
    let started = std::time::Instant::now();
    let report = run_model_instance_smoke(
        manifest,
        PluginProcessConfig {
            installation_root: executable.parent().expect("target directory").to_path_buf(),
            executable,
            state_dir,
            weights_dir: installed.content_root.clone(),
            model_files,
            cache_dir,
            temporary_dir,
            max_request_bytes: 64 * 1024 * 1024,
            max_response_bytes: 256 * 1024 * 1024,
        },
        &prepared.request,
    )
    .await
    .expect("real Rust Plugin smoke process");
    assert!(report.conformance.passed, "{:?}", report.conformance.checks);
    let smoke = evaluate_bundle_smoke_response(
        &prepared.definition,
        &prepared.request,
        &report.response,
        started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        started_at,
    );
    assert_eq!(
        smoke.status,
        annotagent_model_bundle::SmokeTestStatus::Passed,
        "{:?}",
        smoke.checks
    );
    let ready = registry
        .record_model_instance_smoke(instance.id, smoke)
        .expect("record smoke");
    assert_eq!(ready.status, ModelInstanceStatus::Ready);
    let profile = registry
        .model_profiles()
        .into_iter()
        .find(|profile| profile.model_instance_id == ready.id)
        .expect("fixture profile");
    assert!(!profile.selectable, "Fixture must not be publishable");
    assert_eq!(
        profile.availability,
        annotagent_core::ModelAvailability::Unknown
    );

    let prompt = prepared
        .request
        .input_artifacts
        .iter()
        .find(|artifact| matches!(artifact, PipelineArtifact::BoxPromptSet(_)))
        .expect("box prompt")
        .clone();
    let source_detections = prepared
        .request
        .input_artifacts
        .iter()
        .find(|artifact| matches!(artifact, PipelineArtifact::DetectionSet(_)))
        .expect("source detections")
        .clone();
    let masks = report.response.artifacts[0].clone();
    let run_id = RunId::new();
    let bbox_node = node(
        "mask-to-bbox",
        CORE_MASK_TO_BBOX,
        WorkflowNodeKind::Transform,
        ArtifactKind::DetectionSet,
        "detections",
    );
    let refined = CorePipelineRunner
        .run(context(
            run_id,
            prepared.request.image_id,
            &bbox_node,
            vec![source_detections, prompt, masks],
        ))
        .await
        .expect("MaskSet to refined DetectionSet");
    let quality_node = node(
        "geometry-quality",
        CORE_GEOMETRY_QUALITY_EVALUATION,
        WorkflowNodeKind::Validator,
        ArtifactKind::DetectionSet,
        "detections",
    );
    let evaluated = CorePipelineRunner
        .run(context(
            run_id,
            prepared.request.image_id,
            &quality_node,
            refined.pipeline_artifacts,
        ))
        .await
        .expect("geometry safety evaluation");
    assert_eq!(evaluated.metadata["semantic_score_used"], false);

    registry
        .disable(&installed.manifest.id, &installed.manifest.version)
        .expect("disable fixture");
    registry
        .remove(&installed.manifest.id, &installed.manifest.version)
        .expect("remove unreferenced fixture");
    assert!(
        registry
            .get(&installed.manifest.id, &installed.manifest.version)
            .is_none()
    );
}

fn current_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macos-aarch64",
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("windows", "x86_64") => "windows-x86_64",
        target => panic!("unsupported test target {target:?}"),
    }
}

fn node(
    id: &str,
    node_type: &str,
    kind: WorkflowNodeKind,
    artifact_type: ArtifactKind,
    port: &str,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        outputs: vec![NodePort {
            id: port.to_owned(),
            artifact_type,
            required: true,
            multiple: false,
        }],
        ..WorkflowDraftNode::default()
    }
}

fn context(
    run_id: RunId,
    image_id: ImageId,
    node: &WorkflowDraftNode,
    artifacts: Vec<PipelineArtifact>,
) -> DagNodeContext<'_> {
    DagNodeContext {
        project_id: ProjectId::new(),
        run_id,
        image_id,
        node,
        input_artifacts: Vec::new(),
        input_pipeline_artifacts: artifacts,
        input_metadata: BTreeMap::new(),
        cancellation: CancellationToken::new(),
    }
}
