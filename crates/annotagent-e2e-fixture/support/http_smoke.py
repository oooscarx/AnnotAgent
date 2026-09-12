"""Real HTTP seed/checks for http_fixture.py. External inference only is synthetic."""
import datetime
import hashlib
import http.cookiejar
import io
import json
from pathlib import Path
import struct
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zlib
import zipfile


def uid():
    return str(uuid.uuid4())


class Client:
    def __init__(self, base):
        self.base = base
        self.http = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()), urllib.request.ProxyHandler({}))
        self.trace = []
        self.csrf = self.request("GET", "/api/session")["csrf_token"]

    def request(self, method, path, data=None, expected=None, raw=False, extra_headers=None, _retry_deadline=None):
        headers = dict(extra_headers or {})
        if method != "GET":
            headers["x-annotagent-csrf"] = self.csrf
            clean = path.split("?")[0]
            if method == "DELETE" or clean == "/api/settings" or "/credential" in clean or clean.endswith("/active-probe"):
                token = self.request("POST", "/api/session/privileged-confirmation", {"action": f"{method} {clean}", "confirmed": True})
                headers["x-annotagent-privileged-confirmation"] = token["confirmation_token"]
        if isinstance(data, bytes):
            body = data
            headers["content-type"] = "image/png"
        elif data is not None:
            body = json.dumps(data).encode()
            headers["content-type"] = "application/json"
        else:
            body = None
        request = urllib.request.Request(self.base + path, data=body, headers=headers, method=method)
        try:
            response = self.http.open(request, timeout=100)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            status, payload = response.status, response.read()
            if raw:
                result = {"bytes": len(payload), "content_type": response.headers.get("content-type")}
            else:
                try:
                    result = json.loads(payload)
                except ValueError:
                    result = payload.decode(errors="replace")
        if path != "/api/session" and "privileged-confirmation" not in path and "/credential" not in path:
            self.trace.append({"method": method, "path": path, "status": status, "request": data if not isinstance(data, bytes) else {"binary_bytes": len(data)}, "response": result})
        if method != "GET" and status == 429 and isinstance(result, dict) and result.get("code") == "mutation_rate_limited":
            deadline = _retry_deadline or time.monotonic() + 65
            if time.monotonic() < deadline:
                time.sleep(1)
                return self.request(method, path, data, expected, raw, extra_headers, deadline)
        assert status in (expected or range(200, 300)), (method, path, status, result)
        return result

    def get(self, path):
        return self.request("GET", path)

    def post(self, path, data=None):
        return self.request("POST", path, {} if data is None else data)

    def download(self, path):
        request = urllib.request.Request(self.base + path, method="GET")
        with self.http.open(request, timeout=100) as response:
            payload = response.read()
            result = {
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
                "content_type": response.headers.get("content-type"),
            }
            self.trace.append({"method": "GET", "path": path, "status": response.status, "request": None, "response": result})
            assert response.status == 200, (path, response.status)
            return payload, result

    def poll(self, path, predicate):
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            result = self.get(path)
            if predicate(result):
                return result
            time.sleep(0.3)
        raise AssertionError(("poll timeout", path, result))


def review_formal_results_and_package(c, tr, task_images):
    page = c.get(tr + "/delivery-review-items?limit=10")
    items = page["items"]
    assert len(items) == len(task_images) == 6, page
    accepted_objects = 0
    for item in items:
        assert item["child_run_id"] and item["execution_status"] in ["completed", "awaiting_review"], item
        image_path = tr + "/delivery-images/" + item["image_id"]
        source_query = "?" + urllib.parse.urlencode({"source_run_id": item["child_run_id"]})
        view = c.get(image_path + source_query)
        for annotation in item["annotations"]:
            assert annotation["review_status"] == "needs_review", annotation
            c.post(image_path + "/objects", {
                "command_id": uid(),
                "intent_revision": page["intent_revision"],
                "intent_sha256": page["intent_sha256"],
                "source_run_id": item["child_run_id"],
                "annotation_id": annotation["annotation_id"],
                "expected_snapshot_sha256": view["snapshot"]["sha256"],
                "label": annotation["label"],
                "value": annotation["value"],
                "review_status": "human_accepted",
                "reason": "TEST exact formal candidate accepted after inspection",
            })
            accepted_objects += 1
            view = c.get(image_path + source_query)
        assert view["unresolved_objects"] == 0, view
        c.post(image_path, {
            "command_id": uid(),
            "intent_revision": page["intent_revision"],
            "intent_sha256": page["intent_sha256"],
            "image_id": item["image_id"],
            "source_run_id": item["child_run_id"],
            "expected_snapshot_sha256": view["snapshot"]["sha256"],
            "expected_review_revision": view["review"]["revision"] if view["review"] else 0,
            "decision": "positive_complete",
            "reason": None,
            "confirmed": True,
        })

    ready_for_package = c.get(tr + "/workspace")["mainline"]
    assert [action["id"] for action in ready_for_package["available_actions"]] == ["authorize_training_package"], ready_for_package
    package_id = uid()
    consent_body = {
        "id": package_id,
        "intent_revision": page["intent_revision"],
        "intent_sha256": page["intent_sha256"],
        "confirmed": True,
    }
    armed = c.post(tr + "/delivery-package-consents", consent_body)
    assert armed["state"] == "consumed" and armed["effective_state"] == "consumed", armed
    assert armed["job"]["id"] == package_id and armed["readiness"]["confirmed_images"] == 6, armed
    package_path = tr + "/delivery-packages/" + package_id
    settled = c.poll(package_path, lambda value: not value["active"])
    assert settled["job"]["phase"] == "ready", settled
    result = settled["job"]["result"]
    assert result["images"] == 6 and result["objects"] == accepted_objects, result
    assert result["negatives"] == 0 and result["excluded"] == 0, result

    replay = c.post(tr + "/delivery-package-consents", consent_body)
    assert replay["state"] == "consumed" and replay["job"]["id"] == package_id, replay
    consents = c.get(tr + "/delivery-package-consents")
    matching = [item for item in consents["items"] if item["input"]["id"] == package_id]
    assert len(matching) == 1 and matching[0]["job"]["id"] == package_id, consents

    payload, transport = c.download(package_path + "/download")
    assert transport["content_type"] == "application/zip", transport
    assert transport["sha256"] == result["sha256"] and transport["bytes"] == result["bytes"], result
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        names = archive.namelist()
        assert all(not name.startswith("/") and ".." not in name.split("/") for name in names), names
        required = {"data.yaml", "annotagent/manifest.json", "annotagent/split-manifest.json", "annotagent/validation-report.json"}
        assert required.issubset(names), names
        package_manifest = json.loads(archive.read("annotagent/manifest.json"))
        validation = json.loads(archive.read("annotagent/validation-report.json"))
        for name, evidence in package_manifest["files"].items():
            content = archive.read(name)
            assert hashlib.sha256(content).hexdigest() == evidence["sha256"], (name, evidence)
            assert len(content) == evidence["bytes"], (name, evidence)
        expected_hashes = {item["image_id"]: item["sha256"] for item in task_images}
        for image in package_manifest["images"]:
            assert image["image_id"] in expected_hashes and image["image"], image
            assert hashlib.sha256(archive.read(image["image"])).hexdigest() == expected_hashes[image["image_id"]], image
        label_rows = sum(len(archive.read(name).decode().splitlines()) for name in names if name.startswith("labels/") and name.endswith(".txt"))
        assert label_rows == accepted_objects, (label_rows, accepted_objects)
        assert package_manifest["delivery_revision"] == page["intent_revision"]
        assert package_manifest["lineage"]["package_id"] == package_id
        assert validation["publication"].startswith("This archive is published only after"), validation

    completed = c.get(tr + "/workspace")["mainline"]
    assert completed["completion"]["task_completed"] is True, completed
    assert completed["available_actions"] == [], completed
    return {
        "formal_images_reviewed": len(items),
        "formal_objects_human_accepted": accepted_objects,
        "package_consent_id": package_id,
        "package_consent_state": replay["state"],
        "package_job_id": settled["job"]["id"],
        "package_phase": settled["job"]["phase"],
        "package_sha256": result["sha256"],
        "package_bytes": result["bytes"],
        "zip_entry_count": len(names),
        "zip_validation_checks": validation["checks"],
        "duplicate_job_count": len(matching),
    }


def seed_and_verify(manifest, root):
    c = Client(manifest["base_url"])
    evidence = Path(manifest["workspace"]) / "HTTP_TRACE.json"
    try:
        return verify(c, manifest, root)
    finally:
        evidence.write_text(json.dumps({"fixture": "TEST external-model-only; real application HTTP and database", "requests": c.trace}, indent=2, ensure_ascii=False) + "\n")


def verify_demo_onboarding(c, model_profile_id):
    catalog = c.get("/api/demo-catalog?limit=1")
    assert catalog["contract_version"] == "demo-catalog-v1" and len(catalog["items"]) == 1, catalog
    entry = catalog["items"][0]
    assert entry["demo_id"] == "object-detection-review" and entry["image_count"] == 6, entry
    manifest = c.get("/api/demo-catalog/object-detection-review/versions/1.0.0")
    assert manifest["manifest_sha256"] == entry["manifest_sha256"], manifest
    assert all("path" not in asset and asset["download_url"].startswith("/api/demo-catalog/") for asset in manifest["assets"]), manifest
    thumbnail = c.request("GET", entry["thumbnail_url"], raw=True)
    assert thumbnail["bytes"] > 0 and thumbnail["content_type"] == "image/png", thumbnail

    preset_command = uid()
    preset_request = {"command_id": preset_command, "demo_id": entry["demo_id"], "demo_version": entry["version"], "source_mode": "preset_candidates", "model_profile_id": None}
    preset = c.post("/api/demos/start", preset_request)
    assert preset["status"] == "ready" and preset["source_provenance"]["review_status"] == "needs_review", preset
    assert preset["source_provenance"]["live_inference_occurred"] is False, preset
    replay = c.post("/api/demos/start", preset_request)
    assert replay["replayed"] is True and replay["project_id"] == preset["project_id"] and replay["task_id"] == preset["task_id"], replay
    conflict = c.request("POST", "/api/demos/start", {**preset_request, "source_mode": "live_model", "model_profile_id": model_profile_id}, expected=[409])
    assert conflict["code"] == "demo_command_conflict", conflict
    recovered = c.get("/api/demos/start/" + preset_command)
    assert recovered["task_id"] == preset["task_id"] and recovered["replayed"] is True, recovered
    preset_task = f"/api/projects/{preset['project_id']}/conversations/{preset['conversation_id']}/tasks/{preset['task_id']}"
    assert c.get(preset_task + "/model-usage")["attempts"]["items"] == [], preset
    delivery = c.get(preset_task + "/delivery-intent")
    assert len(delivery["saved"]["intent"]["dataset_scope"]) == 6, delivery
    preset_workspace = c.get(preset_task + "/workspace")["mainline"]
    assert preset_workspace["task_id"] == preset["task_id"]
    assert preset_workspace["available_actions"][0]["id"] == "review_delivery_images", preset_workspace
    assert preset_workspace["formal_source"] == {"kind": "preset_candidate_import", "status": "needs_review", "live_inference_occurred": False, "model_run_id": None}, preset_workspace
    review = c.get(preset_task + "/delivery-review-items?limit=6")
    assert review["summary"] == {"selected": 6, "positive": 0, "negative": 0, "excluded": 0, "unreviewed": 6}, review
    first = review["items"][0]
    assert first["child_run_id"] is None and first["annotations"][0]["origin"] == "preset_candidate", first
    assert first["annotations"][0]["review_status"] == "needs_review" and first["annotations"][0]["source_artifact_id"], first
    for item in review["items"]:
        image_path = preset_task + "/delivery-images/" + item["image_id"]
        view = c.get(image_path)
        for annotation in list(view["snapshot"]["annotations"]):
            c.post(image_path + "/preset-objects", {
                "command_id": uid(),
                "intent_revision": review["intent_revision"],
                "intent_sha256": review["intent_sha256"],
                "annotation_id": annotation["id"],
                "expected_snapshot_sha256": view["snapshot"]["sha256"],
                "label": annotation["label"],
                "value": annotation["value"],
                "review_status": "human_accepted",
                "reason": "TEST human inspected imported candidate",
            })
            view = c.get(image_path)
        decision = "negative_confirmed" if not view["snapshot"]["annotations"] else "positive_complete"
        c.post(image_path, {
            "command_id": uid(),
            "intent_revision": review["intent_revision"],
            "intent_sha256": review["intent_sha256"],
            "image_id": item["image_id"],
            "source_run_id": None,
            "expected_snapshot_sha256": view["snapshot"]["sha256"],
            "expected_review_revision": 0,
            "decision": decision,
            "reason": None,
            "confirmed": True,
        })
    reviewed = c.get(preset_task + "/delivery-review-items?limit=6")
    assert reviewed["summary"] == {"selected": 6, "positive": 5, "negative": 1, "excluded": 0, "unreviewed": 0}, reviewed
    ready_workspace = c.get(preset_task + "/workspace")["mainline"]
    assert [item["id"] for item in ready_workspace["available_actions"]] == ["authorize_training_package"], ready_workspace
    assert ready_workspace["result_diagnostics"] == [], ready_workspace
    package_id = uid()
    c.post(preset_task + "/delivery-package-consents", {
        "id": package_id,
        "intent_revision": review["intent_revision"],
        "intent_sha256": review["intent_sha256"],
        "confirmed": True,
    })
    package_path = preset_task + "/delivery-packages/" + package_id
    package = c.poll(package_path, lambda value: not value["active"])
    assert package["job"]["phase"] == "ready" and package["job"]["result"]["images"] == 6, package
    archive, archive_evidence = c.download(package_path + "/download")
    assert archive_evidence["content_type"] == "application/zip", archive_evidence
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        package_manifest = json.loads(bundle.read("annotagent/manifest.json"))
    lineage = list(package_manifest["lineage"]["images"].values())
    assert len(lineage) == 6 and all(item["source_kind"] == "preset_candidate" and item["source_run_id"] is None and item["source_evidence_sha256"] for item in lineage), lineage
    completed_workspace = c.get(preset_task + "/workspace")["mainline"]
    assert completed_workspace["completion"]["status"] == "package_ready", completed_workspace
    assert completed_workspace["available_actions"] == [] and completed_workspace["result_diagnostics"] == [], completed_workspace

    live_command = uid()
    live = c.post("/api/demos/start", {"command_id": live_command, "demo_id": entry["demo_id"], "demo_version": entry["version"], "source_mode": "live_model", "model_profile_id": model_profile_id})
    assert live["source_provenance"] == {"kind": "live_model", "live_inference_occurred": False, "review_status": None, "source_asset_id": None, "source_asset_sha256": None}, live
    live_task = f"/api/projects/{live['project_id']}/conversations/{live['conversation_id']}/tasks/{live['task_id']}"
    assert c.get(live_task + "/model-usage")["attempts"]["items"] == [], live
    assert c.get(live_task + "/calls") == [], live
    assert c.get(f"/api/projects/{live['project_id']}/conversations/{live['conversation_id']}/agent-model")["model_profile_id"] == model_profile_id
    return {"catalog_revision": catalog["catalog_revision"], "manifest_sha256": entry["manifest_sha256"], "preset": preset, "live": live, "preset_task_root": preset_task, "live_task_root": live_task, "preset_package_id": package_id, "preset_package": package["job"], "preset_package_download": archive_evidence}


def verify_diagnostic_scenes(c, manifest):
    seeded = json.loads((Path(manifest["workspace"]) / "P0_DIAGNOSTIC_SCENES.json").read_text())
    assert seeded["contract_version"] == "p0-diagnostic-scenes-v1", seeded
    observed = {}
    for code, scene in seeded["scenes"].items():
        workspace = c.get(scene["workspace_url"])
        matches = [item for item in workspace["mainline"]["result_diagnostics"] if item["code"] == code]
        assert len(matches) == 1, (code, workspace["mainline"]["result_diagnostics"])
        diagnostic = matches[0]
        assert diagnostic["state"] == scene["expected"]["state"], diagnostic
        assert diagnostic["automatic_retry"] is False, diagnostic
        assert diagnostic["preserves_existing_results"] is True, diagnostic
        assert diagnostic["safe_action"]["method"] == "GET", diagnostic
        if scene.get("draft_url"):
            draft = c.get(scene["draft_url"])
            assert draft["id"] == scene["draft_id"] and draft["project_id"] == scene["project_id"], draft
            sample = c.get(scene["sample_test_url"])
            assert sample["sample_test"]["id"] == scene["sample_test_id"], sample
            assert sample["sample_test"]["draft_id"] == scene["draft_id"] and sample["current"] is True, sample
            inputs, results = sample["sample_test"]["inputs"], sample["sample_test"]["report"]["samples"]
            assert len(inputs) == len(results) == 1, sample
            assert inputs[0]["image_id"] == scene["image"]["image_id"], (inputs, scene["image"])
            assert inputs[0]["content_hash"] == scene["image"]["content_hash"], (inputs, scene["image"])
            assert results[0]["image_name"] == scene["image"]["name"], (results, scene["image"])
        observed[code] = {**scene, "diagnostic": diagnostic}
    return {**seeded, "scenes": observed}


def verify(c, manifest, root):
    # Exercise the real middleware: an uncredentialed mutation is rejected.
    request = urllib.request.Request(c.base + "/api/projects", data=b"{}", headers={"content-type": "application/json"}, method="POST")
    try:
        c.http.open(request, timeout=5)
        raise AssertionError("mutation without CSRF unexpectedly accepted")
    except urllib.error.HTTPError as error:
        assert error.code == 403
        c.trace.append({"method": "POST", "path": "/api/projects", "status": error.code, "request": {}, "response": json.loads(error.read())})
    provider = c.post("/api/providers", {"display_name": "TEST integration external HTTP", "adapter": "open_ai_compatible", "base_url": manifest["provider_url"]})
    c.post(f"/api/providers/{provider['id']}/credential", {"source": "session_only", "secret": "TEST-integration-only"})
    model = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST deterministic model", "remote_model_id": "e2e-conversation-bbox-feedback-image-class", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    defaults = c.get("/api/agent-model-bindings")
    c.request("PUT", "/api/agent-model-bindings", {**defaults, "pipeline_builder": model["id"]})
    project = "TEST-agent-ui-" + uid()
    c.post("/api/projects", {"id": project, "yaml": "version: 1\nproject:\n  name: TEST Agent UI HTTP fixture\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"})
    p = f"/api/projects/{project}"
    c.request("PUT", p + "/model-bindings", {"bindings": [{"capability": "vision_language", "role": "primary_inference", "match_kind": "role", "model_profile_id": model["id"], "locked": True}]})
    png = (root / "examples/robocup/images/synthetic-robocup.png").read_bytes()
    task_images = []
    for index in range(6):
        chunk = b"tEXt" + f"TEST\x00p0-autonomy-{index}".encode()
        image = png[:-12] + struct.pack(">I", len(chunk) - 4) + chunk + struct.pack(">I", zlib.crc32(chunk)) + png[-12:]
        imported = c.request("POST", p + f"/image-upload?name=TEST-p0-{index}.png", image)
        assert len(imported["images"]) == 1, imported
        task_images.append({"image_id": imported["images"][0]["image_id"], "sha256": imported["images"][0]["content_hash"]})
    conversation = c.post(p + "/conversations")["conversation_id"]
    cr = p + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": model["id"]})
    schema_revision = c.get(p + "/goal")["revision"]
    command = {"message": {"id": uid(), "text": "标注这些图片中的杯子和瓶子，框住完整可见物体，用于 Ultralytics YOLO 目标检测。先给我看三张样例。", "image": None}, "task_images": task_images, "schema_revision": schema_revision, "mode": "plan"}
    receipt = c.post(cr + "/send", command)
    assert c.post(cr + "/send", command) == receipt
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    c.get("/api/navigation?limit=1")
    c.get(cr + "/task-navigation?limit=1")
    thread = c.get(tr + "/thread?limit=1")
    assert thread["items"][0]["source"] == "persisted_user_message"
    workspace = c.get(tr + "/workspace")
    assert workspace["calls"] == [] and workspace["sample_operations"] == []
    actions = workspace["mainline"]["available_actions"]
    assert len(actions) == 1 and actions[0]["id"] == "build_and_test_pipeline", actions
    assert actions[0]["url"] == tr + "/journey-preview"
    assert len(actions[0]["scope"]["images"]) == 6
    assert actions[0]["scope"]["maximum_sample_images"] == 3
    c.request("GET", cr + "/tasks/" + uid() + "/workspace", expected=[400, 404])
    c.get(tr + "/message-queue")
    settings = c.get("/api/settings?view=agent-ui")
    budget = {**settings["sections"]["usage_budget"]["future_run_budget"], "max_requests": 11}
    c.request("PATCH", "/api/settings", {"expected_revision": settings["revision"], "budget": budget})
    c.request("PATCH", "/api/settings", {"expected_revision": "TEST-stale", "budget": budget}, expected=[409])
    # One explicit consent starts the bounded durable queue. GET is passive and
    # the fixture never calls the legacy execution POST for this P0 path.
    consent = c.get(tr + "/journey-preview")["consent"]
    assert len(consent["images"]) == 3
    consent["allow_unknown_cost"] = True
    consent["schema_proposal"]["allow_unknown_cost"] = True
    consent_started = time.monotonic()
    admitted = c.post(tr + "/journey-consents", consent)
    consent_response_ms = round((time.monotonic() - consent_started) * 1000)
    assert consent_response_ms < 3000, admitted
    execution = tr + "/journey-consents/" + consent["id"] + "/execution"
    first_observation = c.get(execution)
    assert first_observation["dispatch"]["status"] in ["queued", "running"], first_observation
    assert first_observation.get("sample") is None, first_observation
    finished = c.poll(execution, lambda v: (v.get("sample") or {}).get("assistance", {}).get("status") == "completed" or ((v.get("dispatch") or {}).get("status") == "settled" and (v.get("dispatch") or {}).get("error")))
    journey_duration_ms = round((time.monotonic() - consent_started) * 1000)
    assert journey_duration_ms >= 3000, finished
    assert finished.get("sample") and finished["sample"]["status"] == "succeeded", finished
    assert finished["sample"]["id"] == consent["sample_operation_id"], finished
    record = c.get(f"/api/workflow-drafts/{finished['sample']['draft_id']}/sample-test?test_id={consent['sample_operation_id']}")["sample_test"]
    assert 0 < len(record["inputs"]) <= 3
    delivery_schema = c.get(tr + "/delivery-schema")
    assert delivery_schema["schema"] is not None, delivery_schema
    visual = c.get(tr + "/visual-selections?limit=10")
    assert len(visual["items"]) == 1, visual
    assert len(visual["items"][0]["images"]) == 3, visual
    assert all(image["candidates"] for image in visual["items"][0]["images"]), visual
    automatic_reviews = [item for item in c.get(tr + "/human-requests") if item["status"] == "pending" and item["input"]["reason_code"] == "terminal_result_requires_review"]
    assert len(automatic_reviews) == 3, automatic_reviews
    review_workspace = c.get(tr + "/workspace")["mainline"]
    assert review_workspace["review_work_item_id"] == task, review_workspace
    assert review_workspace["sample_review"]["pending_count"] == 3, review_workspace
    assert [action["id"] for action in review_workspace["available_actions"]] == ["review_sample_results"], review_workspace
    # Delivery processing owns the complete six-image Task scope. The prior
    # Journey consent covered only the three-image sample and cannot be reused
    # as a hidden limit on formal processing.
    selection = {"draft_id": record["draft_id"], "sample_test_id": record["id"]}
    blocked_preview = c.request("GET", p + "/processing-preview?" + urllib.parse.urlencode(selection), expected=[409])
    assert blocked_preview["code"] == "sample_reviews_pending" and blocked_preview["admitted"] is False, blocked_preview
    assert len(blocked_preview["sample_review"]["unresolved"]) == 3, blocked_preview
    blocked_command = uid()
    blocked_processing = c.request("POST", p + "/processing-operations", {"request_id": blocked_command, "selection": selection, "expected_revision": record["draft_revision"], "authorization_fingerprint": "0" * 64}, expected=[409])
    assert blocked_processing["code"] == "sample_reviews_pending" and blocked_processing["admitted"] is False, blocked_processing
    assert c.get(tr + "/processing-operations") == []
    first_review_answer = None
    for request in automatic_reviews:
        request_input = request["input"]
        sample_index = next(index for index, image in enumerate(record["inputs"]) if image["image_id"] == request_input["image_id"])
        outcome = next(outcome for outcome in record["report"]["samples"][sample_index]["outcomes"] if outcome["id"] == request_input["outcome_id"])
        answer = {
            "revision_id": uid(), "sample_test_id": record["id"], "image_id": request_input["image_id"],
            "sequence": request_input["expected_feedback_sequence"] + 1, "reason": "correct",
            "outcome_id": request_input["outcome_id"], "corrected_value": outcome["value"],
            "corrected_label": outcome["label"], "note": "TEST reviewed exact saved sample candidate",
            "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()
        }
        applied = c.post(tr + "/human-requests/" + request_input["id"] + "/answer", {"answer": answer})
        assert applied["status"] == "applied" and applied["input"]["sample_test_id"] == record["id"], applied
        if first_review_answer is None:
            first_review_answer = (request_input, answer, applied)
    approval = c.get(p + "/processing-preview?" + urllib.parse.urlencode(selection))
    assert approval["sample_review"]["ready"] is True
    assert len(approval["sample_review"]["applied_request_ids"]) == 3
    processing = c.post(p + "/processing-operations", {"request_id": uid(), "selection": selection, "expected_revision": approval["revision"], "authorization_fingerprint": approval["authorization_fingerprint"]})
    batch = c.poll("/api/batches/" + processing["batch_id"], lambda value: value["batch"]["status"] not in ["pending", "running", "pausing"])
    assert batch["batch"]["status"] in ["completed", "awaiting_review"], batch
    run_id = batch["batch"]["child_run_ids"][0]
    verify_sse(c, run_id)
    c.get("/api/runs/" + run_id + "/result-summary")
    c.get("/api/runs/" + run_id + "/debug-summary")
    human, answer, saved = first_review_answer
    answer_path = tr + "/human-requests/" + human["id"] + "/answer"
    replay = c.post(answer_path, {"answer": answer})
    assert replay["answer"] == saved["answer"]
    c.request("POST", answer_path, {"answer": {**answer, "note": "conflicting retry"}}, expected=[400, 409])
    c.post(tr + "/human-requests/" + human["id"] + "/resume")
    c.get(tr + "/human-requests")
    # FIFO supplements are saved by Send and only dispatched by exact consent.
    supplements = []
    for text in ["TEST also classify indoors", "TEST cancel this supplement"]:
        queued = {"message": {"id": uid(), "text": text, "image": None}, "task_id": task, "schema_revision": schema_revision, "mode": "plan"}
        c.post(cr + "/send", queued)
        supplements.append(queued["message"]["id"])
    queue_root = tr + "/message-queue/"
    c.post(queue_root + supplements[1] + "/cancel")
    preview = c.request("GET", queue_root + supplements[0] + "/schema-preview", expected=[200, 409])
    if preview.get("code") == "human_input_pending":
        assert preview["admitted"] is False
        assert preview["suggested_action"] == "answer_human_then_retry_same_command"
    else:
        queue_consent = {key: preview[key] for key in ["model_id", "scope_hash", "request_hash", "previous_grant_id", "maximum_calls", "expires_at"]}
        queue_consent.update(call_id=uid(), allow_unknown_cost=True)
        queued_receipt = c.post(queue_root + supplements[0] + "/schema-proposals", queue_consent)
        assert c.post(queue_root + supplements[0] + "/schema-proposals", queue_consent) == queued_receipt
        c.get(queue_root + supplements[0] + "/schema-authorization")
    c.get(tr + "/message-queue")
    c.get(tr + "/calls")
    c.get(tr + "/workspace")
    c.get(p + "/export-readiness")
    # The P0 delivery still has whole-image reviews. Export executes its real
    # readiness gate and must not turn model candidates into accepted labels.
    export = c.post(p + "/export", {"format": "native", "conversation": {"id": uid(), "conversation_id": conversation, "task_id": task}, "background": True})
    export = c.poll(tr + "/exports/" + export["job"]["id"], lambda value: not value["active"])
    assert "Resolve pending reviews" in export["job"]["error"], export
    assert export["job"]["result"] is None
    c.get(tr + "/exports")
    package_evidence = review_formal_results_and_package(c, tr, task_images)
    stop = verify_stop(c, cr, schema_revision, provider, model)
    pending = {**human, "id": uid(), "expected_feedback_sequence": 1, "question": "TEST pending answer for frontend adapter", "resume_checkpoint_ref": uid()}
    c.post(tr + "/human-requests", pending)
    plan = c.post(cr + "/send", {"message": {"id": uid(), "text": "TEST Plan only: awaiting explicit approval", "image": None}, "schema_revision": schema_revision, "mode": "plan"})
    builder_item = c.get(tr + "/workspace")["builder_operations"]["items"][0]
    assert builder_item["session"]["builder_proposal"]["draft"]["id"]
    saved_plan = {"task_id": task, "workspace_url": tr + "/workspace", "object_path": "builder_operations.items[0].session.builder_proposal", "operation_id": builder_item["operation"]["id"], "draft_id": builder_item["operation"]["evidence"]["draft_id"], "session_id": builder_item["session"]["id"]}
    from http_control_scenes import seed_controls
    controls = seed_controls(c, root, provider)
    from http_bbox_scene import seed_bbox
    bbox = seed_bbox(c, root, provider)
    from http_stop_scene import prepare_stop
    manual_stop = prepare_stop(c, cr, schema_revision, provider["id"], model["id"])
    describe_before_upload = verify_describe_before_upload(c, p, schema_revision, png)
    ambiguous_goal = verify_ambiguous_goal(c, p, schema_revision, provider, model, task_images[:3])
    first_page = c.get(cr + "/task-navigation?limit=1")
    assert first_page["next_cursor"] is not None
    second_page = c.get(cr + "/task-navigation?limit=1&cursor=" + str(first_page["next_cursor"]))
    assert first_page["items"][0]["task_id"] != second_page["items"][0]["task_id"]
    consent_posts = [entry for entry in c.trace if entry["method"] == "POST" and entry["path"] == tr + "/journey-consents"]
    execution_posts = [entry for entry in c.trace if entry["method"] == "POST" and entry["path"] == execution]
    assert len(consent_posts) == 1, consent_posts
    assert execution_posts == [], execution_posts
    diagnostic_scenes = verify_diagnostic_scenes(c, manifest)
    demo_onboarding = verify_demo_onboarding(c, model["id"])
    return {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "model_profile_id": model["id"], "provider_id": provider["id"], "execution_url": execution, "p0_autonomy": {"task_images": task_images, "task_image_count": len(task_images), "sample_image_count": len(record["inputs"]), "unavoidable_user_decisions": 1, "technical_relay_clicks": 0, "consent_post_count": len(consent_posts), "execution_post_count": len(execution_posts), "consent_response_ms": consent_response_ms, "journey_duration_ms": journey_duration_ms, "first_observation": first_observation, "consent_id": consent["id"], "schema_call_id": consent["schema_proposal"]["call_id"], "builder_operation_id": consent["builder_operation_id"], "sample_operation_id": consent["sample_operation_id"], "draft_id": record["draft_id"], "sample_status": record["status"], "delivery_schema_id": delivery_schema["schema"]["id"], "review_work_item_id": review_workspace["review_work_item_id"], "review_action": review_workspace["available_actions"][0], "automatic_review_request_ids": [item["input"]["id"] for item in automatic_reviews], "processing_review_gate": {"preview_code": blocked_preview["code"], "confirm_code": blocked_processing["code"], "receipt_count_before_reviews": 0, "unresolved_before": len(blocked_preview["sample_review"]["unresolved"]), "applied_after": len(approval["sample_review"]["applied_request_ids"]), "ready_after": approval["sample_review"]["ready"]}, "execution_dispatch": finished["dispatch"], "formal_delivery": package_evidence}, "describe_before_upload": describe_before_upload, "ambiguous_goal": ambiguous_goal, "export": export, "run_id": run_id, "stop": stop, "answered_request_id": human["id"], "pending_request_id": pending["id"], "plan_task_id": plan["task_id"], "controls": controls, "saved_plan": saved_plan, "bbox": bbox, "manual_stop": manual_stop, "diagnostic_scenes": diagnostic_scenes, "demo_onboarding": demo_onboarding, "trace": str(Path(manifest["workspace"]) / "HTTP_TRACE.json")}


def verify_describe_before_upload(c, project_root, schema_revision, png):
    conversation = c.post(project_root + "/conversations")["conversation_id"]
    cr = project_root + "/conversations/" + conversation
    sent = c.post(cr + "/send", {"message": {"id": uid(), "text": "标注杯子和瓶子，用于 Ultralytics YOLO 目标检测；图片稍后上传。", "image": None}, "schema_revision": schema_revision, "mode": "plan"})
    task = sent["task_id"]
    tr = cr + "/tasks/" + task
    before = c.get(tr + "/workspace")["mainline"]
    assert [action["id"] for action in before["available_actions"]] == ["save_delivery_intake"], before
    receipts = []
    for index in range(3):
        chunk = b"tEXt" + f"TEST\x00describe-first-{index}".encode()
        image = png[:-12] + struct.pack(">I", len(chunk) - 4) + chunk + struct.pack(">I", zlib.crc32(chunk)) + png[-12:]
        imported = c.request("POST", project_root + f"/image-upload?name=TEST-describe-first-{index}.png", image)
        receipts.append({"image_id": imported["images"][0]["image_id"], "sha256": imported["images"][0]["content_hash"]})
    command_id = uid()
    attach = {"command_id": command_id, "expected_revision": 0, "image_ids": None, "task_images": receipts, "label_spec": None, "training_target": None, "split_policy": {"train_percent": 80, "seed": 0, "preserve_existing": True, "keep_known_groups_together": True}, "image_metadata": {}}
    saved = c.post(tr + "/delivery-intent", attach)
    assert c.post(tr + "/delivery-intent", attach) == saved
    assert [(image["image_id"], image["content_sha256"]) for image in saved["saved"]["intent"]["dataset_scope"]] == [(image["image_id"], image["sha256"]) for image in receipts]
    changed = {**attach, "task_images": [{**receipts[0], "sha256": "f" * 64}, *receipts[1:]]}
    c.request("POST", tr + "/delivery-intent", changed, expected=[400, 409])
    after = c.get(tr + "/workspace")["mainline"]
    assert [action["id"] for action in after["available_actions"]] == ["build_and_test_pipeline"], after
    preview = c.get(tr + "/journey-preview")
    assert [(image["image_id"], image["content_hash"]) for image in preview["consent"]["images"]] == [(image["image_id"], image["sha256"]) for image in receipts]
    assert c.get(tr + "/thread?limit=10")["items"][0]["message"]["input"]["text"] == "标注杯子和瓶子，用于 Ultralytics YOLO 目标检测；图片稍后上传。"
    return {"conversation_id": conversation, "task_id": task, "task_root": tr, "delivery_revision": saved["saved"]["revision"], "delivery_sha256": saved["saved"]["content_sha256"], "task_images": receipts, "preview_consent_id": preview["consent"]["id"], "model_calls": 0, "execution_started": False}


def verify_ambiguous_goal(c, project_root, schema_revision, provider, visual_model, task_images):
    planner = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST one-question Schema planner", "remote_model_id": "e2e-conversation-clarify", "input_modalities": ["text"], "task_capabilities": ["text_generation"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": planner["id"], "confirmed_billable": True})
    conversation = c.post(project_root + "/conversations")["conversation_id"]
    cr = project_root + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": planner["id"]})
    command = {"message": {"id": uid(), "text": "标注这些图片中的杯子，训练 YOLO。先给我看三张样例。", "image": None}, "task_images": task_images, "schema_revision": schema_revision, "mode": "plan"}
    receipt = c.post(cr + "/send", command)
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    consent = c.get(tr + "/journey-preview")["consent"]
    assert consent["builder_model_id"] == planner["id"]
    assert [image["image_id"] for image in consent["images"]] == [image["image_id"] for image in task_images]
    consent["allow_unknown_cost"] = True
    consent["schema_proposal"]["allow_unknown_cost"] = True
    c.post(tr + "/journey-consents", consent)
    execution = tr + "/journey-consents/" + consent["id"] + "/execution"
    waiting = c.poll(execution, lambda value: (value.get("clarification") or {}).get("status") == "pending")
    clarification = c.get(tr + "/calls/" + consent["schema_proposal"]["call_id"] + "/clarification")
    assert clarification["id"] == consent["schema_proposal"]["call_id"]
    assert all(word in clarification["question"] for word in ["框", "轮廓", "分类"])
    assert [(choice["value"], choice["supported"]) for choice in clarification["choices"]] == [("bounding_box", True), ("segmentation", False), ("classification", False)]
    assert clarification["answer"]["method"] == "POST"
    assert waiting["builder"] is None and waiting["sample"] is None
    first_calls = c.get(tr + "/calls")
    assert len(first_calls) == 1 and first_calls[0]["id"] == clarification["id"], first_calls
    clarification_receipt = {key: value for key, value in clarification.items() if key not in ["choices", "answer"]}
    for _ in range(3):
        observed = c.get(execution)
        assert observed["clarification"] == clarification_receipt
    assert c.post(tr + "/journey-consents", consent)["consent"]["id"] == consent["id"]
    c.post(execution)
    assert c.get(execution)["clarification"] == clarification_receipt
    assert c.get(tr + "/calls") == first_calls
    answer_url = clarification["answer"]["url"]
    answer = {"command_id": uid(), "expected_schema_revision": clarification["expected_schema_revision"], "journey_consent_id": consent["id"], "choice": "bounding_box"}
    calls_before_answer = c.get(tr + "/calls")
    stale = c.request("POST", answer_url, {**answer, "command_id": uid(), "expected_schema_revision": "f" * 64}, expected=[409])
    assert stale["code"] == "schema_clarification_revision_conflict" and stale["admitted"] is False
    unsupported = c.request("POST", answer_url, {**answer, "command_id": uid(), "choice": "segmentation"}, expected=[409])
    assert unsupported["code"] == "segmentation_delivery_not_implemented" and unsupported["admitted"] is False
    assert c.get(tr + "/calls") == calls_before_answer
    assert c.get(tr + "/human-schema-drafts") == []
    answer_receipt = c.post(answer_url, answer)
    saved = answer_receipt["schema"]
    assert saved["task_id"] == task and answer_receipt["selected_choice"] == "bounding_box"
    replayed_answer = c.post(answer_url, answer)
    assert replayed_answer["schema"] == saved
    conflicting_answer = c.request("POST", answer_url, {**answer, "command_id": uid()}, expected=[409])
    assert conflicting_answer["code"] == "schema_clarification_answer_conflict" and conflicting_answer["admitted"] is False
    assert len(c.get(tr + "/human-schema-drafts")) == 1
    calls_after_answer_replay = c.get(tr + "/calls")
    assert sum(call["id"] == clarification["id"] for call in calls_after_answer_replay) == 1
    # This TEST provider has no artificial delay. Do not register an HTTP observer
    # while the child work completes: the server-owned worker must consume durable
    # Builder/Sample receipts even when completion wins that race.
    time.sleep(5)
    first_terminal_observation = c.get(execution)
    assert (first_terminal_observation.get("sample") or {}).get("assistance", {}).get("status") == "completed", first_terminal_observation
    completed = first_terminal_observation
    assert completed.get("sample") and completed["sample"]["id"] == consent["sample_operation_id"], completed
    sample = c.get(f"/api/workflow-drafts/{completed['sample']['draft_id']}/sample-test?test_id={consent['sample_operation_id']}")["sample_test"]
    assert [(image["image_id"], image["content_hash"]) for image in sample["inputs"]] == [(image["image_id"], image["sha256"]) for image in task_images]
    calls_before_replays = c.get(tr + "/calls")
    budget_before_replays = c.get(tr + "/budget")
    samples_before_replays = c.get(tr + "/sample-operations")
    workspace_before_replays = c.get(tr + "/workspace")
    builders_before_replays = workspace_before_replays["builder_operations"]["items"]
    assert [item["id"] for item in samples_before_replays["items"]] == [consent["sample_operation_id"]], samples_before_replays
    assert len(builders_before_replays) == 1 and builders_before_replays[0]["operation"]["id"] == consent["builder_operation_id"], builders_before_replays
    for _ in range(3):
        assert c.post(tr + "/journey-consents", consent)["consent"]["id"] == consent["id"]
        replay = c.post(execution)
        assert replay["sample"]["id"] == consent["sample_operation_id"], replay
    assert c.get(tr + "/calls") == calls_before_replays
    assert c.get(tr + "/budget") == budget_before_replays
    assert c.get(tr + "/sample-operations") == samples_before_replays
    builders_after_replays = c.get(tr + "/workspace")["builder_operations"]["items"]
    assert [(item["operation"]["id"], item["operation"]["status"]) for item in builders_after_replays] == [(item["operation"]["id"], item["operation"]["status"]) for item in builders_before_replays]
    assert c.get(f"/api/workflow-drafts/{completed['sample']['draft_id']}/sample-test?test_id={consent['sample_operation_id']}")["sample_test"] == sample
    delivery = c.get(tr + "/delivery-intent")
    assert delivery["missing_slots"] == []
    assert delivery["saved"]["intent"]["training_target"]["annotation_kind"] == "bounding_box"
    final_clarification = c.get(tr + "/calls/" + clarification["id"] + "/clarification")
    assert final_clarification["status"] == "applied" and final_clarification["schema_draft_id"] == saved["id"]
    return {"conversation_id": conversation, "task_id": task, "task_root": tr, "consent_id": consent["id"], "schema_call_id": clarification["id"], "question": clarification["question"], "question_count": 1, "calls_before_answer": len(first_calls), "sample_before_answer": False, "clarification_choices": clarification["choices"], "stale_choice_code": stale["code"], "unsupported_choice_code": unsupported["code"], "conflicting_answer_code": conflicting_answer["code"], "answer_url": answer_url, "answered_schema_id": saved["id"], "delivery_revision": delivery["saved"]["revision"], "sample_operation_id": completed["sample"]["id"], "sample_image_ids": [image["image_id"] for image in sample["inputs"]], "visual_model_id": visual_model["id"], "immediate_provider_delay_ms": 0, "observer_registered_after_terminal": True, "duplicate_completion_observations": 3, "sample_operation_count_after_replays": len(samples_before_replays["items"]), "builder_operation_count_after_replays": len(builders_after_replays), "budget_unchanged_after_replays": True, "formal_run_created": False}


def verify_stop(c, cr, schema_revision, provider, normal_model):
    delayed = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST delayed schema for Stop", "remote_model_id": "e2e-conversation-classification-schema-background", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": delayed["id"], "confirmed_billable": True})
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": delayed["id"]})
    receipt = c.post(cr + "/send", {"message": {"id": uid(), "text": "TEST delayed schema: classify indoor or outdoor", "image": None}, "schema_revision": schema_revision, "mode": "plan"})
    assert receipt["resolved_agent_model_id"] == delayed["id"]
    task = receipt["task_id"]
    tr = cr + "/tasks/" + task
    query = urllib.parse.urlencode({"consent_id": uid(), "schema_call_id": uid(), "builder_operation_id": uid(), "sample_operation_id": uid(), "planner_model_id": delayed["id"], "allowed_models": json.dumps(["model-profile:" + normal_model["id"]])})
    consent = c.get(tr + "/journey-preview?" + query)["consent"]
    consent["allow_unknown_cost"] = True
    consent["schema_proposal"]["allow_unknown_cost"] = True
    c.post(tr + "/journey-consents", consent)
    execution = tr + "/journey-consents/" + consent["id"] + "/execution"
    c.post(execution)
    c.poll(tr + "/calls", lambda calls: any(call["status"] == "reserved" for call in calls))
    message = {"id": uid(), "text": "停止", "image": None, "reference": {"scope": "stop_request", "task_id": task}}
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": normal_model["id"]})
    frozen = c.get(cr + "/send/" + receipt["message"]["input"]["id"])
    assert frozen["receipt"]["resolved_agent_model_id"] == delayed["id"]
    initial = c.post(cr + "/stop-requests", message)
    assert initial["status"] == "cancel_requested", initial
    stop_path = cr + "/stop-requests/" + message["id"]
    c.poll(execution, lambda value: value["dispatch"]["status"] == "settled")
    final = c.poll(stop_path, lambda value: value["normalized_state"] in ["outcome_unknown", "interrupted"])
    repeated = c.post(cr + "/stop-requests", message)
    assert repeated["selected_target"] == initial["selected_target"]
    c.get(tr + "/workspace")
    return {"task_id": task, "request_url": stop_path, "initial": initial, "final": final}


def verify_sse(c, run_id):
    history = c.get("/api/runs/" + run_id + "/events")["events"]
    # Persisted Run history endpoint returns the actual ordered event array.
    assert len(history) >= 2, history
    cursor = history[0]["event_id"]
    request = urllib.request.Request(c.base + "/api/events?run_id=" + run_id, headers={"Last-Event-ID": cursor})
    with c.http.open(request, timeout=5) as response:
        lines = []
        while True:
            line = response.readline().decode()
            if line in ["\n", "\r\n", ""] and lines:
                break
            lines.append(line)
        event = "".join(lines)
    assert "id: " + history[1]["event_id"] in event, event
    assert "id: " + cursor not in event
    c.trace.append({"method": "GET", "path": "/api/events?run_id=" + run_id, "last_event_id": cursor, "status": 200, "response": event})
    c.request("GET", "/api/events?run_id=" + run_id + "&last_event_id=" + uid(), expected=[409])


def restart_snapshot(c, manifest):
    tr = manifest["task_root"]
    snapshot = {suffix: c.get(tr + suffix) for suffix in ["/calls", "/budget", "/message-queue", "/human-requests", "/thread"]}
    snapshot["stop"] = c.get(manifest["stop"]["request_url"])
    snapshot["run_events"] = c.get("/api/runs/" + manifest["run_id"] + "/events")
    if manifest.get("p0_autonomy"):
        p0 = manifest["p0_autonomy"]
        snapshot["p0_autonomy"] = {
            "execution": c.get(manifest["execution_url"]),
            "sample": c.get(f"/api/workflow-drafts/{p0['draft_id']}/sample-test?test_id={p0['sample_operation_id']}"),
        }
        if p0.get("formal_delivery"):
            package_id = p0["formal_delivery"]["package_job_id"]
            package_path = tr + "/delivery-packages/" + package_id
            payload, transport = c.download(package_path + "/download")
            assert hashlib.sha256(payload).hexdigest() == p0["formal_delivery"]["package_sha256"]
            snapshot["p0_autonomy"]["package"] = c.get(package_path)
            snapshot["p0_autonomy"]["package_download"] = transport
    for kind, scene in manifest.get("controls", {}).items():
        snapshot[kind] = {"budget": c.get(scene["task_root"] + "/budget"), "batch": c.get(scene["batch_url"]), "actions": c.get(scene["task_root"] + "/workspace")["resume_actions"]}
    if manifest.get("bbox"):
        bbox = manifest["bbox"]
        snapshot["bbox"] = {"requests": c.get(bbox["task_root"] + "/human-requests"), "feedback": c.get(bbox["feedback_url"])}
    if manifest.get("diagnostic_scenes"):
        snapshot["diagnostic_scenes"] = verify_diagnostic_scenes(c, manifest)
    if manifest.get("demo_onboarding"):
        demo = manifest["demo_onboarding"]
        snapshot["demo_onboarding"] = {
            "preset_receipt": c.get("/api/demos/start/" + demo["preset"]["command_id"]),
            "live_receipt": c.get("/api/demos/start/" + demo["live"]["command_id"]),
            "preset_workspace": c.get(demo["preset_task_root"] + "/workspace"),
            "preset_review": c.get(demo["preset_task_root"] + "/delivery-review-items?limit=6"),
            "preset_package": c.get(demo["preset_task_root"] + "/delivery-packages/" + demo["preset_package_id"]),
            "live_workspace": c.get(demo["live_task_root"] + "/workspace"),
            "preset_usage": c.get(demo["preset_task_root"] + "/model-usage"),
            "live_usage": c.get(demo["live_task_root"] + "/model-usage"),
        }
    return snapshot
