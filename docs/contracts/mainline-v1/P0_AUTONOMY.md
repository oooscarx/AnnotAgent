# P0 bounded Journey continuation

This contract describes the existing Journey endpoint after the P0 continuation repair. It does not add another executor or grant. Schema, Builder and Sample remain the existing child services and keep their own receipts, limits, cancellation and geometry gates.

## Durable trigger map

| Edge | Trigger and durable input | Authorization | Stable child identity | Next wake / terminal rule |
|---|---|---|---|---|
| upload → Send scope | User `POST image-upload`, then `POST send` with exact returned IDs/hashes | none; metadata only | message ID and new Task ID | Send commits the partial DeliveryIntake; no model starts |
| Send → bounded approval | passive Task workspace and Journey preview | none; preview only | Task-derived consent, Schema call, Builder and Sample UUIDv5 IDs | user approves the one exact consent |
| consent → Schema | consent transaction saves the dispatch before returning | exact text-planning model, call count, expiry and goal scope | `schema_proposal.call_id` | worker rereads the call receipt until it commits |
| Schema draft → Builder | the same Journey worker saves the draft and resolves the consent | original Builder model/scope/call budget | `builder_operation_id` | `reserved/running` keeps the dispatch heartbeat alive |
| Builder → Sample | the same worker observes `draft_ready_for_human_review`, validates and seals the Draft | original image hashes, model binding digests, maximum Sample calls and expiry | original `sample_operation_id` | Sample write is idempotent; duplicate observations cannot create another ID |
| Sample → review | existing Sample runtime saves terminal reports/candidates | no annotation acceptance is inferred | Sample/image/candidate/source Artifact IDs | real candidate review or a typed blocker is the next human boundary |

Formal processing preview and confirmation both read the exact Sample-bound review
readiness from the Application. They return `409 sample_reviews_pending` while any
generated request is not yet applied, including deferred or merely saved-but-not-applied
answers. No processing receipt is reserved on this rejection. Once each answer is
persisted and its existing local feedback checkpoint is applied, the preview includes
the applied request IDs in its authorization fingerprint and may be explicitly approved.
Sample feedback remains Sandbox evidence and is never promoted to a formal annotation.

The worker is server-owned. Browser polling only reads these receipts. Synchronous child
completion is consumed in the current loop; asynchronous completion is observed from persisted
receipts on the next loop. The queue uses one claim/attempt lease, and restart returns routed
`running` work to `queued`. Revoked/expired scope, cancelled children and unknown remote outcomes
end the dispatch without a new call.

When the Schema proposal supplies delivery semantics that exactly complete the saved intake, the
same Journey also calls the existing deterministic `prepare_delivery_schema` Application service
with a consent-derived command ID. This creates one private, versioned delivery Schema and is safe
to replay. It does not mark the Schema human accepted, publish a Workflow, or add image permission.
The technical `prepare_delivery_schema` action therefore does not reappear after an automatically
prepared Sample.

If the proposal instead preserves complete labels but asks which annotation output to use,
the clarification GET exposes stable server choices. The compact answer command supplies only
the selected value, command ID, goal revision and original Journey consent. Bounding-box delivery
is currently supported. Segmentation and whole-image classification remain visible but disabled
with stable reasons because the bounded training-package path has no matching implemented preset.
The server rebuilds the complete Schema from saved semantics and never translates a contour
selection into a box. Exact replay restores the original Schema; unsupported/stale answers write nothing.

## One exact approval and passive reads

For a newly uploaded Task, `POST /api/projects/{project_id}/conversations/{conversation_id}/send`
accepts additive `task_images:[{image_id,sha256}]`. It is valid only while creating an ordinary
Task. The server verifies Project ownership and the current content hash, saves the Send receipt,
then creates revision 1 of the existing DeliveryIntake with the complete ordered image scope.
It does not infer labels, grant a model call, or start work. Repeating the same message ID and
payload restores the original Task/receipt; a changed image scope conflicts. The upload/import
report now returns `images[]` with the stable IDs and hashes for exactly the valid files in that
request, so clients never infer attachment identity from list order.

If the description creates the Task before its files arrive, the later explicit upload action
uses the existing `POST .../delivery-intent` CAS with `task_images:[{image_id,sha256}]` and that
same Task URL. The original message remains the goal. The exact command is restart-safe and a
changed hash, stale revision, duplicate identity or foreign image is rejected before storage.
This attachment-only revision still grants nothing and leads to the same combined preview.

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

There is no browser completion subscription to register. The worker observes child state from the
same SQLite receipts used by the GET projection. This covers both orderings: a slow child commits
after the worker first observes `reserved|running`, while an immediate child may commit before any
browser GET. Replaying the exact consent or execution command after terminal completion returns the
same Builder/Sample identities. It does not add a call receipt, increase
`planning_reserved_calls`, create another Sample operation, or replace the immutable
`WorkflowSampleTest`. Guided Sample is a sandbox result and does not create a formal production
Run; formal processing remains a separate approval boundary.

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

Once the Sample has terminal `needs_review` candidates, the Task projection makes that judgment the
single current action:

```json
{
  "id":"review_sample_results",
  "state":"available",
  "method":"GET",
  "url":"/api/projects/P/conversations/C/tasks/T/visual-selections",
  "requires_confirmation":false,
  "reason":"sample_candidates_require_human_judgment",
  "scope":{"sample_test_id":"S","pending_request_ids":["H1","H2","H3"],"pending_count":3}
}
```

`mainline.sample_review` repeats that bounded summary and `review_work_item_id` is the owned Task
ID. Each HumanRequest and visual selection still contains the full Sample/image/candidate/Artifact
lineage. Until those current Sample requests are resolved, the read model does not present
`prepare_delivery_schema` or full-dataset processing as its current action.

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
commercial model accuracy. Ambiguous label/output/export requests stop at one persisted
`SchemaClarification`, before Builder or image inference. A linked exact answer completes the
private delivery draft and resumes the same Journey; reads/replays cannot duplicate the question
or Schema call. A Schema that omits valid delivery semantics or a Provider result that is unknown
stops before image inference and is never automatically retried. Formal processing and package
admission retain their separate dataset-sized scopes.

Sample selection is identity based even though the legacy executor accepts array indices. The
Journey worker resolves its ordered consent `(image_id,content_hash)` pairs immediately before
admission and the running worker repeats that exact check. Tests add another uploaded image set
before the clarified Journey; the Sample still contains only its original three identities.

The opt-in HTTP smoke uploads six byte-distinct PNGs and sends the fixed Chinese acceptance
request from `07_ACCEPTANCE.md`. Its explicit TEST Provider delays the first Builder HTTP response
for four seconds. The consent POST must return in under three seconds with a queued/running
dispatch and no Sample; passive reads must later observe the original Sample ID as `passed` for
exactly three inputs. The manifest records decision/click counts, timings and POST counts. The
same smoke continues through formal processing and verifies that package export refuses while
whole-image reviews remain, so TEST candidates never become accepted annotations.
