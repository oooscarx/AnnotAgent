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
| `POST D/delivery-intent` | Saves `SaveTaskDeliveryIntent`; owner, image hashes and CAS checked. It never calls a model. |
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
  `POST
  /api/projects/P/processing-operations` requires the returned revision and
  authorization fingerprint. Its saved authorization includes the delivery intent
  revision/hash used later to identify formal child Runs.
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
one package job and exact retries never redispatch. Local capacity exhaustion leaves
the consent armed, so an exact review/consent retry can try again. No consent,
stale/cancelled consent or incomplete review means no admission. GET never admits.

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

When delivery intake is missing or partial, actions include both local
`save_delivery_intake` and `propose_delivery_semantics` with state
`requires_confirmation` and the existing text-only `schema-preview` URL. That Schema
call can propose only missing label/target semantics; it has no image pixels or image
execution authority and never saves the proposal automatically.

Step/result-message projection remains bounded by existing real thread and operation
receipts. Formal source, capability readiness, paged review items and package
admission are implemented.

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
