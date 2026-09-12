"""UIAPI-001: real paused checkpoint and cancelled Processing fixtures."""
import json
import struct
import urllib.parse
import zlib

from http_smoke import uid


def seed_controls(c, root, provider):
    model = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST slow visual control", "remote_model_id": "e2e-slow-sample", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    return {kind: seed_processing(c, root, model, kind) for kind in ["interrupted", "resumable"]}


def seed_processing(c, root, model, kind):
    project = "TEST-agent-ui-" + kind + "-" + uid()
    p = "/api/projects/" + project
    c.post("/api/projects", {"id": project, "yaml": f"version: 1\nproject:\n  name: TEST {kind} control\ndataset:\n  root: images\nruntime:\n  max_parallel_images: 1\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"})
    c.request("PUT", p + "/model-bindings", {"bindings": [{"capability": "image_classification", "role": "classification", "match_kind": "capability", "model_profile_id": model["id"], "locked": False}]})
    png = (root / "examples/robocup/images/synthetic-robocup.png").read_bytes()
    # Distinct synthetic input hashes; valid PNG metadata, no user image edits.
    for index in range(3):
        chunk = b"tEXt" + f"TEST\x00{kind}-{index}".encode()
        image = png[:-12] + struct.pack(">I", len(chunk) - 4) + chunk + struct.pack(">I", zlib.crc32(chunk)) + png[-12:]
        c.request("POST", p + f"/image-upload?name=TEST-{index}.png", image)
    conversation = c.post(p + "/conversations")["conversation_id"]
    cr = p + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": model["id"]})
    receipt = c.post(cr + "/send", {"message": {"id": uid(), "text": "TEST Classify this image as day", "image": None}, "schema_revision": c.get(p + "/goal")["revision"], "mode": "plan"})
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    schema = c.post(tr + "/human-schema-drafts", {"request_id": uid(), "decision": {"decision": "draft", "kind": "classification", "labels": ["day"], "multi_label": False, "attributes": {}, "boundary_rules": ["TEST classification only"], "rationale": "Explicit TEST schema"}})
    preview = c.get(tr + "/builder-preview?" + urllib.parse.urlencode({"operation_id": uid(), "schema_id": schema["id"], "schema_revision": schema["revision"], "model_id": model["id"]}))
    builder = c.post(tr + "/builder-operations", {**{key: preview[key] for key in ["selection", "repair", "previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True})
    assert builder["status"] == "completed"
    draft_id = builder["evidence"]["draft_id"]
    draft = next(item for item in c.get("/api/workflow-drafts?project_id=" + project)["drafts"] if item["id"] == draft_id)
    nodes = [node for node in draft["nodes"] if node.get("model_profile_binding") or node.get("model_binding")]
    assert len(nodes) == 1
    nodes[0]["model_profile_binding"] = {"model_profile_id": model["id"], "locked": True}
    nodes[0]["model_binding"] = model["remote_model_id"]
    # Keep the typed LabelPipeline projection aligned with the authoring graph.
    # Leaving the built-in mock-classifier identity here makes the normal TEST
    # Registry purge correctly treat this as a fixture-backed Draft on restart.
    for pipeline in (draft.get("label_pipeline") or {}).get("label_pipelines", []):
        for step in pipeline.get("steps", []):
            if step.get("id") == nodes[0]["id"] and step.get("model_binding"):
                step["model_binding"]["model_id"] = model["remote_model_id"]
    c.request("PATCH", "/api/workflow-drafts/" + draft_id, draft, extra_headers={"if-match": str(draft["revision"])})
    preview = c.get(tr + "/sample-preview?" + urllib.parse.urlencode({"draft_id": draft_id, "request_id": uid()}))
    budget = preview["conversation_budget"]
    sample = c.post(p + "/sample-operations", {"request_id": preview["request_id"], "draft_id": draft_id, "expected_revision": preview["revision"], "image_indices": list(range(preview["image_count"])), "authorization_fingerprint": preview["authorization_fingerprint"], "conversation": {"conversation_id": conversation, "task_id": task, **{key: budget[key] for key in ["previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True, "human_review": True}})
    c.poll(p + "/sample-operations/" + sample["id"], lambda v: v["status"] == "succeeded" and (v.get("assistance") or {}).get("status") == "completed")
    selection = {"draft_id": draft_id, "sample_test_id": sample["id"], "limit": 3}
    preview = c.get(p + "/processing-preview?" + urllib.parse.urlencode(selection))
    operation = c.post(p + "/processing-operations", {"request_id": uid(), "selection": selection, "expected_revision": preview["revision"], "authorization_fingerprint": preview["authorization_fingerprint"]})
    bp = "/api/batches/" + operation["batch_id"]
    c.poll(bp, lambda v: v["batch"]["progress"]["running_images"] == 1)
    result = {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "batch_url": bp, "draft_id": draft_id, "sample_test_id": sample["id"]}
    if kind == "interrupted":
        message = {"id": uid(), "text": "停止", "image": None, "reference": {"scope": "stop_request", "task_id": task}}
        initial = c.post(cr + "/stop-requests", message)
        sp = cr + "/stop-requests/" + message["id"]
        final = c.poll(sp, lambda v: v["normalized_state"] == "interrupted")
        c.poll(bp, lambda v: v["batch"]["status"] == "cancelled")
        result.update(request_url=sp, initial=initial, final=final)
    else:
        c.post(bp + "/pause")
        paused = c.poll(bp, lambda v: v["batch"]["status"] == "paused" and v["batch"]["progress"]["running_images"] == 0)
        completed = [image["child_run_id"] for image in paused["batch"]["images"] if image["execution_status"] == "completed"]
        assert len(completed) == 1, paused
        c.post(bp + "/resume")
        c.poll(bp, lambda v: v["batch"]["progress"]["running_images"] == 1)
        c.post(bp + "/pause")
        paused = c.poll(bp, lambda v: v["batch"]["status"] == "paused" and v["batch"]["progress"]["running_images"] == 0)
        assert paused["batch"]["progress"]["completed_images"] == 2
        assert paused["batch"]["progress"]["pending_images"] == 1
        assert completed[0] in paused["batch"]["child_run_ids"]
        snapshot = c.get(tr + "/workspace")
        action = next(action for action in snapshot["resume_actions"] if action["kind"] == "batch")
        assert action["available"] is True
        result.update(resume_action=action, completed_child_run_ids=paused["batch"]["child_run_ids"], budget=c.get(tr + "/budget"))
    return result
