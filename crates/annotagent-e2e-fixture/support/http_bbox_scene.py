"""UIAPI-003 bbox geometry through the real sample/answer API."""
import datetime
import json
import struct
import urllib.parse

from http_smoke import uid


def seed_bbox(c, root, provider):
    model = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST bbox geometry", "remote_model_id": "e2e-conversation-bbox-feedback-clarify", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    project = "TEST-agent-ui-bbox-" + uid()
    p = "/api/projects/" + project
    c.post("/api/projects", {"id": project, "yaml": "version: 1\nproject:\n  name: TEST bbox geometry\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"})
    c.request("PUT", p + "/model-bindings", {"bindings": [{"capability": "vision_language", "role": "detection", "match_kind": "capability", "model_profile_id": model["id"], "locked": False}]})
    png = (root / "examples/robocup/images/synthetic-robocup.png").read_bytes()
    width, height = struct.unpack(">II", png[16:24])
    c.request("POST", p + "/image-upload?name=TEST-bbox.png", png)
    conversation = c.post(p + "/conversations")["conversation_id"]
    cr = p + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": model["id"]})
    receipt = c.post(cr + "/send", {"message": {"id": uid(), "text": "Find cups, not bottles. Draw tight bounding boxes.", "image": None}, "schema_revision": c.get(p + "/goal")["revision"], "mode": "plan"})
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    schema = c.post(tr + "/human-schema-drafts", {"request_id": uid(), "decision": {"decision": "draft", "kind": "bounding_box", "labels": ["cup"], "multi_label": False, "attributes": {}, "boundary_rules": ["TEST tight cup box"], "rationale": "Explicit TEST bbox schema"}})
    preview = c.get(tr + "/builder-preview?" + urllib.parse.urlencode({"operation_id": uid(), "schema_id": schema["id"], "schema_revision": schema["revision"], "model_id": model["id"]}))
    builder = c.post(tr + "/builder-operations", {**{key: preview[key] for key in ["selection", "repair", "previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True})
    draft_id = builder["evidence"]["draft_id"]
    draft = next(item for item in c.get("/api/workflow-drafts?project_id=" + project)["drafts"] if item["id"] == draft_id)
    # The safe-default template contains a generic detector. Explicitly edit
    # this TEST draft through the same HTTP CAS used by the Workflow editor,
    # selecting the existing VLM operation for the OpenAI-compatible fixture.
    parameters = {"labels": ["cup"], "target_description": "the cup itself"}
    detector = next(node for node in draft["nodes"] if node["id"] == "shared.detector")
    detector.update(node_type="vlm_detection.detect", kind="vision_language_model", parameters=parameters, model_binding=None, model_profile_binding={"model_profile_id": model["id"], "locked": True})
    for stage in draft["label_pipeline"]["shared_stages"]:
        for step in stage["steps"]:
            if step["id"] == "shared.detector":
                step.update(node_type="vlm_detection.detect", kind="vision_language_model", parameters=parameters, model_binding=None, model_profile_binding={"model_profile_id": model["id"], "locked": True})
    c.request("PATCH", "/api/workflow-drafts/" + draft_id, draft, extra_headers={"if-match": str(draft["revision"])})
    preview = c.get(tr + "/sample-preview?" + urllib.parse.urlencode({"draft_id": draft_id, "request_id": uid()}))
    budget = preview["conversation_budget"]
    sample = c.post(p + "/sample-operations", {"request_id": preview["request_id"], "draft_id": draft_id, "expected_revision": preview["revision"], "image_indices": [0], "authorization_fingerprint": preview["authorization_fingerprint"], "conversation": {"conversation_id": conversation, "task_id": task, **{key: budget[key] for key in ["previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True, "human_review": True}})
    c.poll(p + "/sample-operations/" + sample["id"], lambda v: v["status"] == "succeeded" and (v.get("assistance") or {}).get("status") == "completed")
    sample_url = f"/api/workflow-drafts/{draft_id}/sample-test?test_id={sample['id']}"
    record = c.get(sample_url)["sample_test"]
    assert record["report"]["summary"]["failed_count"] == 0, record["report"]
    assert (record["report"]["samples"][0]["width"], record["report"]["samples"][0]["height"]) == (width, height)
    projection = record["report"]["samples"][0]["projection"]
    candidates = projection["final_candidates"] + [item["candidate"] for item in projection["review_candidates"]]
    human = next(item for item in c.get(tr + "/human-requests") if item["status"] == "pending")
    candidate = next(item for item in candidates if item["outcome"]["id"] == human["input"]["outcome_id"])
    assert candidate["outcome"]["value"]["kind"] == "bounding_box", candidate
    image = next(item for item in record["inputs"] if item["image_id"] == human["input"]["image_id"])
    feedback_url = f"/api/workflow-sample-tests/{record['id']}/images/{image['image_id']}/feedback"
    before = c.get(feedback_url)["revisions"]
    assert not before
    answer = {"revision_id": uid(), "sample_test_id": record["id"], "image_id": image["image_id"], "sequence": human["input"]["expected_feedback_sequence"] + 1, "reason": "poor_boundary", "outcome_id": candidate["outcome"]["id"], "corrected_value": {"kind": "bounding_box", "rect": [0.1, 0.15, 0.12, 0.2]}, "corrected_label": "cup", "note": "TEST normalized bbox HTTP save", "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    answer_url = tr + "/human-requests/" + human["input"]["id"] + "/answer"
    saved = c.post(answer_url, {"answer": answer})
    replay = c.post(answer_url, {"answer": answer})
    assert saved["answer"] == replay["answer"]
    revisions = c.get(feedback_url)["revisions"]
    assert len(revisions) == 1 and revisions[0]["revision_id"] == answer["revision_id"]
    assert all(abs(a - b) < 1e-6 for a, b in zip(revisions[0]["corrected_value"]["rect"], answer["corrected_value"]["rect"]))
    c.request("POST", answer_url, {"answer": {**answer, "note": "TEST conflicting retry"}}, expected=[400, 409])
    # Leave a real unanswered request over the original saved terminal candidate.
    pending_input = {**human["input"], "id": uid(), "expected_feedback_sequence": answer["sequence"], "resume_checkpoint_ref": uid(), "question": "TEST browser: adjust this saved cup bounding box"}
    pending = c.post(tr + "/human-requests", pending_input)
    assert pending["status"] == "pending" and pending.get("answer") is None
    c.get(tr + "/workspace")
    browser_answer = {**answer, "revision_id": uid(), "sequence": answer["sequence"] + 1, "note": "TEST browser bbox correction", "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    return {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "sample_url": sample_url, "sample_test_id": record["id"], "draft_id": record["draft_id"], "candidate_id": candidate["outcome"]["id"], "source_artifact_id": candidate["source_artifact_id"], "terminal_candidate": candidate, "projection_group": "review_candidates" if any(item["candidate"]["outcome"]["id"] == candidate["outcome"]["id"] for item in projection["review_candidates"]) else "final_candidates", "image_id": image["image_id"], "content_hash": image["content_hash"], "image_dimensions": {"width": width, "height": height}, "feedback_url": feedback_url, "feedback_revision_id": answer["revision_id"], "feedback_sequence": answer["sequence"], "pending_request_id": pending_input["id"], "answer_url": tr + "/human-requests/" + pending_input["id"] + "/answer", "answer_example": {"answer": browser_answer}, "normalized_rect_order": "x,y,width,height; unit interval; Rust f32 rounding"}
