"""TEST-only real HTTP regression: one Applied repair, two other images pending.
Run against the explicitly enabled http_fixture.py manifest. Never accepts a live URL.
"""
import json
from pathlib import Path
import sys
import tempfile
import urllib.parse
import urllib.request
from http_smoke import Client, uid
from http_bbox_scene import seed_bbox


def main():
    manifest_path = Path(sys.argv[1]).resolve()
    workspace = manifest_path.parent
    assert workspace.is_relative_to(Path(tempfile.gettempdir()).resolve())
    assert workspace.name.startswith("TEST-agent-ui-")
    assert (workspace / "FIXTURE_ONLY").read_text() == "AnnotAgent HTTP integration fixture\n"
    manifest = json.loads(manifest_path.read_text())
    assert manifest["fixture"] == "external-model-only"
    base = manifest["base_url"]
    assert urllib.parse.urlparse(base).hostname == "127.0.0.1"
    assert urllib.parse.urlparse(base).port not in (8787, 8788)
    with urllib.request.urlopen(base + "/api/health") as response:
        assert response.headers.get("x-annotagent-fixture") == "external-model-only"
    c = Client(base)
    provider = c.get("/api/providers")
    if isinstance(provider, dict):
        provider = provider["providers"]
    provider = next(p for p in provider if p["id"] == manifest["provider_id"])
    assert provider["base_url"] == manifest["provider_url"]
    scene = seed_bbox(c, Path(__file__).resolve().parents[3], provider, image_count=3)
    tr = scene["task_root"]
    before = c.get(tr + "/human-requests")
    others = [r for r in before if r["status"] == "pending" and r["input"]["image_id"] != scene["image_id"]]
    assert len(others) == 2, before
    source = next(r for r in before if r["input"]["id"] == scene["answered_request_id"])
    assert source["status"] == "applied", source
    calls = c.get(tr + "/calls")
    query = {"operation_id": uid(), "schema_id": scene["schema_id"], "schema_revision": scene["schema_revision"], "model_id": scene["model_profile_id"], "repair_request_id": scene["answered_request_id"]}
    preview = c.get(tr + "/builder-preview?" + urllib.parse.urlencode(query))
    consent = {**{key: preview[key] for key in ["selection", "repair", "previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True}
    result = c.post(tr + "/builder-operations", consent)
    assert result["status"] == "completed", result
    after_calls = c.get(tr + "/calls")
    assert len(after_calls) > len(calls), (calls, after_calls)
    assert all(call["status"] == "completed" for call in after_calls[len(calls):]), after_calls
    after = c.get(tr + "/human-requests")
    for other in others:
        assert next(r for r in after if r["input"]["id"] == other["input"]["id"]) == other
    assert c.post(tr + "/builder-operations", consent) == result
    assert c.get(tr + "/calls") == after_calls
    draft_id = result["evidence"]["draft_id"]
    sample_preview = c.get(tr + "/sample-preview?" + urllib.parse.urlencode({"draft_id": draft_id, "request_id": uid()}))
    budget = sample_preview["conversation_budget"]
    sample = c.post("/api/projects/" + scene["project"] + "/sample-operations", {"request_id": sample_preview["request_id"], "draft_id": draft_id, "expected_revision": sample_preview["revision"], "image_indices": [0, 1, 2], "authorization_fingerprint": sample_preview["authorization_fingerprint"], "conversation": {"conversation_id": scene["conversation_id"], "task_id": scene["task_id"], **{key: budget[key] for key in ["previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True, "human_review": True}})
    sample = c.poll("/api/projects/" + scene["project"] + "/sample-operations/" + sample["id"], lambda v: v["status"] in ["succeeded", "failed"])
    assert sample["status"] == "succeeded", sample
    record = c.get(f"/api/workflow-drafts/{draft_id}/sample-test?test_id={sample['id']}")["sample_test"]
    assert record["report"]["summary"]["failed_count"] == 0, record
    assert len(c.get(tr + "/calls")) > len(after_calls)
    final_humans = c.get(tr + "/human-requests")
    for other in others:
        assert next(r for r in final_humans if r["input"]["id"] == other["input"]["id"]) == other
    (workspace / "UIAPI014_REPAIR_HTTP_TRACE.json").write_text(json.dumps({"scene": scene, "result": result, "pending_others": others, "trace": c.trace}, indent=2))
    print("PASS real HTTP Applied Builder + separately authorized Sample admitted; two other images pending unchanged; exact Builder replay adds zero calls")


if __name__ == "__main__":
    main()
