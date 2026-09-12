//! Opt-in, test-binary-only seeds for browser-visible P0 failure evidence.
//! The production router has no seeding route; clients read these records through
//! the ordinary owned Task workspace endpoint.

use annotagent_application::{LocalApplication, ProjectImageSummary, stable_project_id};
use annotagent_core::{
    AnnotationFailureClass, ModelAvailability, ModelCapability, ModelFailure, ModelFailureCategory,
    ModelFailureStage, WorkflowDraft, WorkflowDraftStatus, WorkflowDryRunReport,
};
use annotagent_plugin_registry::{InstallApproval, PluginRegistryError};
use annotagent_storage::{
    BeginConversationTask, ConversationCallAdmission, ConversationCallGrant,
    ConversationCallStatus, ConversationMessageInput, SampleOperation, WorkflowSampleTest,
    WorkflowSampleTestInput, WorkflowSampleTestStatus,
};
use anyhow::{Context, Result, ensure};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
use uuid::Uuid;

const PROJECT: &str = "TEST-p0-result-diagnostics";
const MANIFEST: &str = "P0_DIAGNOSTIC_SCENES.json";

fn fixture_uuid(scene: &str, kind: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("annotagent://TEST/p0-diagnostic/{scene}/{kind}").as_bytes(),
    )
}

fn install_missing_weights_plugin(application: &LocalApplication) -> Result<()> {
    let registry = application.plugin_registry();
    if registry
        .lock()
        .map_err(|_| anyhow::anyhow!("Plugin Registry lock poisoned"))?
        .ready_models()
        .iter()
        .any(|profile| {
            profile.availability == ModelAvailability::MissingWeights
                && profile
                    .capabilities
                    .contains(&ModelCapability::ObjectDetection)
        })
    {
        return Ok(());
    }

    let source = application
        .workspace()
        .join("TEST-p0-missing-weights-plugin-source");
    let binary = source
        .join("bin")
        .join(annotagent_plugin_host::current_target())
        .join("annotagent-plugin-p0-missing-weights");
    fs::create_dir_all(binary.parent().context("TEST Plugin binary parent")?)?;
    fs::write(&binary, b"TEST fixture binary; never executed")?;
    let manifest = format!(
        r#"schema_version = "1"
id = "org.annotagent.p0-missing-weights"
version = "1.0.0"
display_name = "TEST P0 Missing Weights Detector"
description = "Fixture-only unavailable detector"
publisher = "AnnotAgent TEST"
plugin_api = "1"

[runtime]
kind = "native_rust_process"
entrypoint = "bin/{{target}}/annotagent-plugin-p0-missing-weights"
protocol = "http-vision-v1"
startup_timeout_seconds = 5
shutdown_timeout_seconds = 5

[compatibility]
annotagent = ">=0.1.0,<0.2.0"
targets = ["{}"]
accelerators = ["cpu"]

[permissions]
network = "loopback_only"
provider_secrets = false
project_files = false
temporary_images = true
plugin_cache = true
subprocesses = false

[resources]
minimum_memory_mb = 64
recommended_memory_mb = 128
minimum_vram_mb = 0
recommended_vram_mb = 0
maximum_response_mb = 8
maximum_concurrency = 1
maximum_requests_per_process = 1

[[models]]
id = "p0-missing-weights-detector"
display_name = "TEST P0 Missing Weights Detector"
capabilities = ["object_detection"]
input_contracts = [{{ name = "image", data_type = {{ artifact = "image" }}, required = true, multiple = false }}]
output_contracts = [{{ name = "detections", data_type = {{ artifact = "detection_set" }}, required = true, multiple = false }}]
required_file_roles = ["checkpoint"]
score_semantics = "detection_confidence"
geometry_semantics = "predicted_geometry"

[models.runtime_requirements]
devices = ["cpu"]
supports_batch = false

[weights]
bundled = false
required = true
provisioning = "local_path"
checkpoint_sha256_required = true
components = [{{ id = "checkpoint", model_id = "p0-missing-weights-detector", filename = "model.bin" }}]

[license]
code = "MIT"
weights = "TEST-only absent weights"
commercial_use = "unknown"
"#,
        annotagent_plugin_host::current_target()
    );
    fs::write(
        source.join(annotagent_plugin_api::PLUGIN_MANIFEST_FILE),
        manifest,
    )?;
    let package = application
        .workspace()
        .join("TEST-p0-missing-weights.annotplugin");
    annotagent_plugin_host::pack_directory(&source, &package)?;
    let installed = registry
        .lock()
        .map_err(|_| anyhow::anyhow!("Plugin Registry lock poisoned"))?
        .install(
            &package,
            &InstallApproval {
                permissions_reviewed: true,
                code_license_accepted: true,
                weight_license_accepted: true,
            },
        );
    match installed {
        Ok(_) | Err(PluginRegistryError::AlreadyInstalled) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn create_task(
    application: &LocalApplication,
    conversation: Uuid,
    revision: &str,
    scene: &str,
) -> Result<Uuid> {
    let message = ConversationMessageInput {
        id: fixture_uuid(scene, "message"),
        text: format!("TEST diagnostic scene: {scene}"),
        image: None,
        reference: None,
    };
    application.append_project_conversation_message(PROJECT, conversation, &message)?;
    let task = fixture_uuid(scene, "task");
    application.begin_conversation_task(
        PROJECT,
        conversation,
        &BeginConversationTask {
            id: task,
            source_message_id: message.id,
            schema_revision: revision.to_owned(),
        },
    )?;
    Ok(task)
}

fn save_call(
    application: &LocalApplication,
    owner: &str,
    task: Uuid,
    scene: &str,
    status: ConversationCallStatus,
    failure: Option<ModelFailure>,
    maximum_calls: u32,
) -> Result<()> {
    let failure_stage = failure.as_ref().map(|item| item.stage);
    let scope_hash = annotagent_image_tools::sha256(format!("TEST scope {scene}").as_bytes());
    let request_hash = annotagent_image_tools::sha256(format!("TEST request {scene}").as_bytes());
    application.store().authorize_conversation_calls(
        owner,
        &ConversationCallGrant {
            id: fixture_uuid(scene, "grant"),
            task_id: task,
            scope_hash: scope_hash.clone(),
            maximum_calls,
            expires_at: Utc::now() + Duration::hours(1),
        },
    )?;
    let call = fixture_uuid(scene, "call");
    ensure!(
        matches!(
            application.store().reserve_conversation_call(
                owner,
                task,
                call,
                &scope_hash,
                &request_hash,
            )?,
            ConversationCallAdmission::Admitted
        ),
        "new TEST call was not admitted"
    );
    if matches!(
        failure_stage,
        Some(ModelFailureStage::ProviderRequest | ModelFailureStage::ResponseBody)
    ) {
        application
            .store()
            .mark_conversation_call_stage(owner, task, call, "provider_request")?;
    } else if matches!(failure_stage, Some(ModelFailureStage::StructuredOutput)) {
        application
            .store()
            .mark_conversation_call_stage(owner, task, call, "provider_request")?;
        application
            .store()
            .mark_conversation_call_stage(owner, task, call, "response_received")?;
    }
    let failure = failure.map(serde_json::to_value).transpose()?;
    application.store().finish_conversation_call(
        owner,
        task,
        call,
        status,
        json!({"fixture":"TEST deterministic external outcome","failure":failure}),
    )?;
    Ok(())
}

fn save_sample(
    application: &LocalApplication,
    conversation: Uuid,
    task: Uuid,
    scene: &str,
    mut sample: Value,
    image: &ProjectImageSummary,
) -> Result<(String, String)> {
    let id = fixture_uuid(scene, "sample").to_string();
    let now = Utc::now();
    let draft_id = fixture_uuid(scene, "draft").to_string();
    application.store().save_workflow_draft(&WorkflowDraft {
        annotation_schema: None,
        schema_version: 2,
        id: draft_id.clone(),
        project_id: PROJECT.to_owned(),
        name: format!("TEST diagnostic Draft: {scene}"),
        status: WorkflowDraftStatus::Editing,
        revision: 1,
        content_hash: String::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        enabled_skills: BTreeMap::new(),
        resource_versions: BTreeMap::new(),
        runtime_policies: BTreeMap::new(),
        allow_unvalidated_commit: false,
        geometry_risk_acceptance: None,
        label_pipeline: None,
        created_at: now,
        updated_at: now,
    })?;
    let draft = application.store().get_workflow_draft(&draft_id)?;
    let operation = SampleOperation {
        id: id.clone(),
        project_id: PROJECT.to_owned(),
        draft_id: draft.id.clone(),
        authorization_fingerprint: annotagent_image_tools::sha256(scene.as_bytes()),
        request: json!({"conversation":{"conversation_id":conversation,"task_id":task}}),
        status: "queued".to_owned(),
        error: None,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    ensure!(application.store().reserve_sample_operation(&operation)?);
    ensure!(application.store().start_sample_operation(&id)?);
    sample["image_name"] = json!(image.name);
    sample["width"] = json!(160);
    sample["height"] = json!(100);
    let samples = serde_json::to_value(vec![sample])?;
    let report: WorkflowDryRunReport = serde_json::from_value(json!({
        "sandbox":true,
        "validation":{"valid":true,"issues":[],"execution_order":[]},
        "samples":samples,
        "total_latency_ms":0,
        "estimated_cost":"0"
    }))?;
    let inputs = vec![WorkflowSampleTestInput {
        image_id: image.image_id.to_string(),
        content_hash: image.content_hash.clone(),
    }];
    application
        .store()
        .save_workflow_sample_test(&WorkflowSampleTest {
            id: id.clone(),
            draft_id: draft.id.clone(),
            project_id: PROJECT.to_owned(),
            draft_revision: draft.revision,
            request_revision: draft.revision,
            draft_content_hash: draft.content_hash,
            image_set_hash: annotagent_image_tools::sha256(&serde_json::to_vec(&inputs)?),
            model_snapshot_hash: annotagent_image_tools::sha256(&serde_json::to_vec(&BTreeMap::<
                String,
                String,
            >::new(
            ))?),
            status: if report.samples[0].failed {
                WorkflowSampleTestStatus::Failed
            } else {
                WorkflowSampleTestStatus::Passed
            },
            inputs,
            model_bindings: BTreeMap::new(),
            report,
            started_at: now,
            completed_at: now,
        })?;
    application.store().finish_sample_operation(&id, None)?;
    Ok((draft.id, id))
}

fn scene_entry(conversation: Uuid, task: Uuid, code: &str) -> Value {
    let task_root = format!("/api/projects/{PROJECT}/conversations/{conversation}/tasks/{task}");
    let state = if code == "legal_empty_detection" {
        "completed"
    } else {
        "blocked"
    };
    json!({
        "project_id":PROJECT,"conversation_id":conversation,"task_id":task,
        "task_url":format!("/projects/{PROJECT}/work?task={task}"),
        "workspace_url":format!("{task_root}/workspace"),
        "expected":{"code":code,"state":state,"automatic_retry":false}
    })
}

pub(crate) fn seed(application: &LocalApplication) -> Result<Value> {
    ensure!(
        application.workspace().join("FIXTURE_ONLY").is_file(),
        "P0 diagnostics require the explicit TEST workspace marker"
    );
    let manifest_path = application.workspace().join(MANIFEST);
    if manifest_path.is_file() {
        return Ok(serde_json::from_slice(&fs::read(manifest_path)?)?);
    }
    ensure!(
        !application.workspace().join(PROJECT).exists(),
        "partial P0 diagnostic seed exists without its manifest"
    );
    application.create_project(
        PROJECT,
        "version: 1\nproject:\n  name: TEST P0 result diagnostics\n  annotation_goal: Test browser-visible safe failure evidence\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
    )?;
    let image_path = application
        .workspace()
        .join(PROJECT)
        .join("images")
        .join("TEST-diagnostic-input.png");
    annotagent_image_tools::generate_synthetic_inspection(&image_path)?;
    let images = application.list_project_image_summaries(PROJECT)?;
    ensure!(
        images.len() == 1,
        "diagnostic Project must contain one image"
    );
    let image = &images[0];
    install_missing_weights_plugin(application)?;
    let conversation = application.create_project_conversation(PROJECT)?;
    let revision = application.project_goal(PROJECT)?["revision"]
        .as_str()
        .context("TEST Project revision")?
        .to_owned();
    let owner = stable_project_id(&application.workspace().join(PROJECT)).to_string();
    let scenes = [
        "model_weights_missing",
        "provider_request_not_sent",
        "provider_outcome_unknown",
        "model_response_invalid_structure",
        "legal_empty_detection",
        "candidate_projection_failed",
        "authorization_expired",
        "task_call_budget_exhausted",
    ];
    let tasks = scenes
        .iter()
        .map(|scene| {
            Ok((
                *scene,
                create_task(application, conversation, &revision, scene)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    save_call(
        application,
        &owner,
        tasks["provider_request_not_sent"],
        "provider_request_not_sent",
        ConversationCallStatus::Failed,
        Some(ModelFailure {
            stage: ModelFailureStage::PrepareRequest,
            category: ModelFailureCategory::Configuration,
            http_status: None,
        }),
        2,
    )?;
    save_call(
        application,
        &owner,
        tasks["provider_outcome_unknown"],
        "provider_outcome_unknown",
        ConversationCallStatus::InDoubt,
        Some(ModelFailure {
            stage: ModelFailureStage::ProviderRequest,
            category: ModelFailureCategory::Interrupted,
            http_status: None,
        }),
        2,
    )?;
    save_call(
        application,
        &owner,
        tasks["model_response_invalid_structure"],
        "model_response_invalid_structure",
        ConversationCallStatus::Failed,
        Some(ModelFailure {
            stage: ModelFailureStage::StructuredOutput,
            category: ModelFailureCategory::InvalidStructuredOutput,
            http_status: None,
        }),
        2,
    )?;
    let legal_empty = save_sample(
        application,
        conversation,
        tasks["legal_empty_detection"],
        "legal_empty_detection",
        json!({
            "image_index":0,"image_name":"TEST-legal-empty.png","width":160,"height":100,
            "result_count":0,"auto_accepted_count":0,"review_count":0,
            "failed":false,"empty":true,"outcomes":[],"nodes":[],
            "failure_classes":[AnnotationFailureClass::NoCandidate],
            "projection":{"final_candidates":[],"review_candidates":[],"committed_annotations":[],
                "no_target":true,"intermediate_artifact_ids":[]}
        }),
        image,
    )?;
    let projection_failed = save_sample(
        application,
        conversation,
        tasks["candidate_projection_failed"],
        "candidate_projection_failed",
        json!({
            "image_index":0,"image_name":"TEST-projection-failed.png","width":160,"height":100,
            "result_count":0,"auto_accepted_count":0,"review_count":0,
            "failed":true,"empty":false,"outcomes":[],"nodes":[],
            "failure_classes":[AnnotationFailureClass::InvalidArtifact],
            "projection":{"final_candidates":[],"review_candidates":[],"committed_annotations":[],
                "no_target":false,"intermediate_artifact_ids":[]}
        }),
        image,
    )?;

    let expired_at = Utc::now() + Duration::milliseconds(100);
    application.store().authorize_conversation_calls(
        &owner,
        &ConversationCallGrant {
            id: fixture_uuid("authorization_expired", "grant"),
            task_id: tasks["authorization_expired"],
            scope_hash: annotagent_image_tools::sha256(b"TEST expired scope"),
            maximum_calls: 1,
            expires_at: expired_at,
        },
    )?;
    save_call(
        application,
        &owner,
        tasks["task_call_budget_exhausted"],
        "task_call_budget_exhausted",
        ConversationCallStatus::Completed,
        None,
        1,
    )?;
    while Utc::now() <= expired_at {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let mut scene_manifest = tasks
        .iter()
        .map(|(code, task)| ((*code).to_owned(), scene_entry(conversation, *task, code)))
        .collect::<serde_json::Map<_, _>>();
    for (code, (draft_id, sample_test_id)) in [
        ("legal_empty_detection", legal_empty),
        ("candidate_projection_failed", projection_failed),
    ] {
        scene_manifest[code]["draft_id"] = json!(draft_id);
        scene_manifest[code]["draft_url"] = json!(format!(
            "/api/workflow-drafts/{draft_id}?project_id={PROJECT}"
        ));
        scene_manifest[code]["sample_test_id"] = json!(sample_test_id);
        scene_manifest[code]["sample_test_url"] = json!(format!(
            "/api/workflow-drafts/{draft_id}/sample-test?test_id={sample_test_id}"
        ));
        scene_manifest[code]["image"] = json!({
            "image_id":image.image_id,"content_hash":image.content_hash,
            "name":image.name,"width":160,"height":100
        });
    }
    let manifest = json!({
        "contract_version":"p0-diagnostic-scenes-v1",
        "fixture":"TEST external-model-only plus deterministic diagnostic records",
        "project_id":PROJECT,"conversation_id":conversation,
        "selection":{"manifest_file":MANIFEST,"scene_key":"diagnostic_scenes.scenes.<code>"},
        "scenes":scene_manifest
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_core::ProviderAdapterKind;
    use annotagent_provider::InMemorySecretStore;
    use axum::http::Method;
    use std::sync::Arc;

    #[test]
    fn seed_is_idempotent_and_persists_eight_real_read_model_scenes() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("FIXTURE_ONLY"),
            "AnnotAgent HTTP integration fixture\n",
        )
        .unwrap();
        let app = LocalApplication::new(temp.path()).unwrap();
        let first = seed(&app).unwrap();
        let second = seed(&app).unwrap();
        assert_eq!(first, second);
        assert_eq!(first["scenes"].as_object().unwrap().len(), 8);
        for (code, scene) in first["scenes"].as_object().unwrap() {
            let task = Uuid::parse_str(scene["task_id"].as_str().unwrap()).unwrap();
            let workspace = app
                .mainline_task_read_model(PROJECT, conversation(&first), task)
                .unwrap();
            if code != "model_weights_missing" {
                assert!(
                    workspace["result_diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|diagnostic| diagnostic["code"] == *code),
                    "missing {code}: {}",
                    workspace["result_diagnostics"]
                );
            }
        }
    }

    #[tokio::test]
    async fn every_scene_is_visible_through_the_owned_production_workspace_route() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("FIXTURE_ONLY"),
            "AnnotAgent HTTP integration fixture\n",
        )
        .unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let manifest = seed(&app).unwrap();
        let state = test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await;
        app.store()
            .purge_provider_adapter(ProviderAdapterKind::Mock)
            .unwrap();
        app.store().purge_mock_agent_sessions().unwrap();
        let service = crate::router(state, None);
        for (code, scene) in manifest["scenes"].as_object().unwrap() {
            let workspace = response_json(
                request(
                    &service,
                    Method::GET,
                    scene["workspace_url"].as_str().unwrap(),
                    None,
                )
                .await,
            )
            .await;
            let matching = workspace["mainline"]["result_diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|diagnostic| diagnostic["code"] == *code)
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1, "missing {code}");
            assert_eq!(matching[0]["automatic_retry"], false);
            assert_eq!(matching[0]["preserves_existing_results"], true);
            assert_eq!(matching[0]["safe_action"]["method"], "GET");
            if let Some(draft_url) = scene["draft_url"].as_str() {
                let draft =
                    response_json(request(&service, Method::GET, draft_url, None).await).await;
                assert_eq!(draft["id"], scene["draft_id"]);
                assert_eq!(draft["project_id"], PROJECT);
                let sample = response_json(
                    request(
                        &service,
                        Method::GET,
                        scene["sample_test_url"].as_str().unwrap(),
                        None,
                    )
                    .await,
                )
                .await;
                assert_eq!(sample["sample_test"]["id"], scene["sample_test_id"]);
                assert_eq!(sample["sample_test"]["draft_id"], scene["draft_id"]);
                assert_eq!(sample["current"], true);
                assert_eq!(sample["sample_test"]["inputs"].as_array().unwrap().len(), 1);
                assert_eq!(
                    sample["sample_test"]["report"]["samples"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(
                    sample["sample_test"]["inputs"][0]["image_id"],
                    scene["image"]["image_id"]
                );
                assert_eq!(
                    sample["sample_test"]["inputs"][0]["content_hash"],
                    scene["image"]["content_hash"]
                );
                assert_eq!(
                    sample["sample_test"]["report"]["samples"][0]["image_name"],
                    scene["image"]["name"]
                );
            }
        }
    }

    fn conversation(manifest: &Value) -> Uuid {
        Uuid::parse_str(manifest["conversation_id"].as_str().unwrap()).unwrap()
    }
}
