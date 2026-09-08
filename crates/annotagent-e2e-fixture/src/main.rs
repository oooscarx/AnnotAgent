//! Rust-only deterministic HTTP fixture used by browser release tests.
//!
//! This binary proves Provider and legacy HTTP Vision protocol integration. It contains no model,
//! checkpoint or quality claim and is never registered as an installable Expert Model plugin.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};

const WORKER_ID: &str = "annotagent-e2e-contract-fixture";
const MODEL_IDS: [&str; 5] = [
    "sam2.1-hiera-tiny",
    "yolo-specialist",
    "rfdetr-specialist",
    "locate-anything",
    "e2e-generic-detector",
];

#[derive(Clone, Copy)]
struct FixtureState;

fn checked_at() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn checkpoint(model_id: &str) -> String {
    let byte = match model_id {
        "sam2.1-hiera-tiny" => 'a',
        "yolo-specialist" => 'b',
        "rfdetr-specialist" => 'c',
        "locate-anything" => 'd',
        _ => 'e',
    };
    std::iter::repeat_n(byte, 64).collect()
}

fn capabilities(model_id: &str) -> Vec<&'static str> {
    match model_id {
        "sam2.1-hiera-tiny" => vec!["prompted_segmentation"],
        "locate-anything" => vec!["open_vocabulary_detection", "phrase_grounding"],
        _ => vec!["object_detection"],
    }
}

fn model_summary(model_id: &str) -> Value {
    json!({
        "model_id": model_id,
        "display_name": format!("E2E contract fixture · {model_id}"),
        "architecture": "deterministic-contract-fixture",
        "model_version": "e2e-contract-v1",
        "checkpoint_sha256": checkpoint(model_id),
        "capabilities": capabilities(model_id),
        "availability": "unknown",
    })
}

fn artifact_contract(name: &str, kind: &str) -> Value {
    json!({
        "name": name,
        "data_type": {"artifact": kind},
        "required": true,
        "multiple": false,
    })
}

fn model_manifest(model_id: &str) -> Value {
    let segmentation = model_id == "sam2.1-hiera-tiny";
    let mut inputs = vec![artifact_contract("image", "image")];
    if segmentation {
        inputs.push(artifact_contract("box_prompts", "box_prompt_set"));
    }
    json!({
        "schema_version": "1",
        "model_id": model_id,
        "display_name": format!("E2E contract fixture · {model_id}"),
        "architecture": "deterministic-contract-fixture",
        "model_version": "e2e-contract-v1",
        "connection": {
            "kind": "vision_worker_model",
            "worker_id": WORKER_ID,
            "worker_model_id": model_id,
        },
        "capabilities": capabilities(model_id),
        "input_contracts": inputs,
        "output_contracts": [artifact_contract(
            if segmentation { "masks" } else { "detections" },
            if segmentation { "mask_set" } else { "detection_set" },
        )],
        "prompt_contracts": if segmentation {
            vec![json!({"kind": "box", "required": true, "multiple": true})]
        } else {
            Vec::new()
        },
        "score_semantics": "relative_confidence",
        "geometry_semantics": if segmentation { "mask_refined_geometry" } else { "predicted_geometry" },
        "label_space": if segmentation || model_id == "locate-anything" {
            Value::Null
        } else {
            json!(["football"])
        },
        "checkpoint": {
            "sha256": checkpoint(model_id),
            "source": "deterministic browser-test fixture",
            "training_dataset_version": "synthetic-e2e-v1",
        },
        "runtime_requirements": {
            "devices": ["cpu"],
            "minimum_gpu_memory_mb": null,
            "dependencies": [],
            "supports_batch": false,
        },
        "license": {
            "code_license": "test-only",
            "weight_license": "test-only deterministic fixture",
            "source_url": null,
            "commercial_use": "restricted",
            "redistribution": "restricted",
            "usage_notes": ["Protocol fixture only; not real model accuracy."],
            "verified_from_official_source": false,
        },
        "availability": "unknown",
        "availability_evidence": {
            "health_passed": true,
            "protocol_compatible": true,
            "contracts_validated": true,
            "sample_conversion_passed": false,
            "weights_ready": true,
            "checked_at": checked_at(),
            "detail": "Browser fixture requires AnnotAgent selected-image conversion.",
        },
        "metadata": {"fixture": true, "real_model": false},
    })
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "detail": "deterministic Rust contract fixture",
        "checked_at": checked_at(),
    }))
}

async fn openai_models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [{"id": "e2e-pipeline-builder", "object": "model"}],
    }))
}

async fn worker_capabilities() -> Json<Value> {
    let all = MODEL_IDS
        .into_iter()
        .flat_map(capabilities)
        .collect::<BTreeSet<_>>();
    Json(json!({
        "protocol_version": 1,
        "worker_id": WORKER_ID,
        "model_identity": "e2e-multi-model-contract-fixture",
        "capabilities": all,
        "input_types": ["image", {"artifact": "box_prompt_set"}],
        "output_types": ["bounding_box", "detection_set", "mask_set"],
        "limits": {
            "max_images": 1,
            "max_input_artifacts": 8,
            "max_request_bytes": 20_000_000,
            "timeout_seconds": 10,
        },
        "models": MODEL_IDS.map(model_summary),
    }))
}

async fn worker_models() -> Json<Value> {
    Json(json!({
        "protocol_version": 1,
        "worker_id": WORKER_ID,
        "models": MODEL_IDS.map(model_summary),
    }))
}

async fn worker_contracts() -> Json<Value> {
    Json(json!({
        "protocol_version": 1,
        "worker_id": WORKER_ID,
        "models": MODEL_IDS.map(model_manifest),
    }))
}

fn message_text(message: &Value) -> Option<String> {
    match message.get("content")? {
        Value::String(value) => Some(value.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect(),
        ),
        _ => None,
    }
}

fn grounding_completion(request: &Value) -> Option<Value> {
    for message in request.get("messages")?.as_array()?.iter().rev() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let prompt: Value = serde_json::from_str(&message_text(message)?).ok()?;
        if prompt.get("task").and_then(Value::as_str) != Some("visual_object_grounding") {
            return None;
        }
        let label = prompt
            .get("target_label_ids")
            .and_then(Value::as_array)
            .and_then(|labels| labels.first())
            .and_then(Value::as_str)
            .unwrap_or("football");
        let qwen = prompt
            .pointer("/parameters/coordinate_format")
            .and_then(Value::as_str)
            == Some("qwen_0_1000_xyxy");
        let bbox = if qwen {
            json!([350, 350, 550, 550])
        } else {
            json!([0.35, 0.35, 0.2, 0.2])
        };
        return Some(json!({
            "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": json!({"detections": [{"label": label, "bbox": bbox, "confidence": 0.9}]}).to_string(),
                },
                "finish_reason": "stop",
            }],
            "usage": {"prompt_tokens": 40, "completion_tokens": 8, "total_tokens": 48},
        }));
    }
    None
}

fn tools_by_name(request: &Value) -> BTreeMap<&str, &Value> {
    request
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| Some((tool.pointer("/function/name")?.as_str()?, tool)))
        .collect()
}

fn called_tools(request: &Value) -> BTreeSet<&str> {
    request
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|message| {
            message
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|call| call.pointer("/function/name")?.as_str())
        .collect()
}

fn latest_subjects(request: &Value) -> Vec<Value> {
    request
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .rev()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .find_map(|message| {
            let parsed: Value = serde_json::from_str(&message_text(message)?).ok()?;
            parsed.get("subjects")?.as_array().cloned()
        })
        .unwrap_or_default()
}

fn tool_arguments(name: &str, tool: &Value, request: &Value) -> Option<Value> {
    match name {
        "load_skill_resource" => Some(json!({
            "skill_id": tool.pointer("/function/parameters/properties/skill_id/enum")?.as_array()?.first()?,
            "resource_name": tool.pointer("/function/parameters/properties/resource_name/enum")?.as_array()?.first()?,
        })),
        "create_draft_from_template" => Some(json!({"template_id": "safe_default"})),
        "dry_run_pipeline" => Some(json!({"image_indices": [0]})),
        "submit_detections" => Some(json!({
            "detections": [{"label": "football", "bbox": [0.35, 0.35, 0.2, 0.2], "confidence": 0.9}],
        })),
        "submit_classifications" => {
            // Explicit TEST scenario: exercise human assistance without changing live policy.
            let confidence = if request["model"]
                .as_str()
                .is_some_and(|model| model.ends_with("-review"))
            {
                0.4
            } else {
                0.9
            };
            let properties =
                tool.pointer("/function/parameters/properties/classifications/items/properties")?;
            let label = properties
                .pointer("/label/enum")?
                .as_array()?
                .first()?
                .as_str()?;
            let subject_ids = properties
                .pointer("/subject_artifact_id/enum")?
                .as_array()?;
            let mut subjects = latest_subjects(request);
            if subjects.is_empty() {
                subjects = subject_ids
                    .iter()
                    .map(|id| json!({"artifact_id": id, "item_id": null}))
                    .collect();
            }
            Some(json!({
                "classifications": subjects.into_iter().map(|subject| json!({
                    "subject_artifact_id": subject.get("artifact_id"),
                    "subject_item_id": subject.get("item_id").cloned().unwrap_or(Value::Null),
                    "label": label,
                    "confidence": confidence,
                    "scores": {label: confidence},
                })).collect::<Vec<_>>(),
            }))
        }
        _ => Some(json!({})),
    }
}

async fn openai_completion(
    State(_state): State<FixtureState>,
    Json(request): Json<Value>,
) -> Json<Value> {
    // Explicitly named deterministic test fixture for refresh/cancel lifecycle tests.
    if request["model"] == "e2e-slow-sample"
        && request["messages"].to_string().contains("image_url")
    {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }
    // The guided revision fixture must receive actual saved feedback and outcome context,
    // not merely a UI claiming to improve a plan. This does not simulate improved accuracy.
    if request["model"] == "e2e-slow-sample" {
        let contents = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|message| message["content"].as_str())
            .filter_map(|content| serde_json::from_str::<Value>(content).ok())
            .collect::<Vec<_>>();
        if contents
            .iter()
            .any(|value| value["build_mode"]["kind"] == "repair_draft")
            && !contents.iter().any(|value| {
                value["sample_evidence"]["feedback"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                    && value["saved_sample_observations"]
                        .as_array()
                        .is_some_and(|items| !items.is_empty())
            })
        {
            return Json(
                json!({"error": "TEST fixture: sample repair requires saved feedback and observed outcomes"}),
            );
        }
    }
    if let Some(response) = grounding_completion(&request) {
        return Json(response);
    }
    let tools = tools_by_name(&request);
    if tools.contains_key("propose_future_annotation_schema") {
        // A separate bounded TEST protocol. It never routes through the generic
        // planner or the original Schema proposal fixture, and proves no quality.
        let remote = request["model"].as_str().unwrap_or_default();
        if remote.ends_with("-manual-slow") {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        } else if remote.ends_with("-slow") {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        }
        let context = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|message| message["role"] == "user")
            .filter_map(message_text)
            .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
            .find(|value| value["scope"] == "future_tasks_only");
        let Some(context) = context else {
            return Json(
                json!({"error":"TEST future proposal requires the saved future-only context"}),
            );
        };
        let source = &context["source"];
        let base = &context["base_schema"];
        let feedback = &context["feedback"];
        let labels = base["definition"]["task"]["labels"].as_array();
        if tools.len() != 1
            || request.to_string().contains("image_url")
            || feedback["pixels_supplied"] != false
            || source["base_schema_id"] != base["id"]
            || source["base_schema_revision"] != base["revision"]
            || feedback["message"]["input"]["reference"]["task_id"] != base["task_id"]
            || feedback["message"]["input"]["reference"]["scope"] != "sample_candidate"
            || !feedback["message"]["input"]["text"]
                .as_str()
                .is_some_and(|text| text.contains("TEST future rule request"))
            || labels.is_none_or(|labels| labels.len() < 2)
        {
            return Json(
                json!({"error":"TEST future proposal omitted or substituted the exact tested Schema, frozen feedback, or text-only boundary"}),
            );
        }
        if remote.ends_with("-unknown") {
            return Json(
                json!({"error":"TEST future proposal completion unavailable after request admission"}),
            );
        }
        let mut labels = labels.cloned().unwrap_or_default();
        let removed = labels.pop().unwrap_or(Value::Null);
        let mut rules = base["definition"]["boundary_rules"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        rules.push(json!(if base["definition"]["task"]["kind"] == "bounding_box" {
            "TEST occluded targets: annotate only the visible region; never infer the hidden extent."
        } else {
            "TEST obscured scene evidence requires human review; never infer an excluded category."
        }));
        let goal = format!(
            "TEST future-only rules: {}",
            base["definition"]["goal"].as_str().unwrap_or_default()
        );
        let mut arguments = if remote.ends_with("-clarify") {
            json!({"goal":goal,"decision":"clarify","question":"TEST: Should occluded targets use visible boundaries only?","rationale":"TEST asks for human semantics; no new Schema was created"})
        } else {
            json!({"goal":goal,"decision":"draft","kind":base["definition"]["task"]["kind"],"labels":labels,"multi_label":base["definition"]["task"]["multi_label"],"attributes":base["definition"]["task"]["attributes"],"boundary_rules":rules,"rationale":format!("TEST scripted proposal from Schema {} revision {}; remove {}. No pixels were inspected and no saved data was changed.",base["id"],base["revision"],removed)})
        };
        if remote.ends_with("-invalid") {
            arguments["geometry"] = json!([0.0, 0.0, 1.0, 1.0]);
        }
        return Json(json!({
            "id":format!("TEST-future-schema-{}",uuid::Uuid::new_v4()),"object":"chat.completion",
            "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"test-future-schema-call","type":"function","function":{"name":"propose_future_annotation_schema","arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":40,"completion_tokens":8,"total_tokens":48}
        }));
    }
    if tools.contains_key("propose_candidate_feedback") {
        // Deterministic TEST transport only. These scenarios validate bounded
        // orchestration and persisted evidence, not LLM semantic reliability.
        let remote = request["model"].as_str().unwrap_or_default();
        if remote.ends_with("-feedback-slow") {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        }
        if remote.ends_with("-feedback-unknown") {
            // A request reached the transport, but no usable completion or usage
            // receipt came back. The client cannot assume a free, safe retry.
            return Json(json!({"error":"TEST unusable completion after request admission"}));
        }
        let context = request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|message| message["role"] == "user")
            .filter_map(message_text)
            .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
            .find(|value| value.get("saved_candidate_context").is_some());
        let Some(context) = context else {
            return Json(json!({"error":"TEST feedback requires saved candidate context"}));
        };
        if request.to_string().contains("image_url") {
            return Json(json!({"error":"TEST feedback interpretation must not send pixels"}));
        }
        let candidate = &context["saved_candidate_context"]["candidate"]["outcome"];
        let message = context["saved_user_message"].as_str().unwrap_or_default();
        let arguments = if remote.ends_with("-feedback-invalid") {
            json!({"decision":"request_correction","reason":"poor_boundary","question":"TEST unauthorized coordinates","rationale":"TEST invalid output must not become feedback","bbox":[0.0,0.0,1.0,1.0]})
        } else if remote.ends_with("-feedback-clarify")
            || remote.ends_with("-feedback-future-base-clarify")
        {
            json!({"decision":"clarify_scope","question":"TEST: remove this candidate, or change the label definition?","rationale":"TEST ambiguous scope; no change was authorized"})
        } else {
            json!({"decision":"request_correction","reason":if candidate["value"]["kind"] == "bounding_box" {"poor_boundary"} else {"wrong_label"},"question":"TEST: confirm the correction to this saved candidate","rationale":format!("TEST frozen candidate {} and saved message: {}", candidate["id"].as_str().unwrap_or_default(), message)})
        };
        return Json(json!({
            "id":format!("TEST-feedback-{}",uuid::Uuid::new_v4()),"object":"chat.completion",
            "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"test-feedback-call","type":"function","function":{"name":"propose_candidate_feedback","arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":40,"completion_tokens":8,"total_tokens":48}
        }));
    }
    if request["model"] == "e2e-conversation-classification-schema-background"
        && tools.contains_key("propose_annotation_schema")
    {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }
    if request["model"] == "e2e-conversation-classification-background"
        && !tools.is_empty()
        && !tools.contains_key("propose_annotation_schema")
    {
        // TEST-only slow planner: prove the HTTP reply/navigation precedes completion.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if tools.contains_key("propose_annotation_schema") {
        let serialized = request.to_string();
        if serialized.contains("image_url") {
            return Json(json!({"error":"TEST schema proposal must be text only"}));
        }
        let classification = serialized.contains("室内");
        let arguments = if request["model"] == "e2e-conversation-invalid-schema" {
            json!({"decision":"draft","kind":"classification","labels":[],"multi_label":false,"attributes":{},"boundary_rules":[],"rationale":"TEST invalid empty label set"})
        } else if request["model"] == "e2e-conversation-clarify" {
            json!({"decision":"clarify","question":"TEST: Which output type and labels should this task use?","rationale":"TEST ambiguous goal; human semantics required"})
        } else if request["model"] == "e2e-conversation-bbox-feedback-future-base-clarify" {
            // Only this new TEST baseline has two bbox labels, so the later
            // proposal can remove one without an invalid empty label schema.
            json!({"decision":"draft","kind":"bounding_box","labels":["cup","bottle"],"multi_label":false,"attributes":{},"boundary_rules":["TEST fixture rule"],"rationale":"TEST two-label baseline for a future-only rule proposal"})
        } else {
            json!({"decision":"draft","kind":if classification {"classification"} else {"bounding_box"},"labels":if classification {json!(["室内","室外"])} else {json!(["cup"])},"multi_label":false,"attributes":{},"boundary_rules":["TEST fixture rule"],"rationale":"TEST scripted Schema proposal, not Live model quality evidence"})
        };
        return Json(
            json!({"id":format!("TEST-schema-{}",uuid::Uuid::new_v4()),"object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"test-schema-call","type":"function","function":{"name":"propose_annotation_schema","arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":40,"completion_tokens":8,"total_tokens":48}}),
        );
    }
    let called = called_tools(&request);
    let preferences = [
        "get_pipeline_builder_context",
        "load_skill_resource",
        "resolve_pipeline_feasibility",
        "create_draft_from_template",
        "create_blocked_draft",
        "validate_pipeline",
        "dry_run_pipeline",
        "inspect_dry_run_summary",
        "submit_draft_for_human_approval",
        "finish_with_setup_requirements",
        "submit_detections",
        "submit_classifications",
    ];
    for name in preferences {
        let Some(tool) = tools.get(name) else {
            continue;
        };
        if called.contains(name) {
            continue;
        }
        let Some(arguments) = tool_arguments(name, tool, &request) else {
            continue;
        };
        return Json(json!({
            "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": format!("call-{}", uuid::Uuid::new_v4()),
                        "type": "function",
                        "function": {"name": name, "arguments": arguments.to_string()},
                    }],
                },
                "finish_reason": "tool_calls",
            }],
            "usage": {"prompt_tokens": 40, "completion_tokens": 8, "total_tokens": 48},
        }));
    }
    Json(json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "OK"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
    }))
}

fn mask_response(request: &Value) -> Result<Value> {
    let prompt_set = request
        .get("input_artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|envelope| envelope.get("kind").and_then(Value::as_str) == Some("box_prompt_set"))
        .and_then(|envelope| envelope.get("artifact"))
        .context("missing BoxPromptSet")?;
    let prompt = prompt_set
        .get("prompts")
        .and_then(Value::as_array)
        .and_then(|prompts| prompts.first())
        .context("missing box prompt")?;
    let source_prompts = prompt_set
        .get("reference")
        .context("missing prompt reference")?;
    Ok(json!({
        "protocol_version": 1,
        "request_id": request.get("request_id"),
        "model_identity": request.get("model_id"),
        "artifacts": [{
            "kind": "mask_set",
            "artifact": {
                "reference": {
                    "artifact_id": format!("e2e-mask-set:{}", uuid::Uuid::new_v4()),
                    "source_node": request.get("node_id"),
                    "port": "masks",
                    "artifact_type": "mask_set",
                    "item_id": null,
                },
                "image_id": request.get("image_id"),
                "model_binding": request.get("model_id"),
                "source_prompts": source_prompts,
                "validation_state": "unvalidated",
                "masks": [{
                    "mask_id": "e2e-mask",
                    "prompt": {
                        "artifact_id": source_prompts.get("artifact_id"),
                        "source_node": source_prompts.get("source_node"),
                        "port": source_prompts.get("port"),
                        "artifact_type": source_prompts.get("artifact_type"),
                        "item_id": prompt.get("id"),
                    },
                    "mask": {"encoding": "coco_rle", "width": 4, "height": 4, "counts": "5 2 2 2 5"},
                    "score": {"value": 0.9, "semantics": "relative_confidence"},
                    "attributes": {"fixture": true},
                }],
                "metadata": {"fixture": true, "real_model": false},
            },
        }],
        "metadata": {"fixture": true},
        "usage": {"source": "deterministic_fixture", "compute_milliseconds": 1},
        "timings": {"total_ms": 1},
        "warnings": ["Deterministic Rust contract fixture; no real model inference."],
        "error": null,
    }))
}

fn detection_response(request: &Value) -> Value {
    let model_id = request
        .get("model_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    json!({
        "protocol_version": 1,
        "model_identity": model_id,
        "artifacts": [{
            "id": uuid::Uuid::new_v4(),
            "image_id": request.get("image_id"),
            "task_id": request.get("task_id"),
            "label": "football",
            "role": "candidate",
            "value": {"kind": "bounding_box", "rect": [0.35, 0.35, 0.2, 0.2]},
            "source_node": request.get("node_id"),
            "confidence": 0.9,
            "metadata": {"fixture": true, "real_model": false},
            "validation_state": "unvalidated",
            "provenance": {
                "provider": "e2e_contract_fixture",
                "model": model_id,
                "tool": null,
                "request_id": request.get("request_id"),
                "model_digest": checkpoint(model_id),
                "input_artifact_ids": [],
            },
            "revision": 1,
            "replaces_artifact_id": null,
            "created_at": checked_at(),
        }],
        "request_id": request.get("request_id"),
        "metadata": {"fixture": true},
        "usage": {"source": "deterministic_fixture", "compute_milliseconds": 1},
        "warnings": ["Deterministic Rust contract fixture; no real model inference."],
        "timings": {"total_ms": 1},
        "error": null,
    })
}

async fn infer(Json(request): Json<Value>) -> (StatusCode, Json<Value>) {
    if request.get("operation").and_then(Value::as_str) == Some("prompted_segmentation") {
        match mask_response(&request) {
            Ok(response) => (StatusCode::OK, Json(response)),
            Err(error) => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("invalid fixture request: {error}")})),
            ),
        }
    } else {
        (StatusCode::OK, Json(detection_response(&request)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let port = std::env::var("ANNOTAGENT_E2E_WORKER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8796);
    let app = Router::new()
        .route("/health", get(health))
        .route("/openai/v1/models", get(openai_models))
        .route("/openai/v1/chat/completions", post(openai_completion))
        .route("/v1/capabilities", get(worker_capabilities))
        .route("/v1/models", get(worker_models))
        .route("/v1/contracts", get(worker_contracts))
        .route("/v1/infer", post(infer))
        .with_state(FixtureState);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("AnnotAgent deterministic Rust E2E fixture: http://127.0.0.1:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
