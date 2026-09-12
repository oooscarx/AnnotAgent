"""Real HTTP seed/checks for http_fixture.py. External inference only is synthetic."""
import datetime
import http.cookiejar
import json
from pathlib import Path
import struct
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zlib


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

    def poll(self, path, predicate):
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            result = self.get(path)
            if predicate(result):
                return result
            time.sleep(0.3)
        raise AssertionError(("poll timeout", path, result))


def seed_and_verify(manifest, root):
    c = Client(manifest["base_url"])
    evidence = Path(manifest["workspace"]) / "HTTP_TRACE.json"
    try:
        return verify(c, manifest, root)
    finally:
        evidence.write_text(json.dumps({"fixture": "TEST external-model-only; real application HTTP and database", "requests": c.trace}, indent=2, ensure_ascii=False) + "\n")


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
    approval = c.get(p + "/processing-preview?" + urllib.parse.urlencode(selection))
    processing = c.post(p + "/processing-operations", {"request_id": uid(), "selection": selection, "expected_revision": approval["revision"], "authorization_fingerprint": approval["authorization_fingerprint"]})
    batch = c.poll("/api/batches/" + processing["batch_id"], lambda value: value["batch"]["status"] not in ["pending", "running", "pausing"])
    assert batch["batch"]["status"] in ["completed", "awaiting_review"], batch
    run_id = batch["batch"]["child_run_ids"][0]
    verify_sse(c, run_id)
    c.get("/api/runs/" + run_id + "/result-summary")
    c.get("/api/runs/" + run_id + "/debug-summary")
    projection = record["report"]["samples"][0]["projection"]
    candidate = (projection["final_candidates"] or [entry["candidate"] for entry in projection["review_candidates"]])[0]
    image = record["inputs"][0]
    human = {"id": uid(), "task_id": task, "conversation_id": conversation, "sample_test_id": record["id"], "image_id": image["image_id"], "content_hash": image["content_hash"], "outcome_id": candidate["outcome"]["id"], "expected_feedback_sequence": 0, "reason_code": "poor_boundary", "question": "TEST confirm saved sample box", "resume_checkpoint_ref": record["draft_id"]}
    c.post(tr + "/human-requests", human)
    answer = {"revision_id": uid(), "sample_test_id": record["id"], "image_id": image["image_id"], "sequence": 1, "reason": "poor_boundary", "outcome_id": candidate["outcome"]["id"], "corrected_value": candidate["outcome"]["value"], "corrected_label": candidate["outcome"]["label"], "note": "TEST saved answer", "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    answer_path = tr + "/human-requests/" + human["id"] + "/answer"
    saved = c.post(answer_path, {"answer": answer})
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
    stop = verify_stop(c, cr, schema_revision, provider, model)
    pending = {**human, "id": uid(), "expected_feedback_sequence": 1, "question": "TEST pending answer for frontend adapter"}
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
    return {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "model_profile_id": model["id"], "provider_id": provider["id"], "execution_url": execution, "p0_autonomy": {"task_images": task_images, "task_image_count": len(task_images), "sample_image_count": len(record["inputs"]), "unavoidable_user_decisions": 1, "technical_relay_clicks": 0, "consent_post_count": len(consent_posts), "execution_post_count": len(execution_posts), "consent_response_ms": consent_response_ms, "journey_duration_ms": journey_duration_ms, "first_observation": first_observation, "consent_id": consent["id"], "schema_call_id": consent["schema_proposal"]["call_id"], "builder_operation_id": consent["builder_operation_id"], "sample_operation_id": consent["sample_operation_id"], "draft_id": record["draft_id"], "sample_status": record["status"], "delivery_schema_id": delivery_schema["schema"]["id"], "review_work_item_id": review_workspace["review_work_item_id"], "review_action": review_workspace["available_actions"][0], "automatic_review_request_ids": [item["input"]["id"] for item in automatic_reviews], "execution_dispatch": finished["dispatch"]}, "describe_before_upload": describe_before_upload, "ambiguous_goal": ambiguous_goal, "export": export, "run_id": run_id, "stop": stop, "answered_request_id": human["id"], "pending_request_id": pending["id"], "plan_task_id": plan["task_id"], "controls": controls, "saved_plan": saved_plan, "bbox": bbox, "manual_stop": manual_stop, "trace": str(Path(manifest["workspace"]) / "HTTP_TRACE.json")}


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
    clarification = waiting["clarification"]
    assert clarification["id"] == consent["schema_proposal"]["call_id"]
    assert all(word in clarification["question"] for word in ["框", "轮廓", "分类"])
    assert waiting["builder"] is None and waiting["sample"] is None
    first_calls = c.get(tr + "/calls")
    assert len(first_calls) == 1 and first_calls[0]["id"] == clarification["id"], first_calls
    for _ in range(3):
        observed = c.get(execution)
        assert observed["clarification"] == clarification
    assert c.post(tr + "/journey-consents", consent)["consent"]["id"] == consent["id"]
    c.post(execution)
    assert c.get(execution)["clarification"] == clarification
    assert c.get(tr + "/calls") == first_calls
    answer = {
        "request_id": uid(),
        "decision": {
            "decision": "draft", "kind": "bounding_box", "labels": ["cup"],
            "multi_label": False, "attributes": {},
            "boundary_rules": ["框住完整可见杯子；排除杯子图案"],
            "rationale": "用户明确选择目标框；不把轮廓或分类转换为框。",
            "delivery": {
                "labels": [{"existing_id": None, "display_name": "杯子", "aliases": ["cup"], "include": "真实杯子", "exclude": "杯子图案"}],
                "training_target": {"annotation_kind": "bounding_box", "framework": "ultralytics", "export_profile": "ultralytics_yolo_detection", "profile_revision": 1}
            }
        },
        "clarification": {"call_id": clarification["id"], "expected_schema_revision": clarification["expected_schema_revision"]},
        "journey_consent_id": consent["id"]
    }
    saved = c.post(tr + "/human-schema-drafts", answer)
    assert saved["task_id"] == task
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
    return {"conversation_id": conversation, "task_id": task, "task_root": tr, "consent_id": consent["id"], "schema_call_id": clarification["id"], "question": clarification["question"], "question_count": 1, "calls_before_answer": len(first_calls), "sample_before_answer": False, "answered_schema_id": saved["id"], "delivery_revision": delivery["saved"]["revision"], "sample_operation_id": completed["sample"]["id"], "sample_image_ids": [image["image_id"] for image in sample["inputs"]], "visual_model_id": visual_model["id"], "immediate_provider_delay_ms": 0, "observer_registered_after_terminal": True, "duplicate_completion_observations": 3, "sample_operation_count_after_replays": len(samples_before_replays["items"]), "builder_operation_count_after_replays": len(builders_after_replays), "budget_unchanged_after_replays": True, "formal_run_created": False}


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
    for kind, scene in manifest.get("controls", {}).items():
        snapshot[kind] = {"budget": c.get(scene["task_root"] + "/budget"), "batch": c.get(scene["batch_url"]), "actions": c.get(scene["task_root"] + "/workspace")["resume_actions"]}
    if manifest.get("bbox"):
        bbox = manifest["bbox"]
        snapshot["bbox"] = {"requests": c.get(bbox["task_root"] + "/human-requests"), "feedback": c.get(bbox["feedback_url"])}
    return snapshot
