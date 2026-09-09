#!/usr/bin/env python3
"""Prepare a fresh, unstarted real HTTP Stop scene on an explicit TEST fixture."""
import argparse
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request

from http_smoke import Client, uid


def prepare_stop(c, cr, schema_revision, provider_id, sample_model_id):
    model = c.post("/api/model-profiles", {"provider_id": provider_id, "display_name": "TEST manual 30-second stop window", "remote_model_id": "e2e-conversation-classification-schema-manual-stop", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider_id}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    original = c.get(cr + "/agent-model")
    selected = c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": original["revision"], "model_profile_id": model["id"]})
    receipt = c.post(cr + "/send", {"message": {"id": uid(), "text": "TEST browser stop evidence: classify indoor/outdoor", "image": None}, "schema_revision": schema_revision, "mode": "plan"})
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": selected["revision"], "model_profile_id": original["model_profile_id"]})
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    query = urllib.parse.urlencode({"consent_id": uid(), "schema_call_id": uid(), "builder_operation_id": uid(), "sample_operation_id": uid(), "planner_model_id": model["id"], "allowed_models": json.dumps(["model-profile:" + sample_model_id])})
    consent = c.get(tr + "/journey-preview?" + query)["consent"]
    consent["allow_unknown_cost"] = True
    consent["schema_proposal"]["allow_unknown_cost"] = True
    c.post(tr + "/journey-consents", consent)
    stop_body = {"id": uid(), "text": "停止", "image": None, "reference": {"scope": "stop_request", "task_id": task}}
    return {"task_id": task, "task_root": tr, "workspace_url": tr + "/workspace", "model_profile_id": model["id"], "delay_seconds": 30, "expires_at": consent["expires_at"], "start": {"method": "POST", "url": tr + "/journey-consents/" + consent["id"] + "/execution", "body": {}}, "wait_for_reserved_url": tr + "/calls", "stop": {"method": "POST", "url": cr + "/stop-requests", "body": stop_body}, "observation_url": cr + "/stop-requests/" + stop_body["id"], "expected_final": "outcome_unknown; underlying schema call in_doubt", "status": "prepared_not_started"}


def verify_stop(c, scene):
    c.post(scene["start"]["url"], scene["start"]["body"])
    c.poll(scene["wait_for_reserved_url"], lambda calls: any(call["status"] == "reserved" for call in calls))
    time.sleep(2)
    assert any(call["status"] == "reserved" for call in c.get(scene["wait_for_reserved_url"]))
    initial = c.post(scene["stop"]["url"], scene["stop"]["body"])
    assert initial["normalized_state"] == "stopping", initial
    time.sleep(2)
    during = c.get(scene["observation_url"])
    assert during["normalized_state"] in ["stopping", "outcome_unknown"], during
    final = c.poll(scene["observation_url"], lambda v: v["normalized_state"] == "outcome_unknown")
    calls = c.get(scene["wait_for_reserved_url"])
    assert any(call["status"] == "in_doubt" for call in calls), calls
    snapshot = c.get(scene["workspace_url"])
    assert snapshot["actions"]["resume"]["available"] is False
    return {"initial": initial, "after_two_seconds": during, "final": final, "calls": calls}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--enable-fixture", action="store_true", required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--verify", action="store_true", help="Start/stop this new scene and verify it; otherwise leave for browser")
    args = parser.parse_args()
    manifest_path = args.manifest.resolve()
    assert (manifest_path.parent / "FIXTURE_ONLY").read_text() == "AnnotAgent HTTP integration fixture\n"
    m = json.loads(manifest_path.read_text())
    url = urllib.parse.urlparse(m["base_url"])
    assert url.scheme == "http" and url.hostname == "127.0.0.1" and url.port != 8787
    with urllib.request.urlopen(m["base_url"] + "/api/health", timeout=5) as response:
        assert response.headers["x-annotagent-fixture"] == "external-model-only"
    c = Client(m["base_url"])
    cr = m["task_root"].rsplit("/tasks/", 1)[0]
    scene = prepare_stop(c, cr, c.get("/api/projects/" + m["project"] + "/goal")["revision"], m["provider_id"], m["model_profile_id"])
    if args.verify:
        scene["verification"] = verify_stop(c, scene)
        scene["status"] = "verified_outcome_unknown"
    path = manifest_path.parent / ("UIAPI-003_STOP_" + scene["task_id"] + ".json")
    path.write_text(json.dumps({"fixture": "external-model-only", "scene": scene, "http_trace": c.trace}, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(scene, ensure_ascii=False, indent=2))
    print("Scene and real HTTP trace:", path)


if __name__ == "__main__":
    main()
