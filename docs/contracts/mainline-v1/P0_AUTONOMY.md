# P0 bounded Journey continuation

This contract describes the existing Journey endpoint after the P0 continuation repair. It does not add another executor or grant. Schema, Builder and Sample remain the existing child services and keep their own receipts, limits, cancellation and geometry gates.

## Explicit execution and passive reads

`POST /api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents/{consent_id}/execution` accepts `{}`. The first accepted request persists a server execution intent before waiting for worker capacity. A successful response may therefore contain either:

```json
{
  "record": {"consent": {"id": "...", "builder_operation_id": "...", "sample_operation_id": "..."}},
  "schema": null,
  "builder": null,
  "sample": null,
  "dispatch": {
    "attempt_id": "queue-or-worker-uuid",
    "project_route_id": "project-route-id",
    "status": "queued",
    "error": null,
    "updated_at": "2026-09-12T10:00:00Z"
  }
}
```

or `dispatch.status:"running"` if a worker claimed it before the response was composed. `queued` and `running` are both active. Repeating the POST while either is active returns the same saved child identities and never admits a second worker. Capacity exhaustion leaves the intent `queued`; it is not a 429 that requires a human relay.

All GET routes remain passive. In particular, GET execution/status, Task workspace and capability readiness never create a dispatch, call a Provider, save a Draft or start a Sample.

## Child completion and restart

When this exact Journey finds its already saved Builder in `reserved`/`running`, the worker stays attached and rereads durable receipts. A completed Builder with `evidence.outcome:"draft_ready_for_human_review"` is validated, sealed to the original consent and starts only the consent's original `sample_operation_id`. A duplicate completion observation is harmless because the Sample operation ID and sealed scope are immutable.

On restart, a new P0 dispatch that was `running` and contains its server-saved `project_route_id` returns to `queued`. Startup schedules it without a browser. Child recovery runs first: a text call in doubt, interrupted Builder, or interrupted Sample is read as terminal and is not resent. Legacy dispatch rows have no route identity and retain the old `interrupted` behavior.

Terminal child states produce a safe dispatch error such as:

```json
{
  "status": "settled",
  "error": "Builder ended as interrupted; no Sample was started. Inspect the saved Builder receipt."
}
```

No raw Provider response or credential is copied into this error. Repeating an execution POST after a settled error returns the saved failure and does not retry it automatically.

`record.resolved_consent:null` is expected when the Journey began with an already saved Schema. `record.consent` is then the effective immutable authorization. `resolved_consent` is populated only when an initial Schema/clarification or acknowledged feedback repair had to bind a later exact server result.

## Mainline projection

While the dispatch is queued or running and no Sample record exists, `GET .../workspace` exposes one passive action:

```json
{
  "id": "inspect_automatic_sample_progress",
  "state": "available",
  "method": "GET",
  "url": "/api/projects/P/conversations/C/tasks/T/journey-consents/J/execution",
  "requires_confirmation": false,
  "reason": "authorized_journey_is_automatically_continuing",
  "scope": {
    "journey_consent_id": "J",
    "builder_operation_id": "B",
    "sample_operation_id": "S",
    "dispatch_status": "queued"
  }
}
```

The same Journey is not projected as `test_pipeline_samples/requires_confirmation` during automatic continuation.

`GET .../capability-readiness` exposes:

```json
{
  "authorization": {
    "source": "journey_consent",
    "consent_id": "J",
    "active": true,
    "can_resume_without_authorization": true,
    "continuation_state": "queued",
    "continuation_reason": "saved_execution_intent_is_active"
  }
}
```

The boolean is true only for `queued`/`running`, unexpired, unrevoked consent whose image hashes, Schema revision and model binding digests still validate. A changed scope becomes `continuation_state:"approval_required"`, `continuation_reason:"authorized_scope_changed"`, and the boolean is false.

## Current boundary

This increment closes the Builder-to-Sample relay and durable capacity queue. It does not yet prove that free-form upload text can deterministically establish every missing DeliveryIntake slot without a question. Ambiguous label/output/export requests must still stop at one explicit clarification. Formal processing and package admission retain their separate dataset-sized scopes until their P0 increments are delivered.
