# AnnotAgent Mainline v1 — HTTP bindings

Audit baseline: `d3220cb54bd50589efed86b6c0753bf4ea8db0db`.
This file separates endpoints present at that commit from the minimal additions
planned for B1–B4. `GET` routes below are passive: none plans, installs, retries,
starts inference, publishes, admits a package, or changes review state.

## Stable ownership and common rules

- URL `project_id` is the project route ID. Application resolves it to the stable
  `project_owner_id`; clients never supply or infer that owner UUID.
- Conversation, Task, message, operation, Run, image, annotation, Draft and package
  IDs are stable opaque IDs. A display name, row number, latest Run, or current UI
  selection is never an ownership key.
- Mutations use the existing local session + CSRF boundary. Unknown JSON fields are
  rejected. Exact command replay returns the saved result; reusing a command ID with
  changed content/scope is rejected.
- Revision/hash fields are CAS inputs. A preview, saved intent, Plan, or execution
  mode does not grant image/model/package authority.
- Model/provider calls with unknown remote outcome are not automatically retried.

## Current Task and journal APIs (implemented)

| Method and URL | Request / response | Boundary |
|---|---|---|
| `GET /api/navigation?cursor=&limit=50` | `{workspace_id,items:[{project_id,project_owner_id,title,conversation_id}],next_cursor}` | Owner-keyset page, limit 1..100. |
| `GET /api/projects/P/conversations/C/task-navigation?cursor=0&limit=50` | `{items:[{task_id,source_message_id,schema_revision,sequence,state}],next_cursor}` | Message-sequence keyset; state is activity only, not dataset completion. |
| `GET /api/projects/P/conversations/C/tasks/T/thread?cursor=0&limit=50` | `{items:[ResultMessageProjection],next_cursor}` | Currently only persisted user messages; no fabricated assistant reply. |
| `GET /api/projects/P/conversations/C/tasks/T/visual-selections?cursor=0&limit=10` | Canonical paged Sample selection envelopes with Task Schema, Draft/Sample, image hash/result revision and per-candidate Artifact lineage | Limit 1..20. Whole-image/null results are not candidate references; a nil source Artifact is returned with `feedback_available:false`. |
| `GET /api/projects/P/conversations/C/tasks/T/workspace` | existing calls, queue, HumanRequests, Builder/Journey/Sample/processing and B1 `read_model_revision`, `delivery`, `mainline` | Individually committed reconciliation snapshot. B1 reconciles delivery, matching Schema, current review counts and package receipts. Exact formal source and capability readiness remain B3/B4. |
| `POST /api/projects/P/conversations/C/send` | `ConversationSendInput` → frozen `ConversationSendReceipt` | New task or follow-up. Ordinary follow-up may queue; SampleCandidate reference takes feedback disposition. Send itself grants nothing. |
| `GET /api/projects/P/conversations/C/send/M` | saved receipt | Lost-response recovery, no dispatch. |

`ConversationSelectionRef` supports `stop_request`, exact `sample_candidate`, and
the B2 `formal_annotation` reference. Sample references include Task/Schema, Draft
revision, Sample Test, candidate and source Artifact plus an image ID/hash.
Application validates every link against saved Sandbox evidence before persisting
the message. A bare bbox, candidate ID or annotation ID is rejected.

B2 adds the passive canonical `visual-selections` read model so clients do not join
those identities from independent responses. Each terminal candidate has its own
`source_artifact_id`; an Artifact is never inherited from another candidate or the
image. The existing SampleCandidate send DTO and validator remain unchanged.

`delivery-review-items[].annotations[].conversation_reference` is the canonical
formal annotation reference. The enclosing `ConversationMessageInput.image` carries
the image ID/hash; the reference carries Task/Schema, delivery intent revision/hash,
processing operation, Batch, child Run, annotation revision and expected formal
snapshot hash. Application validation and the storage transaction both recheck the
full lineage, latest revision and current snapshot. An ordinary send with this
reference is saved with disposition `formal_feedback` and is never admitted to the
generic message queue. `GET .../feedback-preview?message_id=M` and
`GET .../feedback?message_id=M` return the exact saved subject plus the existing
formal object edit URL. `model_call_supported:false` is explicit: v1 does not infer
or guess geometry from prose.

## Delivery intent, Schema and formal review (implemented)

Base URL `D=/api/projects/P/conversations/C/tasks/T`.

| Method and URL | Exact meaning |
|---|---|
| `GET D/delivery-intent` | `TaskDeliveryView {saved,missing_slots,blockers,maximum_sample_images:3,execution_authorized:false,proposals}`. |
| `POST D/delivery-intent` | Saves `SaveTaskDeliveryIntent`; owner, image hashes and CAS checked. New upload flows use `task_images:[{image_id,sha256}]`; legacy `image_ids` remains accepted, and the two fields are mutually exclusive. It never calls a model. |
| `GET D/delivery-schema` | Current human Schema matching exact delivery revision/hash, or null. |
| `POST D/delivery-schema` | `{command_id,expected_revision,expected_sha256}`; deterministically creates the existing human Schema Draft. No LLM, publication or execution. |
| `GET D/formal-result` | Current exact delivery revision's processing operation, Batch, Published Workflow version and ordered image→child Run/status/error projection; null before an exact processing operation exists. |
| `GET D/delivery-review-items?cursor=0&limit=50` | Frozen intent revision/hash, bounded counts and ordered image items with processing/batch/child Run, review revision/decision, snapshot and error. Limit 1..100; cursor is an index into the frozen delivery order. |
| `GET D/delivery-images/I?source_run_id=R` | `TaskDeliveryImageView`; source choices join this Task's exact delivery processing operation→Batch→child Run. Project-global terminal Runs are excluded. |
| `POST D/delivery-images/I` | Whole-image receipt with intent/snapshot/review CAS. Empty detection is not negative; `confirmed:true` is required. |
| `POST D/delivery-images/I/objects` | Revises one formal object with Run, annotation, image snapshot and intent lineage. |
| `POST D/delivery-images/I/missing-objects` | Creates one human formal object with Run/image snapshot and intent lineage. |

Only `ultralytics_yolo_detection` profile revision 1 + bounding boxes is a complete
training-package target in v1. Classification/segmentation intent is preserved and
reported blocked; it is not silently converted.

## Schema, Builder, Journey, Sample, processing and control (implemented)

- `GET D/schema-preview`, `POST D/schema-proposals`, call receipts/cancellation and
  human Schema routes retain exact grant, model revision, budget, stop and unknown
  outcome behavior.
- `GET D/builder-preview`, `POST D/builder-operations` and Builder history reuse the
  existing Registry/model selection and call ledger.
- `GET D/journey-preview`, `POST/GET D/journey-consents`, explicit execution and
  revoke compose Builder + Sample with a frozen image/model scope.
- `GET D/sample-preview`, `GET D/sample-operations`; project Sample POST remains the
  existing bounded Sandbox path. A Plan grant is not vision permission.
- `GET /api/projects/P/processing-preview?draft_id=...&sample_test_id=...&limit=N`
  keeps the legacy first-N behavior for non-delivery work. For a delivery Task it
  freezes the saved image IDs/hashes in saved order and rejects any `limit` query.
  A conversation-owned Sample is eligible only after its local assistance pass has
  settled and every HumanRequest bound to that exact `sample_test_id` is `applied`.
  `pending`, `answered`, deferred, cancelled or stale requests return HTTP 409 before
  an authorization fingerprint is issued. The same check runs again in the confirm
  command before any processing receipt, publication, Batch or Provider call is made.
  `POST
  /api/projects/P/processing-operations` requires the returned revision and
  authorization fingerprint. Its saved authorization includes the delivery intent
  revision/hash used later to identify formal child Runs, plus the passive
  `sample_review` readiness snapshot.

The review gate error is stable and safe to render:

```json
{
  "status":409,
  "code":"sample_reviews_pending",
  "error":"Resolve every exact Sample review before authorizing formal processing.",
  "admitted":false,
  "suggested_action":"review_sample_results",
  "sample_review":{
    "sample_test_id":"SAMPLE_ID",
    "assistance_status":"completed",
    "applied_request_ids":[],
    "unresolved":[{"request_id":"REQUEST_ID","image_id":"IMAGE_ID","reason_code":"terminal_result_requires_review","status":"pending","deferred":false}],
    "ready":false,
    "reason_code":"sample_reviews_pending"
  }
}
```

Assistance still running returns `sample_review_preparation_incomplete`; a failed
assistance projection returns `sample_review_preparation_failed`. After all exact
answers have been saved and locally applied, preview returns
`sample_review:{ready:true,reason_code:null,unresolved:[],applied_request_ids:[...]}`.
These Sandbox answers remain feedback evidence; they do not become formal annotations.
- After a Journey has saved a `passed|human_approved` Sample and before formal
  processing exists, `mainline.available_actions` returns
  `start_delivery_processing` as `requires_confirmation`. Its stable GET URL selects
  that exact Draft and Sample. Its scope repeats the current delivery revision/hash,
  ordered image IDs/hashes and Draft/Sample identity. Calling the preview freezes and
  validates current model bindings, destination, allowance and known/unknown cost;
  the action itself grants and dispatches nothing. If no eligible Sample exists the
  read model continues to return `build_and_test_pipeline`.
- Existing stop, queue, HumanRequest, resume and event routes keep their current
  command IDs, budgets and terminal semantics. Paused checkpoints can resume;
  in-doubt Provider calls cannot.

## Training package and one-shot consent (implemented)

Implemented HTTP:

| Method and URL | Contract |
|---|---|
| `POST D/delivery-packages` | Exact `DeliveryPackageInput`; synchronously admits a frozen job, then runs the existing local Rust packager. Exact command replay does not redispatch. |
| `GET D/delivery-packages/J` | `{job,active,interrupted}`; phases `preparing/exporting/validating/ready/failed/cancelled`. |
| `POST D/delivery-packages/J/cancel` | `{confirmed:true}`. |
| `GET D/delivery-packages/J/download` | Owned ZIP, only after Ready; no work starts on GET. |

Added B3 routes backed by the existing Application/Storage consent methods:

- `GET D/delivery-package-consents` → `{items,next_cursor:null}` (latest 20)
- `POST D/delivery-package-consents` with
  `{id,intent_revision,intent_sha256,confirmed:true}`
- `GET D/delivery-package-consents/K`
- `POST D/delivery-package-consents/K/cancel` with `{confirmed:true}`

`GET/list` projects saved `state`, effective `armed/blocked/consumed/cancelled/stale`, readiness counts,
reasons, frozen delivery revision/hash and linked job. Authorizing uses
`DeliveryPackageConsentInput {id,intent_revision,intent_sha256,confirmed:true}`.
The final qualifying review, or authorization after all reviews are current, tries
the existing durable admission. Storage atomically transitions one armed consent to
one package job and exact retries never redispatch. The admitted job is persisted as
`preparing` before a local worker waits for the existing bounded export semaphore;
capacity exhaustion therefore needs no browser retry. Startup resumes admitted
`preparing|exporting|validating` local jobs from their frozen snapshot. It also
rechecks still-armed consents against the current intent and every current whole-image
receipt, covering a crash after the final review commit but before its event hook.
No consent, stale/cancelled consent or incomplete review means no admission. GET never
admits or resumes work.

## Model readiness, CAS and history cutoff (implemented facts)

- `GET /api/models`, `GET /api/model-profiles`, compatible profile and project model
  binding routes expose roles, declared capabilities, availability group/status,
  model/profile revision and safe endpoint summary. They do not perform a paid probe.
- `PATCH /api/model-profiles/ID` accepts optional `expected_revision`. Storage uses
  `BEGIN IMMEDIATE`, verifies the latest persisted revision and appends exactly one
  revision. Stale writer returns HTTP 409
  `model_profile_revision_conflict {expected_revision,current_revision}`. Published
  snapshots are immutable. Legacy PATCH without CAS remains accepted and must not be
  presented by the new UI as atomic concurrent-edit safety.
- `GET /api/history-scope` is passive. `POST /api/history-scope/preview` is read-only;
  explicit `POST /api/history-scope` establishes one durable scope using command ID
  and snapshot hash. Scoped Run/Batch/Pipeline/trash queries filter membership in SQL
  before pagination; direct old references remain readable and scoped management
  refuses excluded objects.

B4 adds `GET D/capability-readiness` and embeds the same object at
`workspace.mainline.capability_readiness`. The server-owned response contains
Project owner/Conversation/Task identity and Schema revision; optional current Draft
ID/revision/content hash; a `snapshot_sha256` Registry revision; compatible Agent and
visual Model Profile, Plugin model and Model Instance candidates; current exact
Journey permission digest/`allowed_models`; existing Task call allowance; and a
task-cost projection. The Conversation Agent preference and each Profile's
`selected_for_next_agent_request` flag apply only to a future Send. Candidate
readiness is exactly
`ready|unknown|unavailable|disabled`; `production_eligible`, TEST-fixture status,
quality contracts, setup API and blocker are explicit. A configured or unknown
profile is never reported Ready from a read. Unknown priced usage remains unknown;
zero receipts report a known zero without choosing a currency.

The endpoint reads no credential bytes and performs no Provider/Plugin call, health
probe, install, resume or grant write. Model setup completion changes the next
Registry digest and causes a recheck only. It never expands an old `allowed_models`
grant; each candidate says whether its current exact binding digest is in that grant.

## Mainline Task read model and local advance (B1 implemented; B2–B4 additive)

The existing `GET D/workspace` is extended rather than creating a parallel Task API.
Implemented additive fields are `read_model_revision`, `delivery`, and `mainline`.
The `mainline` object currently contains:

- `read_model_revision`: server digest/revision for command CAS.
- `delivery`: three-slot intake revision/hash and the matching human Schema Draft;
- bounded current whole-image review counts and the latest 20 package consent/job
  receipts;
- `blockers[]`, `available_actions[]` using exact action IDs,
  methods/URLs, scope/revision/hash and `requires_confirmation`.
- `package`: consent and job reconciliation. Task completion is true only when a package is
  Ready; a completed model call, Schema, Draft, Sample or Batch is not task completion.

When delivery intake is absent, `save_delivery_intake` remains available. A new Send
may instead carry `task_images:[{image_id,sha256}]`; the server creates a partial
intake with that complete ordered scope and returns one `build_and_test_pipeline`
approval action when only labels/target remain. The action points to the existing
Journey preview; the single consent freezes one text Schema proposal, one Builder and
at most three Sample images. It grants no formal processing, publication, annotation
acceptance or package write.

Step/result-message projection remains bounded by existing real thread and operation
receipts. Formal source, capability readiness, paged review items and package
admission are implemented.

After the automatic Journey saves real Sample candidates requiring review,
`workspace.mainline.available_actions` contains the one passive
`review_sample_results` GET action to `D/visual-selections`.
`mainline.sample_review` contains `sample_test_id`, `pending_count`, exact
`pending_request_ids` and that URL; `review_work_item_id` is the Task ID. The same
Journey has already used the existing deterministic delivery-Schema service with a
consent-derived idempotency ID, so the UI is not asked to prepare another Schema.
Pending Sample review also prevents `start_delivery_processing` from being projected
as the current action. The action does not itself accept any candidate.

Implemented command `POST D/advance` accepts
`{command_id,expected_read_model_revision,action_id}`. The server may execute only the
exact local/durable action already returned as `authorized` by the read model. Actions
requiring a new model/image/package scope return an existing preview/approval URL and
do no work. B1 exposes only `prepare_delivery_schema` as `authorized`; it creates the
existing deterministic human Schema Draft and reports `replayed`. GET/mount never
calls this command. Exact replay uses the existing `source_request_id` receipt and is
restart-safe; stale revision/scope returns `409 task_revision_conflict`. Other action
states return the existing preview/confirmation URL and cannot be invoked through
`advance`.

## Errors and pagination

- Typed JSON decoding/unknown fields: 422; malformed JSON: 400; missing/foreign owned
  object is 404 where the existing route provides owner-hiding, otherwise a safe
  conversation contract error.
- CAS, stale snapshot, command reuse and changed authorization scope are 409 in new
  mainline routes. Existing routes that currently map some contract failures to 400
  retain compatibility until their error mapping is explicitly amended.
- Task/thread/navigation keyset pages use `next_cursor`; existing package consent
  history is bounded to 20 but not yet externally paged. B3 will use stable cursor
  pagination before claiming 1000-row history support.

Actual and planned DTO examples are in `EXAMPLES.json`; planned entries carry
`contract_status:"planned"` and must not be called until their delivery commit.

## ML-004 G0 semantic alignment (implemented)

`GET D/workspace` remains the single Task read route. No parallel Task API was
added. `workspace.mainline` now carries the G0 fields directly, alongside the
richer wire receipts retained for existing clients:

- `revision` is exactly `read_model_revision`. `intake.missing_slots` uses
  `dataset_scope|label_rules|training_target`; `label_rules` is the saved
  server `label_spec`, not a client reconstruction.
- `steps[]` includes stable `id`, `kind`, `title`, and G0 `status`, while the
  existing `state/request_completed/task_completed` evidence remains present.
- `actions[]` is the G0 projection of `available_actions[]`. Every action carries
  `scope_revision` plus the authoritative `method` and `url` (additive to the G0
  public type); clients never infer a route from `kind`. The richer
  `available_actions[]` is retained for compatibility.
- `active_operation_ids`, optional `review_work_item_id` (the Task UUID identifying
  its owned formal-review collection), optional latest `package_id`, and
  `completion {status,package_id?,download_url?}` are server-derived. Only a
  Ready package has `status:package_ready` and a download URL.
- `messages[]` contains only safe observable model-call `system_receipt`
  projections when such receipts exist. It includes stage/timing/typed failure,
  not raw Provider data or hidden reasoning. Persisted user text remains paged at
  the returned `links.thread`; the server still does not manufacture assistant
  replies.
- `links` is authoritative for self, thread, visual selections, capability
  readiness, formal review items, package consents and local advance.

`GET D/visual-selections` now places a complete G0 `selection` on every candidate
that has its own non-nil source Artifact. It repeats Project/Conversation/Task and
Schema identity, image ID/hash, Draft revision, Sample Test, candidate/Artifact,
annotation kind/label and that image result revision. A candidate without an
Artifact keeps `feedback_available:false` and `selection:null`.

`GET D/capability-readiness` now returns `setup_requests[]`. Each request is a
stable digest over Task Schema revision, Registry snapshot revision, role,
required capabilities and compatible candidate IDs. The server returns the exact
same-Project/Task `return_path`; `ready` requires current production-eligible
Ready evidence for every required capability. Unknown and TEST/Mock candidates
never make a production request Ready. Refreshing either GET only re-reads state
and cannot probe, install, grant, resume, plan, or call a Provider.

ML-015 keeps the planning request at `role:task_planning` and
`required_capabilities:[text_generation]`. P0 adds a separate
`role:visual_inference` request with one compatible requirement, never an AND-list:
it prefers a currently available detection/grounding capability and uses
`vision_language` as the bounded review-required fallback. Its status is Ready only
when exactly one usable Project visual binding is selected (preferably
`primary_inference`). Registry-wide Ready models are choices for setup, not implicit
data recipients. The response's
`visual_readiness_boundary` therefore says `awaiting_frozen_draft` before a Draft
and `validate_exact_draft` afterward, with the existing Builder/Sample preview
URLs. Those previews validate the actual selected nodes/bindings and permissions.
Neither setup request auto-selects a model or weakens production eligibility.

## Saved Journey sample continuation (implemented)

The Task read model distinguishes a saved grant from an admitted dispatch. New P0
consents are both saved and durably queued by their one approval POST. A completed
Builder with `outcome:draft_ready_for_human_review` is not a completed Sample and
does not authorize a new Builder:

- A legacy consent created before automatic admission can still return
  `id:test_pipeline_samples`, `state:requires_confirmation` and the exact saved
  Journey/Draft/Sample scope. `method:GET` + `url` reads the original consent;
  `execution_method:POST` + `execution_url` invokes its existing explicit
  execution boundary with `{}`. This prevents a software upgrade from executing
  old grants that were saved under a two-step contract.
- A new P0 consent, or any Journey whose durable dispatch is `queued|running`,
  returns `id:inspect_automatic_sample_progress`, `state:available`,
  `requires_confirmation:false` and the execution status GET URL. It never asks
  for a second Sample POST inside the same current authorization.
- The scope includes `journey_consent_id`, `sample_operation_id`, exact Draft
  ID/revision/content hash, ordered image IDs/hashes, allowed model binding
  digests, the saved maximum Sample calls and expiry. A revoked, expired, edited
  Draft, changed image or changed Registry destination returns the same action as
  `blocked` with `reason:saved_journey_sample_scope_stale`.
- Once that Sample Operation is durably reserved, refresh returns
  `inspect_pipeline_samples` with its exact project-owned GET URL. It does not
  offer another Builder or Sample. Failed/interrupted/cancelled Sample work is not
  automatically retried.
- `start_delivery_processing` appears only when the saved operation resolves to a
  real `WorkflowSampleTest` whose business status is `passed|human_approved` and
  whose current Draft still matches its exact revision/content hash. Operation
  transport status `succeeded` alone is insufficient.

The two existing routes used by this projection are:

| Method and URL | Request / response | Side effects |
|---|---|---|
| `GET D/journey-consents/K` | Original owned `ConversationJourneyRecord` including frozen consent and optional sealed Sample scope. | None. It does not claim a dispatch or create a Sample. |
| `POST D/journey-consents` | Exact preview consent → saved consent plus current dispatch. | A newly inserted, current P0 consent atomically queues one server-owned continuation. Exact replay returns the same identities. |
| `POST D/journey-consents/K/execution` | Empty object `{}` → current `ConversationJourneyExecutionStatus`. | Persists one `queued` execution intent, then one worker claims it as `running`. Existing Builder and Sample IDs are reused; active or already-created work is returned without redispatch. |
| `GET D/calls/Q/clarification` | `SchemaClarification {id,task_id,conversation_id,source_message_id,kind:"clarify_task",question,reason_code:"annotation_semantics_ambiguous",expected_schema_revision,schema_draft_id,status}`. | Passive. Repeated reads return the one saved question for call `Q`. |
| `POST D/human-schema-drafts` | `{request_id,decision:{decision:"draft",...},clarification:{call_id:Q,expected_schema_revision},journey_consent_id:K}` → saved revision-1 private Schema plus `journey_resume`. | Saves the linked answer, completes only explicitly supplied delivery semantics, and resumes the same Journey. No new Schema model call, publication or annotation acceptance. |

For an existing mid-Journey consent, clients render and confirm the server action,
GET its exact consent when a review screen is needed, then POST the returned
`execution_url`. A new upload-scoped Task instead GETs the parameterless
`journey-preview` and POSTs its exact consent once; saving that consent queues the
full Schema→Builder→Sample continuation. Clients must not construct new Journey,
Builder, or Sample UUIDs. Repeating GET is passive. Worker-capacity pressure remains
durably `queued`. Repeating either consent or execution POST returns current receipts
and cannot create another Builder or Sample. See `P0_AUTONOMY.md` for restart and
read-model fields.

The execution GET is an observer only, never a completion subscription. An immediate Provider can
finish Builder and Sample before the first GET; the first observation then returns the saved
terminal Sample. Exact consent/execution replays after that point retain one Builder operation, one
Sample operation and unchanged task-budget counters. The Sample report is the durable sandbox
artifact; this path does not claim or create a formal production Run.

When the saved goal says only `YOLO`, the Schema protocol treats detection,
segmentation and whole-image classification as materially different outputs. A
`decision:"clarify"` receipt therefore creates exactly one clarification object and
stops before Builder/Sample. GET, repeated consent, and repeated execution reads do
not create another question or call. An answer must bind `call_id` and
`expected_schema_revision`; its `decision` must state the selected output and, for
delivery work, the matching typed `delivery.training_target`. The resulting human
Schema freezes the completed delivery revision/hash and keeps the consent's original
ordered image IDs/hashes. A cancelled/stale clarification or a classification versus
detection mismatch is rejected before continuation.

The owned clarification GET now adds a compact answer contract when the saved model
receipt already contains complete label semantics and is missing only its output type:

```json
{
  "id":"SCHEMA_CALL_UUID",
  "status":"pending",
  "expected_schema_revision":"PROJECT_GOAL_SHA256",
  "question":"需要框出目标、描出轮廓，还是做整图分类？",
  "choices":[
    {"value":"bounding_box","label":"框住目标","supported":true,"unsupported_reason_code":null,"unsupported_reason":null},
    {"value":"segmentation","label":"描出轮廓","supported":false,"unsupported_reason_code":"segmentation_delivery_not_implemented","unsupported_reason":"当前交付运行时没有可发布的分割 Schema 与导出路径。"},
    {"value":"classification","label":"整图分类","supported":false,"unsupported_reason_code":"classification_delivery_not_implemented","unsupported_reason":"当前有界训练交付只支持 Ultralytics YOLO 目标检测。"}
  ],
  "answer":{"method":"POST","url":".../calls/SCHEMA_CALL_UUID/clarification/answer","required_fields":["command_id","expected_schema_revision","journey_consent_id","choice"]}
}
```

`POST .../calls/Q/clarification/answer` accepts only:

```json
{"command_id":"UUID","expected_schema_revision":"PROJECT_GOAL_SHA256","journey_consent_id":"UUID","choice":"bounding_box"}
```

It reconstructs the complete private Schema from the call's persisted label proposal and
the current owned DeliveryIntake, saves the exact clarification link, then resumes the same
Journey. It performs no new Schema model call and does not accept annotations or publish.
An exact `command_id` replay returns the same Schema. A stale revision returns
`409 schema_clarification_revision_conflict`; a reused/different command after an answer
returns `409 schema_clarification_answer_conflict`; a disabled choice returns its listed
stable reason code with `admitted:false`. Empty `choices` means the saved proposal lacks a
safe output-type-only context and the full Schema editor remains necessary.

Journey Sample execution resolves the frozen image IDs/hashes to current Project
indices at admission and repeats that check during execution. The index values are
presentation state, not authority. Later uploads or sort-order changes cannot replace
the images in the sealed Sample; missing or changed content stops with
`A Journey image is missing or changed; no Sample was started`.

### P0 upload identity and automatic Journey HTTP example

`POST /api/projects/P/conversations/C/send` accepts:

```json
{
  "message":{"id":"COMMAND_UUID","text":"标注这些图片中的杯子和瓶子，框住完整可见物体，用于 Ultralytics YOLO 目标检测。先给我看三张样例。","image":null},
  "task_images":[
    {"image_id":"IMAGE_UUID_1","sha256":"64_HEX_1"},
    {"image_id":"IMAGE_UUID_2","sha256":"64_HEX_2"},
    {"image_id":"IMAGE_UUID_3","sha256":"64_HEX_3"},
    {"image_id":"IMAGE_UUID_4","sha256":"64_HEX_4"},
    {"image_id":"IMAGE_UUID_5","sha256":"64_HEX_5"},
    {"image_id":"IMAGE_UUID_6","sha256":"64_HEX_6"}
  ],
  "task_id":null,
  "schema_revision":"PROJECT_SHA256",
  "mode":"plan"
}
```

`task_images` is optional for legacy sends. If present it is accepted only for a new
ordinary Task, must contain distinct same-Project current identities, and is frozen
in the original Send command. `POST /api/projects/P/image-upload?name=...` and
`POST /api/projects/P/import` return additive `images[]` entries containing
`image_id`, `relative_path`, `content_hash`, `width`, `height` and format.

For describe-before-upload, keep the original Task ID and user message, then submit the exact
upload receipts through the existing delivery CAS:

```json
{
  "command_id":"COMMAND_UUID",
  "expected_revision":0,
  "image_ids":null,
  "task_images":[{"image_id":"IMAGE_UUID","sha256":"64_HEX"}],
  "label_spec":null,
  "training_target":null,
  "split_policy":{"train_percent":80,"seed":0,"preserve_existing":true,"keep_known_groups_together":true},
  "image_metadata":{}
}
```

The Application checks every ID/hash against the current Project before the existing storage
transaction checks Task ownership and revision. Exact command replay returns the original
delivery revision. Reusing a command with a changed valid scope returns
`409 delivery_command_conflict`; a different command with stale `expected_revision` returns
`409 delivery_revision_conflict` with safe `expected_revision`, `current_revision`, `command_id`
and `suggested_action:reload_delivery_intent`. Changed content, a duplicate identity or a foreign
Project image is rejected before a revision is created. The resulting partial intake preserves
the original description and exposes the same parameterless combined Journey preview; it starts
no model or execution. GET and mount never attach images.

`GET D/journey-preview` needs no query for the new primary action. Missing/ambiguous
Project visual binding returns:

```json
{
  "status":400,
  "code":"capability_setup_required",
  "suggested_action":"configure_project_model_binding",
  "registry_revision":"SNAPSHOT_SHA256",
  "setup_requests":[{"role":"visual_inference","required_capabilities":["object_detection"],"status":"required"}],
  "eligible_project_model_ids":[]
}
```

With one binding, the preview returns exact `consent.allowed_models[]` binding digests,
all Provider destinations in `data.models[]`, `consent.images` capped at three, and
stable UUIDv5 command identities for this Task. The UI changes both
`consent.allow_unknown_cost` and `consent.schema_proposal.allow_unknown_cost` only
after the user confirms the displayed unknown-cost boundary, then POSTs the exact
consent. `200` means the consent and queue intent are durable; it does not claim a
model result succeeded.

### ML-021 executable visual route and failed admission projection

For a bounding-box conversation, Builder materialization now selects the existing
`vlm_detection.detect` operation when the frozen Registry has no eligible
`object_detection` Profile but does have an eligible `vision_language` Profile.
Selection is capability-based and deterministic; it does not infer a model brand,
label, or capability. An eligible object detector remains preferred when present.
The chosen Profile revision is retained on the Draft and the existing Sample
preview/execution checks remain authoritative.

If an exact Journey execution settles before reserving its Sample, the Task action
is no longer re-presented as approvable. It returns:

```json
{
  "id": "test_pipeline_samples",
  "state": "blocked",
  "reason": "saved_journey_sample_execution_failed",
  "failure": {
    "stage": "sample_admission",
    "category": "validation",
    "message": "safe persisted dispatch error"
  }
}
```

No Sample receipt is invented. A Draft saved before this change with an unresolved
placeholder remains immutable evidence and is not silently rewritten; it needs a
new explicitly authorized Builder/Journey. Fresh Builders can materialize the
capability-compatible VLM route. Once the original Journey execution intent is
queued, its worker observes Builder completion and reserves the original
`sample_operation_id`; no second human execution is required.

### P0 A6 result diagnostics

`GET D/workspace` now includes `mainline.result_diagnostics[]`. This is a passive,
additive projection over persisted call receipts, Sample reports and the embedded
Capability readiness snapshot. It never retries, installs, accepts an annotation or
converts an empty image into a negative example.

Every diagnostic has a stable `code`, `category`, `state`, exact `source`,
`automatic_retry:false`, `preserves_existing_results:true`, and a `safe_action`
with its actual method and route. Current codes are:

| code | persisted evidence | safe action |
|---|---|---|
| `model_weights_missing` | matching Registry candidate has `blocker.code=missing_weights` | inspect that model instance/profile setup route |
| `model_capability_unavailable` | a required setup request has no Ready compatible binding | inspect the exact Task Capability readiness route before configuring a binding |
| `provider_request_not_sent` | failed call has `failure.stage=prepare_request` | fix configuration, then obtain a new authorization; `provider_received:false` |
| `provider_outcome_unknown` | call status is `in_doubt` | inspect the call receipt and resolve explicitly; receipt is never auto-retried |
| `model_response_invalid_structure` | failed call category is `invalid_structured_output` | inspect the completed response receipt before separately authorizing another attempt |
| `legal_empty_detection` | persisted Sample image is empty, not failed, has no terminal candidate and no Provider/infrastructure/budget failure | inspect the saved Sample; `human_negative_recorded:false` |
| `candidate_projection_failed` | persisted Sample image includes `invalid_artifact` | inspect the saved artifact/report; other saved candidates/results remain unchanged |
| `authorization_expired` | current saved call grant is past `expires_at` | inspect `D/budget`; a read never renews or replaces the grant |
| `authorization_revoked` | current saved call grant is revoked | inspect `D/budget`; a read never restores permission |
| `task_call_budget_exhausted` | `used_calls >= maximum_calls` on the current saved call grant | inspect `D/budget`; spent and unknown calls remain counted |

Example:

```json
{
  "code":"provider_outcome_unknown",
  "category":"remote_outcome",
  "state":"blocked",
  "source":{"kind":"model_call","id":"CALL_UUID"},
  "stage":"settled",
  "failure":{"stage":"provider_request","category":"interrupted","http_status":null},
  "provider_received":null,
  "automatic_retry":false,
  "preserves_existing_results":true,
  "safe_action":{"id":"inspect_receipt_and_resolve_unknown","method":"GET","url":"D/calls"}
}
```

Capability readiness retains its own `registry_revision`; it is composed by the
server after the Application Task digest and does not silently invalidate an
unchanged execution grant. Calls and Sample records are already part of the
Application read-model digest.

An authorization blocker has this additive shape:

```json
{
  "code": "task_call_budget_exhausted",
  "category": "authorization",
  "state": "blocked",
  "reason": "saved_model_call_allowance_is_exhausted",
  "source": {"kind": "call_grant", "id": "GRANT_UUID"},
  "scope": {
    "task_id": "TASK_UUID",
    "maximum_calls": 1,
    "used_calls": 1,
    "expires_at": "2026-09-12T17:15:21Z",
    "revoked": false
  },
  "automatic_retry": false,
  "preserves_existing_results": true,
  "safe_action": {
    "id": "inspect_task_authorization_budget",
    "method": "GET",
    "url": "D/budget"
  }
}
```

### P0 A6/A7 deterministic HTTP scenes

The opt-in HTTP fixture seeds no production route. Its ignored Rust server test
idempotently writes ordinary owned Task, call-ledger and Sample-report records into
the marked isolated SQLite workspace before the Router is exposed. The browser then
reads every scene through the production `GET D/workspace` handler. A local Plugin
package with deliberately absent weights supplies the Registry evidence for
`model_weights_missing`; it is installed only in this explicit TEST workspace and is
never executed.

Start and retain a browser fixture:

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py \
  --enable-fixture --web-dist /absolute/path/to/existing/dist
```

Run once and stop owned processes after verification:

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py \
  --enable-fixture --smoke
```

The printed `manifest.json` contains
`diagnostic_scenes.scenes.<code>.task_url`, `workspace_url`, `task_id`, and the
exact observed `diagnostic`. Supported scene keys are:

```json
[
  "model_weights_missing",
  "provider_request_not_sent",
  "provider_outcome_unknown",
  "model_response_invalid_structure",
  "legal_empty_detection",
  "candidate_projection_failed",
  "authorization_expired",
  "task_call_budget_exhausted"
]
```

The two Sample scenes additionally publish `draft_id`, an owned `draft_url`,
`sample_test_id`, and `sample_test_url`. The Draft is a real persisted editing
Draft and the Sample Test freezes its actual revision and content hash. Production
pages may therefore follow either reference without suppressing a storage-integrity
error. The smoke and restart checks require both GETs to resolve and require the
Sample response to report `current:true`.

The test-data boundary is explicit: the fixture creates typed external outcome
receipts and Sample reports, while ownership checks, SQLite writes, Registry
readiness, budget math, restart recovery, and the HTTP projection execute the real
Rust code. It does not call a third party, retry a call, create a candidate, accept a
human decision, or touch a user workspace. Reusing the printed workspace with
`--workspace <TEST-path> --smoke` must yield
`seed_snapshot_unchanged:true` and `restart_verified:true`.
