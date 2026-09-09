"""TEST-only delivery scenario, using real Rust HTTP services and scripted model output.

No training, weights, user images or paid Provider. Deliberate test Draft edits are
recorded; they are not evidence of autonomous Builder model selection.
"""
import json
import struct
import urllib.parse
import zlib

from http_smoke import uid


def test_png(index):
    width, height = 1000, 500
    rows = []
    boxes = [(120, 100, 160, 110), (500, 275, 160, 110), (720, 75, 120, 125)]
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            color = (230 - index, 235 - index, 240 - index)
            if index != 10:
                for n, (bx, by, bw, bh) in enumerate(boxes):
                    if bx <= x < bx + bw and by <= y < by + bh:
                        color = [(220, 170, 30), (220, 170, 30), (40, 120, 180)][n]
            row.extend(color)
        rows.append(row)
    def chunk(kind, payload):
        data = kind + payload
        return struct.pack(">I", len(payload)) + data + struct.pack(">I", zlib.crc32(data))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b"")


def seed_delivery(c, provider):
    assert provider["display_name"] == "TEST integration external HTTP"
    assert urllib.parse.urlparse(provider["base_url"]).hostname == "127.0.0.1"
    project = "TEST-delivery-" + uid()
    p = "/api/projects/" + project
    c.post("/api/projects", {"id": project, "yaml": "version: 1\nproject:\n  name: TEST original-image delivery\ndataset:\n  root: images\nruntime:\n  max_parallel_images: 1\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"})
    model = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST delivery scripted detection", "remote_model_id": "e2e-conversation-bbox-feedback-image-class", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    c.request("PUT", p + "/model-bindings", {"bindings": [{"capability": "vision_language", "role": "detection", "match_kind": "capability", "model_profile_id": model["id"], "locked": False}]})
    for index in range(12):
        c.request("POST", p + f"/image-upload?name=TEST-delivery-{index:02}.png", test_png(index))
    images = c.get(p + "/images")["images"]
    conversation = c.post(p + "/conversations")["conversation_id"]
    cr = p + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": model["id"]})
    sent = c.post(cr + "/send", {"message": {"id": uid(), "text": "TEST: label cups and bottles with bounding boxes for Ultralytics YOLO Detection; inspect whole images before delivering originals and a validated training ZIP.", "image": None}, "schema_revision": c.get(p + "/goal")["revision"], "mode": "plan"})
    tr = cr + "/tasks/" + sent["task_id"]
    intent = c.post(tr + "/delivery-intent", {"command_id": uid(), "expected_revision": 0, "image_ids": [i["image_id"] for i in images], "label_spec": [{"stable_id": label, "display_name": name, "aliases": [], "include": "TEST geometric proxy, not a quality claim", "exclude": ""} for label, name in [("cup", "杯子"), ("bottle", "瓶子")]], "training_target": {"annotation_kind": "bounding_box", "framework": "ultralytics", "export_profile": "ultralytics_yolo_detection", "profile_revision": 1}, "split_policy": {"train_percent": 80, "seed": 7, "preserve_existing": True, "keep_known_groups_together": True}})["saved"]
    schema = c.post(tr + "/delivery-schema", {"command_id": uid(), "expected_revision": intent["revision"], "expected_sha256": intent["content_sha256"]})
    preview = c.get(tr + "/builder-preview?" + urllib.parse.urlencode({"operation_id": uid(), "schema_id": schema["id"], "schema_revision": schema["revision"], "model_id": model["id"]}))
    builder = c.post(tr + "/builder-operations", {**{key: preview[key] for key in ["selection", "repair", "previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True})
    assert builder["status"] == "completed", builder
    draft_id = builder["evidence"]["draft_id"]
    draft = next(d for d in c.get("/api/workflow-drafts?project_id=" + project)["drafts"] if d["id"] == draft_id)
    # Explicit TEST integration correction: the scripted Builder selects a generic
    # template; bind its shared detector to the real VLM protocol under test.
    for node in [*draft["nodes"], *[s for stage in draft["label_pipeline"]["shared_stages"] for s in stage["steps"]]]:
        if node["id"] == "shared.detector":
            node.update(node_type="vlm_detection.detect", kind="vision_language_model", parameters={"labels": ["cup", "bottle"], "target_description": "TEST cups and bottles"}, model_binding=None, model_profile_binding={"model_profile_id": model["id"], "locked": True})
    c.request("PATCH", "/api/workflow-drafts/" + draft_id, draft, extra_headers={"if-match": str(draft["revision"])})
    preview = c.get(tr + "/sample-preview?" + urllib.parse.urlencode({"draft_id": draft_id, "request_id": uid()}))
    budget = preview["conversation_budget"]
    sample = c.post(p + "/sample-operations", {"request_id": preview["request_id"], "draft_id": draft_id, "expected_revision": preview["revision"], "image_indices": [0, 1, 2], "authorization_fingerprint": preview["authorization_fingerprint"], "conversation": {"conversation_id": conversation, "task_id": sent["task_id"], **{key: budget[key] for key in ["previous_grant_id", "scope_hash", "expires_at"]}, "allow_unknown_cost": True, "human_review": True}})
    c.poll(p + "/sample-operations/" + sample["id"], lambda v: v["status"] == "succeeded" and (v.get("assistance") or {}).get("status") == "completed")
    selection = {"draft_id": draft_id, "sample_test_id": sample["id"], "limit": 12}
    authorization = c.get(p + "/processing-preview?" + urllib.parse.urlencode(selection))
    operation = c.post(p + "/processing-operations", {"request_id": uid(), "selection": selection, "expected_revision": authorization["revision"], "authorization_fingerprint": authorization["authorization_fingerprint"]})
    batch_url = "/api/batches/" + operation["batch_id"]
    batch = c.poll(batch_url, lambda v: v["batch"]["status"] in ["completed", "completed_with_review", "budget_exceeded", "failed", "cancelled"])
    return {"project": project, "conversation_id": conversation, "task_id": sent["task_id"], "task_root": tr, "model_profile_id": model["id"], "schema_id": schema["id"], "draft_id": draft_id, "sample_test_id": sample["id"], "batch_url": batch_url, "batch": batch, "images": images, "delivery": intent, "limitations": ["TEST scripted outputs, not model accuracy", "explicit test Draft model-node correction", "whole-image and object reviews not yet performed"]}


if __name__ == "__main__":
    import argparse
    import tempfile
    import urllib.request
    from pathlib import Path
    from http_smoke import Client
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-manifest", required=True, type=Path)
    parser.add_argument("--recover-trace", type=Path, help="Read an existing local TEST trace; never repeats model or processing commands")
    args = parser.parse_args()
    workspace = args.fixture_manifest.resolve().parent
    assert workspace.is_relative_to(Path(tempfile.gettempdir()).resolve())
    assert workspace.name.startswith("TEST-agent-ui-")
    assert (workspace / "FIXTURE_ONLY").read_text() == "AnnotAgent HTTP integration fixture\n"
    manifest = json.loads(args.fixture_manifest.read_text())
    base = urllib.parse.urlparse(manifest["base_url"])
    assert base.hostname == "127.0.0.1" and base.port not in [8787, 8788]
    with urllib.request.urlopen(manifest["base_url"] + "/api/health") as response:
        assert response.headers.get("x-annotagent-fixture") == "external-model-only"
    client = Client(manifest["base_url"])
    provider = next(p for p in client.get("/api/providers")["providers"] if p["id"] == manifest["provider_id"])
    record = workspace / ("DELIVERY_SCENE_" + uid() + ".json")
    try:
        if args.recover_trace:
            trace_path = args.recover_trace.resolve()
            assert trace_path.parent == workspace and trace_path.name.startswith("DELIVERY_SCENE_")
            trace = json.loads(trace_path.read_text())
            selection = next(r for r in trace if r["method"] == "POST" and r["path"].endswith("/processing-operations"))
            operation = selection["response"]
            project = operation["project_id"]
            assert project.startswith("TEST-delivery-")
            context = operation["authorization"]["conversation"]
            conversation, task = context["conversation_id"], context["task_id"]
            p = "/api/projects/" + project
            tr = p + "/conversations/" + conversation + "/tasks/" + task
            batch_url = "/api/batches/" + operation["batch_id"]
            scene = {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "schema_id": context["schema"]["id"], "draft_id": operation["draft_id"], "sample_test_id": operation["sample_test_id"], "batch_url": batch_url, "batch": client.get(batch_url), "images": client.get(p + "/images")["images"], "delivery": client.get(tr + "/delivery-intent")["saved"], "limitations": ["Read-only recovery from existing TEST operation; no inference repeated", "TEST scripted outputs and explicit Draft correction", "budget-exceeded images remain incomplete; no review or export inferred"]}
        else:
            scene = seed_delivery(client, provider)
        with record.open("x") as out:
            json.dump(scene, out, indent=2, ensure_ascii=False)
        print(json.dumps({"scene": str(record), "project": scene["project"], "task": scene["task_id"], "batch_status": scene["batch"]["batch"]["status"]}), flush=True)
    finally:
        with record.with_suffix(".trace.json").open("x") as out:
            json.dump(client.trace, out, indent=2, ensure_ascii=False)
        print("TEST trace: " + str(record.with_suffix(".trace.json")), flush=True)
