#!/usr/bin/env python3
"""UIAPI-004: consume TEST bbox pending answer to verify queue preflight/races."""
import argparse
import datetime
import json
from pathlib import Path
import urllib.parse
import urllib.request

from http_smoke import Client, uid


def verify(c, m):
    bbox = m["bbox"]
    tr = bbox["task_root"]
    cr = tr.rsplit("/tasks/", 1)[0]
    human = next(h for h in c.get(tr + "/human-requests") if h["input"]["id"] == bbox["pending_request_id"])
    assert human["status"] == "pending", "Use a fresh unedited TEST seed"
    command = {"message": {"id": uid(), "text": "TEST supplement: retain tight cup boundaries", "image": None}, "task_id": bbox["task_id"], "schema_revision": c.get("/api/projects/" + bbox["project"] + "/goal")["revision"], "mode": "plan"}
    receipt = c.post(cr + "/send", command)
    queue = tr + "/message-queue/" + command["message"]["id"]
    calls_before = c.get(tr + "/calls")
    budget_before = c.get(tr + "/budget")
    blocked = c.request("GET", queue + "/schema-preview", expected=[409])
    assert blocked["code"] == "human_input_pending" and blocked["admitted"] is False
    assert c.get(queue + "/schema-authorization") is None
    assert c.get(tr + "/calls") == calls_before
    c.post(bbox["answer_url"], bbox["answer_example"])
    preview = c.get(queue + "/schema-preview")
    consent = {key: preview[key] for key in ["model_id", "scope_hash", "request_hash", "previous_grant_id", "maximum_calls", "expires_at"]}
    consent.update(call_id=uid(), allow_unknown_cost=True)
    # Reproduce input arriving between successful preview and approval.
    pending = {**human["input"], "id": uid(), "expected_feedback_sequence": bbox["answer_example"]["answer"]["sequence"], "resume_checkpoint_ref": uid(), "question": "TEST queued preview race: confirm bbox again"}
    c.post(tr + "/human-requests", pending)
    blocked_post = c.request("POST", queue + "/schema-proposals", consent, expected=[409])
    assert blocked_post["code"] == "human_input_pending" and blocked_post["admitted"] is False
    assert c.get(queue + "/schema-authorization") is None
    assert c.get(tr + "/calls") == calls_before
    assert c.get(tr + "/budget") == budget_before
    answer = {**bbox["answer_example"]["answer"], "revision_id": uid(), "sequence": pending["expected_feedback_sequence"] + 1, "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    c.post(tr + "/human-requests/" + pending["id"] + "/answer", {"answer": answer})
    admitted = c.post(queue + "/schema-proposals", consent)
    assert admitted["status"] == "completed", admitted
    assert c.post(queue + "/schema-proposals", consent) == admitted
    assert len(c.get(tr + "/calls")) == len(calls_before) + 1
    return {"send": receipt, "blocked_preview": blocked, "blocked_post": blocked_post, "same_consent_after_answer": admitted, "calls_added": 1, "no_grant_or_usage_change_when_blocked": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--enable-fixture", action="store_true", required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    path = args.manifest.resolve()
    assert (path.parent / "FIXTURE_ONLY").read_text() == "AnnotAgent HTTP integration fixture\n"
    m = json.loads(path.read_text())
    url = urllib.parse.urlparse(m["base_url"])
    assert url.scheme == "http" and url.hostname == "127.0.0.1" and url.port != 8787
    with urllib.request.urlopen(m["base_url"] + "/api/health", timeout=5) as response:
        assert response.headers["x-annotagent-fixture"] == "external-model-only"
    c = Client(m["base_url"])
    result = {}
    try:
        result = verify(c, m)
        print(json.dumps(result, indent=2))
    finally:
        output = path.parent / "UIAPI-004_HTTP.json"
        output.write_text(json.dumps({"result": result, "trace": c.trace}, ensure_ascii=False, indent=2) + "\n")
        print(output)


if __name__ == "__main__":
    main()
