"""Real HTTP seed/checks for http_fixture.py. External inference only is synthetic."""
import datetime
import http.cookiejar
import json
from pathlib import Path
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


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
    model = c.post("/api/model-profiles", {"provider_id": provider["id"], "display_name": "TEST deterministic model", "remote_model_id": "e2e-conversation-classification-feedback-clarify", "input_modalities": ["text", "image"], "task_capabilities": ["text_generation", "vision_language", "image_classification"], "protocol_features": {"tool_calls": True, "structured_output": True}})
    c.post(f"/api/providers/{provider['id']}/active-probe", {"model_profile_id": model["id"], "confirmed_billable": True})
    defaults = c.get("/api/agent-model-bindings")
    c.request("PUT", "/api/agent-model-bindings", {**defaults, "pipeline_builder": model["id"]})
    project = "TEST-agent-ui-" + uid()
    c.post("/api/projects", {"id": project, "yaml": "version: 1\nproject:\n  name: TEST Agent UI HTTP fixture\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"})
    p = f"/api/projects/{project}"
    c.request("PUT", p + "/model-bindings", {"bindings": [{"capability": "image_classification", "role": "classification", "match_kind": "capability", "model_profile_id": model["id"], "locked": False}]})
    c.request("POST", p + "/image-upload?name=TEST-synthetic.png", (root / "examples/robocup/images/synthetic-robocup.png").read_bytes())
    conversation = c.post(p + "/conversations")["conversation_id"]
    cr = p + "/conversations/" + conversation
    preference = c.get(cr + "/agent-model")
    c.post(cr + "/agent-model", {"request_id": uid(), "expected_revision": preference["revision"], "model_profile_id": model["id"]})
    schema_revision = c.get(p + "/goal")["revision"]
    command = {"message": {"id": uid(), "text": "按室内和室外给图片分类", "image": None}, "schema_revision": schema_revision, "mode": "plan"}
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
    assert workspace["actions"]["approve"]["available"] is False
    c.request("GET", cr + "/tasks/" + uid() + "/workspace", expected=[400, 404])
    c.get(tr + "/message-queue")
    settings = c.get("/api/settings?view=agent-ui")
    budget = {**settings["sections"]["usage_budget"]["future_run_budget"], "max_requests": 11}
    c.request("PATCH", "/api/settings", {"expected_revision": settings["revision"], "budget": budget})
    c.request("PATCH", "/api/settings", {"expected_revision": "TEST-stale", "budget": budget}, expected=[409])
    # Journey is explicit independent approval, not an authority granted by Send.
    query = urllib.parse.urlencode({"consent_id": uid(), "schema_call_id": uid(), "builder_operation_id": uid(), "sample_operation_id": uid(), "planner_model_id": model["id"], "allowed_models": json.dumps(["model-profile:" + model["id"]])})
    consent = c.get(tr + "/journey-preview?" + query)["consent"]
    consent["allow_unknown_cost"] = True
    consent["schema_proposal"]["allow_unknown_cost"] = True
    c.post(tr + "/journey-consents", consent)
    execution = tr + "/journey-consents/" + consent["id"] + "/execution"
    c.post(execution)
    finished = c.poll(execution, lambda v: (v.get("sample") or {}).get("assistance", {}).get("status") == "completed")
    assert finished["sample"]["status"] == "succeeded", finished
    record = c.get(f"/api/workflow-drafts/{finished['sample']['draft_id']}/sample-test?test_id={consent['sample_operation_id']}")["sample_test"]
    selection = {"draft_id": record["draft_id"], "sample_test_id": record["id"], "limit": 1}
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
    human = {"id": uid(), "task_id": task, "conversation_id": conversation, "sample_test_id": record["id"], "image_id": image["image_id"], "content_hash": image["content_hash"], "outcome_id": candidate["outcome"]["id"], "expected_feedback_sequence": 0, "reason_code": "wrong_target", "question": "TEST confirm saved sample classification", "resume_checkpoint_ref": record["draft_id"]}
    c.post(tr + "/human-requests", human)
    answer = {"revision_id": uid(), "sample_test_id": record["id"], "image_id": image["image_id"], "sequence": 1, "reason": "wrong_target", "outcome_id": candidate["outcome"]["id"], "corrected_value": {"kind": "classification", "labels": ["室内"]}, "corrected_label": "室内", "note": "TEST saved answer", "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
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
    preview = c.get(queue_root + supplements[0] + "/schema-preview")
    queue_consent = {key: preview[key] for key in ["model_id", "scope_hash", "request_hash", "previous_grant_id", "maximum_calls", "expires_at"]}
    queue_consent.update(call_id=uid(), allow_unknown_cost=True)
    queued_receipt = c.post(queue_root + supplements[0] + "/schema-proposals", queue_consent)
    assert c.post(queue_root + supplements[0] + "/schema-proposals", queue_consent) == queued_receipt
    c.get(queue_root + supplements[0] + "/schema-authorization")
    c.get(tr + "/message-queue")
    c.get(tr + "/calls")
    c.get(tr + "/workspace")
    c.get(p + "/export-readiness")
    # Export the actual accepted annotation produced by Processing.
    export = c.post(p + "/export", {"format": "native", "conversation": {"id": uid(), "conversation_id": conversation, "task_id": task}, "background": True})
    export = c.poll(tr + "/exports/" + export["job"]["id"], lambda value: not value["active"])
    assert export["job"]["error"] is None, export
    delivery = export["job"]["result"]["delivery"]
    download = c.request("GET", p + "/exports/" + delivery["id"] + "/download", raw=True)
    assert download["bytes"] == delivery["bytes"]
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
    first_page = c.get(cr + "/task-navigation?limit=1")
    assert first_page["next_cursor"] is not None
    second_page = c.get(cr + "/task-navigation?limit=1&cursor=" + str(first_page["next_cursor"]))
    assert first_page["items"][0]["task_id"] != second_page["items"][0]["task_id"]
    return {"project": project, "conversation_id": conversation, "task_id": task, "task_root": tr, "model_profile_id": model["id"], "provider_id": provider["id"], "execution_url": execution, "export": export, "run_id": run_id, "stop": stop, "answered_request_id": human["id"], "pending_request_id": pending["id"], "plan_task_id": plan["task_id"], "controls": controls, "saved_plan": saved_plan, "bbox": bbox, "manual_stop": manual_stop, "trace": str(Path(manifest["workspace"]) / "HTTP_TRACE.json")}


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
    for kind, scene in manifest.get("controls", {}).items():
        snapshot[kind] = {"budget": c.get(scene["task_root"] + "/budget"), "batch": c.get(scene["batch_url"]), "actions": c.get(scene["task_root"] + "/workspace")["resume_actions"]}
    if manifest.get("bbox"):
        bbox = manifest["bbox"]
        snapshot["bbox"] = {"requests": c.get(bbox["task_root"] + "/human-requests"), "feedback": c.get(bbox["feedback_url"])}
    return snapshot
