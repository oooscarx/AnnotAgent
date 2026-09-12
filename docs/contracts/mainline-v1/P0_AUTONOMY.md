# P0 bounded Journey continuation

This contract describes the existing Journey endpoint after the P0 continuation repair. It does not add another executor or grant. Schema, Builder and Sample remain the existing child services and keep their own receipts, limits, cancellation and geometry gates.

## One exact approval and passive reads

For a newly uploaded Task, `POST /api/projects/{project_id}/conversations/{conversation_id}/send`
accepts additive `task_images:[{image_id,sha256}]`. It is valid only while creating an ordinary
Task. The server verifies Project ownership and the current content hash, saves the Send receipt,
then creates revision 1 of the existing DeliveryIntake with the complete ordered image scope.
It does not infer labels, grant a model call, or start work. Repeating the same message ID and
payload restores the original Task/receipt; a changed image scope conflicts. The upload/import
report now returns `images[]` with the stable IDs and hashes for exactly the valid files in that
request, so clients never infer attachment identity from list order.

When that intake has an exact image scope and is missing only label rules/training target,
`GET .../workspace` exposes one `build_and_test_pipeline` action. Its URL is parameterless
`GET .../journey-preview`. The preview derives stable consent/Schema-call/Builder/Sample IDs,
keeps all Task images in the intake, and freezes no more than three ordered images for Sample.
It selects only the one Ready Project-bound `primary_inference` model (or the sole Ready visual
Project binding). It never expands to all Registry models. No binding or an ambiguous binding
returns `400 capability_setup_required` with the server-owned `visual_inference` setup request.
The request uses one `required_capabilities` entry (detection/grounding when available, otherwise
VLM fallback); it does not require segmentation.

`POST .../journey-consents` is the single user approval for the exact Schema + Builder + Sample
scope. A newly saved initial consent immediately persists its durable execution intent and starts
the existing worker when capacity is available. Repeating the POST returns the same consent and
dispatch. Older clients may still call the execution POST below; it is idempotent and cannot add
a second worker.

`POST /api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents/{consent_id}/execution` accepts `{}`. For an older saved consent without a dispatch, the first accepted request persists a server execution intent before waiting for worker capacity. A successful response may therefore contain either:

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

This increment closes upload identity → partial intake → one bounded approval → durable Schema,
Builder and Sample continuation. A deterministic external TEST Provider smoke reaches a real
saved Sample through the normal HTTP/SQLite/provider path; it is test evidence, not a claim about
commercial model accuracy. Ambiguous label/output/export requests still stop at the existing
HumanRequest. A Schema that omits valid delivery semantics or a Provider result that is unknown
stops before image inference and is never automatically retried. Formal processing and package
admission retain their separate dataset-sized scopes.

Fixed-SHA acceptance at `d48ab89bf0d26d83b96b6a0fa92a1af8262c93da` used the
external TEST HTTP Provider with the same multi-label bbox identity used by the packaged UI.
It uploaded four distinct images, sent their exact IDs/hashes, read the parameterless preview,
POSTed the consent once, and only polled passive GETs until the three-image Sample was `passed`.
The retained manifest and request trace are under
`/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-tqi6wd3n`.
Starting the service again against that same marked workspace produced
`seed_snapshot_unchanged:true` and `restart_verified:true`; the consent, call, Builder, Draft,
Sample, Run and review identities did not change. Formal processing covered all four delivery
images under its own approval. Package export then correctly stopped on outstanding whole-image
review instead of treating the TEST model candidates as accepted annotations.
