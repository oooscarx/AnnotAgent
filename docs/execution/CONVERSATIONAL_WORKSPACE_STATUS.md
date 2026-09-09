# Conversational Annotation Workspace — execution record

## Current checkpoint (2026-09-09 acceptance audit; not a completion declaration)

The default Project entry now uses the persisted conversation/image workspace. Explicit goal
selection survives re-entry, sample candidate references are frozen, and clarification/correction
requests can be saved, cancelled or (for corrections) deferred/reopened without resetting budgets.
The isolated bbox/classification tests exercise real Rust services through TEST HTTP model
transports, including correction, revision, formal confirmation, review and export.
Authorized valid Schema proposals now become editable Drafts without a separate local-save
click; idempotent retry preserves subsequent human edits and never issues another model call.
Stop/pause commands have a separately bounded admission lane under ordinary write saturation.

Initial goals and saved labels now offer one bounded authorization, backed by persistent
background dispatch and the existing Schema/Builder/Sample services. Valid first proposals
continue to samples without a second phase consent. Refresh restores owned work;
explicit phase-by-phase building and manual labels remain available.

New initial-goal consent explicitly allows continuing after its linked human clarification
answer within the original scope. Older consent does not acquire that permission. Answers save
before continuation admission; model/scope changes stop inference without discarding the answer.

Saved candidate messages now have a chat feedback card. Explicit, immutable one-call consent
is persisted before the existing text Provider dispatch; the card restores original scope and
receipts after refresh, exposes Stop, and opens the existing correction canvas on request.
Pending human work is opened directly instead of silently cancelled or bypassed to spend again.
Interpretation proposes human work only; it never applies geometry or formal annotations.

Scope clarification has controlled saved answers. Current-image class review now applies a scoped
atomic Sandbox correction; future-rule proposals produce a separate Schema only after explicit
acceptance, leaving the old scope intact. These are implemented in `conversation_image_class`,
`conversation_future_proposal` and `conversation_future_schema`, not merely saved intentions.
Standalone stop commands have frozen task/operation selection and recovery in `conversation_stop`;
the workspace parser distinguishes a stop command from an annotation request about a stop sign.
The earlier statement that these features were absent was stale and is superseded by this audit.

Formal processing and export now link saved task receipts to real immutable archive deliveries;
background export workers, transactional events, keyset history and URL restoration are implemented.
Thumbnail rendering is bounded to 24 images. A bounded browse-preview endpoint is implemented;
the dataset metadata API still returns a full index. The current full browser sweep includes
`conversation-previews.spec.ts`; do not infer that sweep's result from implementation alone.

Pending correction cards now allow an explicit bounded continuation grant before the answer.
Saving the real canvas answer atomically saves its delivery intent; the existing Builder and
sample services continue under that grant. A real SIGKILL/restart test verifies the original
operation resumes once, without another feedback revision or budget reset (027b8ea). An expired,
revoked or changed binding still blocks inference without discarding the saved correction.

Latest production code is 2502ab3. Full Rust fmt/clippy/test/build process 6393 exited 0
(713 tests passed, 6 explicitly ignored); see `/tmp/annotagent-rust-post-binding-20260909.log`.
Web typecheck, 231 unit tests and production build passed. The current 178-case browser sweep
37872 remains in progress, logged to `/tmp/annotagent-web-combined-20260909.log`; its terminal
result must not be inferred from individual passing cases. The earlier run numbers and counts
below are historical, not the current final result.

Default-goal discovery now uses one bounded owned query rather than downloading the full
message journal (6eee1e0). Dataset metadata still uses a full index; visible thumbnails and
decoded browse previews are bounded. New Draft review boundaries retain explicit task/label
identity through export (e4d1659, eba14b0, 2502ab3); historical immutable exports are not rewritten.
Run ownership no longer depends on the Project inventory's current page (909980d).

Remaining verification: terminal full-browser result and consolidated requirement/evidence
audit. Actual 200% browser zoom, native IME and assistive-technology checks are unverified;
automated reflow/composition tests are not substitutes. Live model quality and real-human novice
usability have not been tested. Point-only and general candidate-comparison request protocols
are not claimed; the implemented visual requests use reference boxes/classes and corrections.

## Consolidated verification map (current sweep still pending)

This map locates executable evidence; it does not substitute a test file's existence for
its result. The current full log is authoritative. TEST HTTP transports call real Rust
Application/Storage/Runtime services but provide scripted predictions, not model accuracy.

| Requirement group | Executable evidence / boundary |
| --- | --- |
| Default entry, persistent messages, input order, independent goals, frozen image references | `conversation-entry`, `conversation-workspace`, `conversation-goals` browser suites; bounded history unit/storage/server tests. Viewing and refresh do not authorize inference. |
| Automatic bbox/classification Schema, clarification, manual fallback | `conversation-schema`, `conversation-human-schema`, `conversation-initial-journey`, `conversation-clarification`; exact Schema/Builder/Sample receipts, not free text as business truth. |
| One bounded initial authorization and resumed clarification | `conversation-initial-journey`, `conversation-budget`; grants preserve model, image, revision, expiry and call ceilings across child services. |
| Missing model and setup return/cancel | `journey-model`, `journey-local-model` and task setup checks; fixture setup is not evidence of installing real model weights. |
| Scoped feedback, stale answers, no focus theft, future rules | `conversation-feedback`, `conversation-image-class`, `conversation-future-schema`, `conversation-future-schema-proposal`; new rules fork rather than mutate old Runs. |
| Reference box/class and correction as structured input | `conversation-reference-target`, `conversation-joint-repair`, `conversation-samples`; explicit geometry/class, subject and expected revision are validated and saved to Sandbox. |
| Answer delivery, idempotence and restart | `conversation-answer-restart`; real killed writer at Application commit/pre-dispatch boundary, production startup recovery, unchanged second-restart dispatch/budget. Not arbitrary Provider exactly-once billing. |
| Stop, independent tasks, cancelled/unknown inference | `conversation-stop`, `conversation-stop-executors`, `conversation-schema-background`, feedback/sample unknown-outcome cases; unknown external outcomes are not blindly retried. |
| Sample terminal results, correction, compare and formal separation | `conversation-samples` bbox/classification and human variants; samples do not become formal annotations merely by being corrected. |
| Immutable publication, processing retry, actual Review/export | `conversation-samples`, `guided-workspace`; actual archive payload checks include non-empty/non-unbound task identity. Existing snapshots are not rewritten. |
| Owner-scoped detail, management, deletion/recovery | `guided-workspace`, route unit tests; resolved Run owner works outside the paged Project inventory. Existing management pages remain reused. |
| History, previews, responsive layout and keyboard | `conversation-history`, `conversation-previews`, `conversation-workspace`, stop composition cases; native zoom/IME/screen-reader checks remain unverified. |

Fresh screenshots inspected from `/tmp/annotagent-conversation-combined-verified`:
`sample-bbox.png` shows the actual two-pane TEST workspace and editable terminal candidate;
`formal-export-bbox.png` shows one accepted annotation, zero unresolved reviews and a real
download receipt. The synthetic green image is a protocol fixture; its cup prediction is
not a correct semantic annotation and must never be used as accuracy or marketing evidence.
The export page clearly identifies the result folder as a server path, not the user's device.

Role separation remains: LLM proposes Schema/method or interprets bounded feedback; existing
validators/evidence logic assess feasibility and geometry risks; deterministic Runtime executes
the frozen Workflow; human UI supplies verified edits and explicit processing authorization.
No permission to install plugins, read credentials or delete history is added to the model.
Detailed historical commit hashes, object chains and failure/fix observations follow below.

## Direction and baseline (M0, 2026-09-08)

Latest request: `b9212e8e-88ee-40a7-af04-261d2adcf421/pasted-text.txt`.
This replaces the default page-by-page Journey, not its authorization, immutable
execution, geometry, management or Rust boundaries. Previous execution records
remain historical; this file is the only ongoing record for the new task.

Baseline: `main` at `07be406`, 40 commits ahead of origin; unrelated modified
screenshots preserved. No AGENTS.md found in the repository or ancestor checks.
No push, remote edits, real Workspace cleanup or old API keys. The running user
workspace on port 8787 was not restarted or migrated by this work.

Read current routing, AgentSession, storage migrations, Sample/Processing
operations, Review, Registry compatibility and actual package/test scripts.
`ARCHITECTURE.md` is historical in several places: its legacy adapter/secret
descriptions do not describe the current Registry and plugin implementation.
The existing `projects` SQLite table is also not the current Project catalog:
Application resolves stable Project identity from project.yaml, while images,
Runs and other records store that identity. New references must use this path,
not join against a legacy unpopulated table or infer ownership from a name.

### Fresh browser baseline

All below use explicitly labeled TEST transports and synthetic images, not live
model quality evidence. Tests use existing Playwright and Rust fixture tooling.

| Path | Current route/API chain and observed behavior | Reuse / gap |
| --- | --- | --- |
| A: configured model | `/projects?new=1` → `/task/goal` → planning consent/Builder → `/task/samples` → bounded sample operation → editable result. Images and goal persist; reload recovers exact Test/Image; direct corrections are sandbox feedback. | Reuse uploads, goal service, Builder, Sample operations and canvas. Current separate pages have no persistent user conversation or automatic schema proposal. |
| B: missing model | `/task/model` connects Provider/Model, explicit active probe and Project binding, then returns to goal/revision. Cancel retains saved task. Local model setup uses TEST metadata transport; it does not prove real installation. | Reuse Registry/setup components and consent checks. Return context must gain Conversation/Task/Request without resetting authorization. |
| C: existing Run | Owner-scoped Run → Review → source Run → Review; terminal results, no-target manual creation and failed-save preservation; Export readiness → real export report. Run Trash/restore remains accessible. | Reuse Review/Revision, terminal projection, export and management. No Human Request record or answer→resume outbox links these actions to a conversation. |

Fresh commands/results:

- `npm --prefix web run test:e2e -- journey-ready.spec.ts journey-model.spec.ts journey-local-model.spec.ts`: **3/3 passed**, 44.9s, workspace `/tmp/annotagent-guided-e2e-73164`.
- `npm --prefix web run test:e2e -- guided-workspace.spec.ts`: **36/36 passed**, 54.9s, workspace `/tmp/annotagent-guided-e2e-73883`.
- Example persisted baseline Run: `090f19ab-cbce-40a4-987b-24d16be1e828`, owner UUID `7b34a817-9b05-51a6-8a36-ae6f536e884e`, `completed_with_review` (read from isolated SQLite, not a client name).
- Captures: `conversational-workspace/before-{projects,sample,model-setup,manual-review,review,export}.png`. These are old default screens, not the requested new dual-pane UI.
- Non-target screenshot directories restored byte-for-byte from `/tmp/annotagent-conversation-baseline.aCGSwR` after capturing these scoped baseline assets.

## Reuse decisions

- Existing AgentSession owns Builder steps, Working Draft, Plan Candidates,
  budgets, proposal and termination. Its `pending_human_action: Option<String>`
  is insufficient for a versioned answer transaction and resume checkpoint.
- Keep Annotation, Sample Feedback/Revision, Workflow and Run in their existing
  stores. Message payloads contain references, not a second annotation database.
- Existing sample and processing receipts already handle admission, scope seals,
  durable budgets and publish-success/start-failure recovery. The task coordinator
  will call these services rather than create an executor or grant fresh budgets.
- A message journal sequence is not yet a task event/outbox. Do not claim SSE
  recovery or human-answer exactly-once application merely because messages persist.

## First implementation: message persistence boundary (M1 foundation)

Before implementation, the new storage test failed to compile because conversation
commands/types did not exist. Added migration 24 and bounded Rust journal methods:

- Explicitly create one main conversation per stable Project UUID; creation retry
  returns the same identity. Application must resolve the Project before calling.
- Append user message with caller idempotency UUID and optional frozen Image/hash.
  The same ID/body returns its original sequence; changed body/reference conflicts.
- Check image ownership and current hash before a *new* message. Retrying an
  already saved message returns its historical reference even after image change.
- Persist atomically, page by monotonically increasing sequence (maximum 100
  messages per read), and restore after SQLite reopen.
- Message DTO rejects extra authorization/annotation/role fields. This is not a
  replacement for server permission checks and does not itself authorize inference.

Validation: 41 storage unit tests, all-target/all-feature storage Clippy and fmt
check passed after the final legacy-project-catalog correction. Rust compilation
initially required adapting sequence storage to SQLite's signed integer contract.
Screenshot inspection caught the model setup capture mid-fade; its existing test
now disables animations for capture rather than misrepresenting faded text as the
settled design. The focused model test passed 1/1, 24.1s; its settled capture was
inspected and replaced the initial faded baseline image.

## Application and HTTP journal integration (M1 foundation, continued)

Explicit long-running Conversational Workspace goal is now active; no token budget
was requested. Added Project-owned create/append/page use cases and existing-router
HTTP endpoints. Ownership uses `project_path` canonical containment + schema load
and the same stable directory-derived Project UUID as images/Runs. No legacy
catalog join, mutable display-name matching or client-provided owner is involved.

`POST /api/projects/:project/conversations` explicitly creates/reuses the main
conversation; `GET/POST .../:conversation/messages` reads/appends the journal.
GET never creates it. Existing global session/CSRF, same-origin and body limits
cover these routes. Messages grant no execution authorization, change no Schema
and call no model. The UI and coordinator are not connected yet.

Validation: targeted Application ownership/restart/idempotency test **1/1 passed**;
targeted HTTP authentication/ownership/idempotency/unknown-field test **1/1 passed**;
server check, Application+Server all-target/all-feature strict Clippy and fmt passed.
The first HTTP test expected 403 for a request without a session; actual existing
security correctly returned 401. Corrected the test, not the security middleware.
Only temporary workspaces were used. No paid inference or user server restart.

## Remaining stages and limits

### Dual-pane journal and images slice (M1, continued)

Added typed `/projects/:project/work?conversation=:id&image=:id` with existing
FocusHeader/Project menu, no global sidebar, independent desktop divider (pointer
and arrow keys), and mobile conversation/images switch. It uses existing images
and upload APIs plus the journal endpoints. Read-only main-conversation discovery
returns null before explicit first send; mount/reload never creates a record.

This is an explicitly labeled **integration-in-progress** deep-link, not yet the
default Project entry. It saves user messages rather than fabricating Agent replies;
automatic planning, annotation overlays and a Task coordinator are not connected.
The new view currently uses English UI copy; localization remains before default
rollout. Management stays in the existing Project menu and canonical routes.

Goal-before-upload, frozen image/hash on send, network-response loss + same-ID
retry, reload, deep-link image selection, accessible divider and no-inference
behavior passed a real isolated HTTP/browser test. Existing image deduplication is
reused. Pending text has a navigation/unload guard; same-workspace image selection
does not discard it. Historical image refs reject changed/missing current content
instead of representing new pixels as old evidence. This guard is implemented but
its browser mutation/conflict coverage remains to be expanded.

Evidence: `conversation-workspace.spec.ts` **1/1 passed**, 12.7s on final rerun,
workspace `/tmp/annotagent-guided-e2e-76636`. Captures `journal-{1440,1280,1024,390}.png`
show TEST synthetic input, **not annotation/model quality evidence**. Desktop and
mobile captures visually inspected; native file button styling adjusted afterward
and captures regenerated. Web **105/105** unit tests, server journal HTTP **1/1**,
production build/typecheck, strict storage/Application/server all-feature Clippy
passed. Existing production chunk-size warning remains. One intermediate rerun
was stopped by TypeScript's nullable image reference check; corrected explicit
reference narrowing and reran successfully. No real server restart or paid calls.

M0 is not fully closed: Human Request transactional answer/resume failure tests
and task-authorization admission tests still need to be added at their real service
boundaries. The journal is exposed through Application/HTTP and an opt-in UI slice.

### Task admission identity (M1/M2 boundary, continued)

Inspection found the legacy Builder `retry_session_id` explicitly starts fresh
budgets. The conversation coordinator must not treat that endpoint as a task-wide
retry ledger. Added migration 25 and Project-owned task admission/list endpoints:
`GET/POST .../conversations/:conversation/tasks`. These currently persist identity
and references only; they do **not** call Builder or grant inference permissions.

Each task binds a saved message (FK scoped to its Conversation) and exact Project
goal/schema revision hash. Missing/foreign messages and cross-Project conversations
are rejected. Admission locks against existing in-process Schema writes. One saved
goal message has one root task: repeating with the same or a fresh client request
ID recovers it; conflicts in schema/message/ID reject. Global ID collision takes
precedence over the valid same-message recovery path. GET is read-only. New task
admission rejects stale schema; historical admission retries recover the old
reference without implying the old task may run against the changed Project.

Validation: storage **43/43**, focused Application ownership/stale-schema/restart
**1/1**, HTTP journal+task routes **1/1**, storage/Application/server all-target
all-feature strict Clippy and fmt passed. Only temporary workspaces. No UI changes
or new browser screenshots in this checkpoint. No new paid calls or server restart.

Remaining before task execution: durable task-level authorization/call reservations,
child Builder budget propagation, immutable Schema proposal/snapshot and structured
Human Requests. The current task record is only an identity/reference, not a saved
Schema body, task-state machine or permission receipt. These limits prevent it being
presented as working automatic planning in the UI.

M1: wire existing Project resolver, messages and typed Task/reference context to
the real two-pane workspace, upload and existing terminal results. Do not redirect
default navigation to an empty or fake chat shell before the vertical slice works.

M2: schema proposal/patch with real configured LLM, existing Builder, shared bounded
authorization and sample execution; classification and bbox must both work.

### Bounded Schema proposal service (M2, continued)

Added Application proposal service over existing `VisionModelProvider::complete`,
not another Agent or executor. It sends one bounded text-only request (saved goal
and existing TaskConfig context) and offers one controlled Schema proposal tool.
Model output must be either a bbox/classification Draft or a necessary clarification.
Labels, attributes, boundary rules and rationale have explicit type/size validation;
duplicate labels, unsupported output kinds, unknown privilege fields and multiple
tool actions reject. Stable internal task IDs derive from the admitted task UUID.
Conversion returns the existing Core TaskConfig; it never writes Project YAML,
Published Versions or annotations. Boundary rules remain in the proposal, not lost
by pretending TaskConfig alone captures the entire semantics.

Provider response/usage is retained even when decision validation fails. There is
no automatic repair/retry or additional model call. Prompt context is user-data
role, not extra system instructions; no images are sent or visual findings claimed.
This service is **not HTTP/UI-wired yet**: its caller must supply a durably authorized
and budget-limited Provider. Do not use the unconstrained legacy retry endpoint to
work around the pending task-wide authorization/reservation integration.

Validation: **3/3** isolated TEST Provider tests cover bbox, Chinese indoor/outdoor
classification, stable Core IDs, single text-only call, clarification, invalid output
and preserving 200 fixture tokens/request evidence on failure. These scripted tests
prove contracts, not real LLM schema inference or live quality. Initial compile
corrected fixture usage to the existing explicit `UsageSource::Mock` contract;
strict Clippy caught documentation/constructor style and passed after correction.
No new frontend behavior or screenshots, no paid calls, no user workspace mutation.

M3: structured Human Requests, sandbox versus formal answer transactions, revision
conflict handling, durable resume event/outbox and no extra "I am done" chat step.

### Durable call allowance and Schema execution (M2, continued)

Added migration 26: one immutable explicit call grant per task and one immutable
request-hash receipt per logical Provider call. Ownership resolves through task →
conversation → stable Project identity. New reservations atomically check scope,
expiry, revocation and total prior reservations; failed/unknown requests are not
refunded. Repeating a grant cannot enlarge/reset it or undo revocation. Reusing a
call key returns its receipt, including `reserved` after an interrupted observation;
it does not grant another execution. Terminal evidence cannot be overwritten.

Application Schema execution now consumes this ledger before calling the existing
Provider, records complete response/decision evidence, and returns persisted receipts
on duplicate execution or restart. Project/schema changes block new execution.
Provider failures conservatively record `in_doubt`, not a zero-cost success or an
automatic retry. Receipt reads remain possible independently of inference.

Scope limits: this ledger currently enforces **logical Provider call counts**, not
a monetary hard cap or transport-level exactly-once guarantee. HTTP admission must
resolve the Registry binding and derive its complete model/data scope hash server-
side; it is not wired yet. Existing OpenAI-compatible adapters can retry transport
requests internally, so this task path must disable those retries or charge each
attempt before enabling live execution. Builder/Sample adapters must still join
the same task allowance. Explicit scope/budget extensions are not implemented and
are rejected; no silent grant replacement. Reserved-after-crash is intentionally
not automatically rescheduled. Outbox-driven Human Requests remain separate work.

Validation: storage ledger test covers unapproved/foreign/changed requests, crash
reopen, duplicate and immutable settlement, exhaustion, revocation and no reset.
Application Schema tests **4/4 passed**, including zero calls without grant, one
call with grant, duplicate+restart returning the saved response, exhausted new
call rejection and no Project Schema mutation. Storage/Application strict all-
feature/all-target Clippy passed. All tests use TEST fixtures, not Live Providers.
No frontend changes/screenshots or restart of the real user workspace.

M4: exact publication/Batch cards, Review/export and management returns, reconnect,
performance, split-pane keyboard/narrow layout, full regressions and two golden paths.

### Registry-backed Schema HTTP consent boundary (M2, continued)

Added Project/Conversation/Task-owned `schema-preview`, `schema-proposals` and call
receipt GET routes. Preview resolves the existing planning-model Registry binding,
checks task ownership/current Schema and hashes actual model revision, Provider
configuration, task identity and text-only scope. Public output contains model,
destination, one-call cap, output-token bound, expiry and **unknown cost**, not secrets.
POST requires explicit unknown-cost acknowledgement and the unchanged scope digest;
credential resolution reuses the existing secret service. It authorizes one Schema
completion, not dataset processing, publication or annotation acceptance.

This path forces OpenAI-compatible `max_retries=0` before creating the existing
Provider, so one reservation cannot hide that adapter's transport retries. The
endpoint is included in the existing expensive-operation concurrency limiter.
Only POST can call the Provider; GET restores immutable receipts. Same consent/key
returns the same saved result; a fresh key cannot silently replace the task grant.

Actual isolated HTTP test (`conversation-schema.spec.ts`) **1/1 passed**, 19.2s,
workspace `/tmp/annotagent-guided-e2e-78198`. It traverses API → Registry credential
resolution → existing OpenAI-compatible HTTP transport → explicitly scripted TEST
fixture → durable receipt for both bbox and Chinese classification. It checks no
consent/stale scope rejection, identical POST/GET recovery, fresh-key rejection,
retained usage and unchanged Project Schema. Fixture refuses image-url input.
This is protocol integration evidence, not Live LLM inference/accuracy evidence.
Server/Application/fixture all-target/all-feature strict Clippy and production
web build passed; existing chunk-size warning remains. No new screen capture: the
conversation UI has not yet been wired to this operation.

Before default UI rollout: add directly reachable cancellation and task progress,
then bind the consent card and persisted proposal card to these endpoints. Explicit
additional authorization must extend the existing task ledger without resetting
spent calls before Builder and image testing can proceed. Current one-call Schema
consent deliberately cannot authorize those later phases. Full task cancellation,
budget extension, Schema Draft persistence/application and Builder linkage remain
unfinished; they are not substituted by this HTTP slice.

No live Provider validation, new UI usability claim, accuracy claim or human-user
testing has been performed. Native zoom and OS assistive-technology checks remain
unexecuted. No new model, Python Worker, autonomous install or permissive tool API.

### Conversation consent/result cards and stop control (M2, continued)

The opt-in workspace now restores the first goal message's admitted task and real
call history. Explicit prepare resolves the existing Registry preview; a separate
unchecked consent box gates the single text request. The response card is rendered
from the saved receipt's validated decision, not assistant prose. Reload/mount only
GETs task/call state and does not invoke the Provider. Rejected admission is not
displayed as running. This remains an integration view: it explicitly says the
proposal has not changed Project labels or started annotation.

Added Project-owned call history and cancel endpoints plus a distinct cancellation
token registry (not shared IDs with Builder sessions). Active call cancellation
revokes further allowance and signals the existing Provider token. A post-reservation
grant check closes cancellation between reservation and token registration; cancelled
before-send work does not invoke the Provider. Interrupted reservations become
`in_doubt` during Application startup without model replay. GET does not reconcile
or mutate state. Leaving the component does not cancel the server request.

Limits still requiring hardening: cancel before a server reservation exists returns
not-found rather than recording a durable cancellation intent; an abandoned request
whose handler disappears without server restart can remain reserved. These cases
must be fixed before default rollout. The current active-token cancellation test
does not prove those edge cases. No durable Human Request/outbox exists yet.

Evidence: two isolated browser tests **2/2 passed**, 20.0s, workspace
`/tmp/annotagent-guided-e2e-78745`. They exercise the actual consent checkbox→HTTP
Provider→saved card→reload path plus journal/upload/idempotency regression. Updated
`journal-*` and added `schema-{1440,390}.png`; desktop/mobile Schema captures inspected.
The empty image panel in Schema captures deliberately tests goal-before-upload,
not a finished annotation result. Application Schema **4/4** tests now also verify
active Provider cancellation yields retained unknown-outcome evidence and removes
the token handle. Web **105/105**, strict storage/Application/server Clippy and
format checks passed. No Live credentials/models, real workspace restart or push.

### Cancellation race closure (M2, continued)

The two cancellation gaps above are now addressed. Migration 27 persists an
owned cancellation intent independently of a call receipt. Cancellation may arrive
before admission; the eventual same-ID request is rejected before reserving/sending.
The intent endpoint never manufactures a model receipt or an inference result.
Intent retries are idempotent and survive reopen. Cancelling an individual call
does not revoke unrelated future task phases; the per-call intent plus live token
controls this operation. After reservation, registration checks the saved intent.

The handler guard now conditionally settles only still-reserved calls as `in_doubt`
when its future is dropped; completed evidence is untouched. This covers an abandoned
handler without requiring a server restart. Crash startup recovery remains in place.
UI restores cancellation intents separately from call results, stops implying an
unregistered request is running, and does not offer a cancelled nonce as a new call.
It still cannot resume/extend an explicitly cancelled task into Builder; that phase
needs the planned task-level authorization revision rather than bypassing this guard.

Evidence: targeted storage ledger test passed (pre-admission cancellation, identical
intent retry, foreign rejection and reopen); Application Schema **4/4** passed with
an added dropped-pending-future case and active cancellation regression. Strict
storage/Application/server Clippy passed. `conversation-schema.spec.ts` **1/1**,
19.6s, workspace `/tmp/annotagent-guided-e2e-79690`, now holds the real outgoing POST,
cancels before server admission, releases it to a verified 400 and reloads the saved
cancellation without a proposal. `schema-cancelled.png` captured and inspected.
Existing bbox/classification consent→result browser checks also passed in that test.
No Live calls, real workspace changes, default navigation switch or push.

### Independent semantic Schema Draft revisions (M2, continued)

Migration 28 stores a separate conversation Schema Draft with immutable numbered
revisions, its source call, task owner and original Project schema digest. Initial
materialization revalidates the persisted model tool response, derives the existing
Core TaskConfig and freezes the saved goal. Clarifications/invalid or unfinished
responses cannot manufacture a draft. No Project YAML, published workflow or formal
annotation is updated. Human edits accept bounded schema semantics, never arbitrary
model bindings, validators, refiners, dependencies or execution permissions.

Creation retries preserve the latest human edit, not overwrite it with revision 1.
Editing uses an expected revision plus idempotency key; stale/conflicting requests
fail while exact retries return their original saved revision. Exact old revisions
remain readable. Owner checks, restart persistence and revision integer overflow are
covered. HTTP exposes explicit POST materialization/edit and read-only revision GET.

Evidence: Application Schema tests **4/4** (expanded with revision/retry/foreign-owner/
reopen cases), storage **44/44**, fmt and strict storage/Application/server Clippy
passed. Isolated `conversation-schema.spec.ts` **1/1**, 24.9s, workspace
`/tmp/annotagent-guided-e2e-80711`, exercised actual HTTP bbox/classification proposals,
draft materialization, edit retries, stale edits and historical reads. Existing
consent/cancellation UI regression also passed. Production Web build passed with the
existing >500 kB chunk warning. TEST HTTP fixture only; no live model inference.

Still incomplete: editing these revisions in the conversation UI and explicitly
binding them to existing Workflow Draft/Builder/sample execution. The current
Builder still reads Project YAML; it must not be advertised as consuming this new
Schema Draft yet. Shared later-phase authorization, Human Request/outbox and the
complete sample→processing→review→export journey remain active work. No default
navigation switch, real-workspace restart, push or human usability test occurred.

### Conversation Schema editing and read-only restoration (M2, continued)

The conversation card can now explicitly materialize a saved proposal, edit labels
and boundary rules, and restore its current revision through a new read-only lookup
by owned task/call. Mount and reload never materialize a draft. The editor preserves
the proposed output kind, multi-label setting and attributes; editing these additional
fields in the UI remains pending. The model proposal remains historical evidence,
separate from the editable revision. Neither is advertised as a running Pipeline.

Unsaved edits participate in the workspace dirty guard and browser unload warning.
Saving uses a frozen edit/idempotency key. If the server saves but its response is
lost, the typed labels remain visible and retry returns the same revision. Rejected
edits retain input, with a deliberate reload-latest-base action for revision conflicts.
Initial browser testing exposed an implicit textarea label lookup problem; explicit
unique input/label association fixed it and the same test passed afterward.

Evidence: Web typecheck and **105/105** unit tests passed, Application Schema **4/4**
passed, strict storage/Application/server Clippy passed. Isolated browser test **1/1**,
7.0s, `/tmp/annotagent-guided-e2e-81466`, verifies materialization, editing, refresh with
zero POST requests, and server-save/response-loss→same-request retry→revision 3 (not
4). Existing transport consent, revision API and cancellation checks also pass.
Updated `schema-{1440,390}.png`; mobile screenshot inspected for wrapping and input/
action spacing. These remain TEST goal-before-upload screens, not live visual quality
evidence. Production build has the existing large-chunk warning. Full workspace
regression, actual Pipeline schema binding, human requests and complete golden paths
remain pending. No real workspace data, remote or server restart was changed.

### Frozen Workflow semantics binding (M2, continued)

WorkflowDraft now carries an optional semantic-only Schema binding (source draft,
exact revision, goal, task and boundary rules). Absent bindings are omitted from
serialization and both custom hash materials, preserving legacy content identity.
Present bindings participate in authoring and publication snapshot hashes. Loading
an editable binding verifies ownership and equality to its immutable stored Schema
revision; injected task definitions are rejected. A bounded Application binding
command uses existing Workflow optimistic saves, never Project YAML mutation.

Existing save/label compilation, static/geometry validation and label-pipeline sample
execution resolve the bound schema. Published DAG execution applies its frozen copy
to the per-image Project request, without changing dataset, review policy or runtime
settings. Retry/Improve Existing Builder contexts consume those semantics; Builder
identity patches and label recompilation preserve the binding. Boundary rules are
included in the effective annotation goal. Older editors omitting the optional field
cannot silently clear it. Existing workflow ownership cannot be changed on save.

Regression proves Schema revision 2 does not modify a Workflow bound to revision 1,
binding changes alter content hash, frozen snapshots retain old semantics, forged
and foreign bindings fail, and an older editor round trip preserves binding.
Project labels and safety policy remain unchanged. This is backend integration,
not yet the complete conversation→authorized Builder→sample golden path: initial
workflow creation from conversation and the shared multi-phase allowance are still
pending. No UI button claims that path works yet. Dedicated bound-schema model-input
inference evidence must still be added beyond the current binding regression.

Evidence: `cargo check --workspace --all-features`, strict workspace/all-targets/
all-features Clippy, workspace/all-features tests and build passed. Existing opt-in
Live/real-weight tests remain ignored (SAM, YOLOX, PIDNet, RF-DETR and environment-
dependent application smoke), not claimed as inference verification. After the final
old-editor/compilation preservation edits, Application/all-features tests reran:
**89 passed, 1 ignored**, and targeted Schema **4/4** plus strict Application Clippy
passed. No new screenshots: this change is backend-only. Full goal remains active;
no push, remote edits, real-data writes or real-user usability testing.

### Cumulative phase authorization and Provider adapter (M2, continued)

Migration 29 records immutable authorization revisions and each call's original
grant. Explicit next-phase consent uses an expected previous grant, retains all
spent/failed/unknown calls and changes only the cumulative ceiling/scope/expiry.
Active reservations prevent scope replacement. Stale confirmations cannot fork a
grant chain, exact retries do not reset budgets or revive revoked authority, and
historical call retries still resolve against their original scope after advancement.
Existing single-phase rows are backfilled without changing their call receipts.

The Application now provides a task-ledger adapter implementing the existing
VisionModelProvider trait for Builder text completions. It checks the fixed model,
rejects images, reserves before dispatch and retains completed or unknown receipts;
dropping an unresolved handler conditionally settles it as unknown. Caller must
still disable underlying transport retries. This counts completion calls, not an
enforced monetary cap, and is not yet wired into the conversation Builder launch
endpoint. Native vision tools/sample execution still need the shared allowance
integration; no claim that all task phases are already covered.

Evidence: storage **45/45**, Application Schema **4/4** and strict storage/Application
Clippy passed. Tests cover old-scope receipts after restart, no spend reset, two
additional calls under a cumulative cap of three, active-call transition rejection,
foreign owners, revocation and stale/idempotent consent. Adapter regression starts
with one consumed Schema call, explicitly advances to a total of two, rejects an
unapproved model/image without dispatch, allows one further completion and rejects
the next before calling the fixture. Isolated conversation HTTP/browser regression
**1/1**, 18.8s, `/tmp/annotagent-guided-e2e-83700`, also passed after the migration.
Production Web build retains its known chunk-size warning. No Live credentials,
real workspace migration/restart, push or real-user testing was performed.

### Existing Builder execution under a conversation operation (M2, continued)

Migration 30 adds idempotent Builder orchestration receipts, separate from billable
call receipts. One operation ID identifies its existing Agent Session and working
Workflow Draft. Duplicate admission returns saved state; startup/abandoned handlers
mark unfinished operations interrupted without redispatch. Late settlement cannot
rewrite a terminal receipt. Existing AgentExecutionGuard still settles its session.

`build_conversation_pipeline` resolves an exact owned Schema revision, applies it to
the existing Registry advisor input, and invokes the existing Builder loop with the
task-ledger Provider. It does not implement another planner or executor. Remaining
cumulative calls bound Builder turns; samples are explicitly disabled for this phase
(`maximum_dry_runs=0`), and no publication/Run authorization is granted. A server-owned
optional Session ID lets this caller correlate state without changing legacy callers.
Retries read the saved operation instead of regenerating a Draft or calling a model.

Evidence: Application/all-features **89 passed, 1 ignored**, targeted Schema **4/4**,
and strict storage/Application/server Clippy passed. The expanded orchestration test
used exactly **2** scripted model completions through the real Builder loop after
Schema/edit/explicit allowance advancement, retained the edited label in the generated
working Draft, did not publish and repeated the same operation without another call.
The scripted Provider only inspects context; this proves bounded execution and state
linkage, not a runnable quality pipeline or Live inference. Storage operation regression
passed for duplicate/hash-conflict/foreign-owner admission, restart interruption and
terminal result protection. No browser evidence added for a backend-only change.

Next required integration: server consent preview/launch/status/cancel routes plus
conversation UI, then authorized sample execution and terminal-artifact canvas. The
current operation is not exposed as a working UI action yet. Saved interruptions are
recoverable evidence, not automatic resumed execution. Human Request/outbox, complete
golden paths and real-user usability remain incomplete. No real workspace mutation,
server restart, push or remote changes.

### Builder HTTP consent, history and pre-start cancellation (M2, continued)

Project/conversation/task-scoped Builder preview and operation routes now resolve
the real Registry text model and endpoint, freeze the exact semantic revision,
disable transport retries, cap output tokens and require an explicit unknown-cost
acknowledgement. Preview declares zero image pixels and no dry-run/publication/Run.
Consent advances the existing cumulative grant; historical authorization reads keep
its original predecessor available if the same request must be recovered. Completed
operation POST retries validate their request hash and return saved state without
resolving credentials or advancing the grant again. GET history is read-only, limited
to the latest 32 owned operations and joins only same-project Agent Sessions.

Builder cancellation uses the durable pre-admission intent and a registered live
token. A stopped-before-start operation is interrupted without a fake Session or
model call. Guard cleanup removes the live token. Cancel rejects another task's
existing Builder identity; operation admission also rejects IDs already used by a
Workflow, Agent Session or model call. The Schema card now restores the first Schema
receipt rather than accidentally displaying a later Builder call as its proposal.

Evidence: targeted storage operation **1/1**, Application Schema **4/4**, strict
storage/Application/server Clippy passed. Isolated HTTP/browser test **1/1**, 23.1s,
`/tmp/annotagent-guided-e2e-84665`: both bbox/classification proposal→editable revision
→Builder consent→existing Builder→saved operation/session→identical POST retry pass.
Pre-start cancel is exercised for each type: HTTP returns 400, operation is interrupted,
Session is null and model-call history is exactly unchanged. Fixture-driven Builder
results are not evidence of Live model quality or a complete annotation pipeline.
Production Web build passed with the known chunk-size warning. No new UI launch button
or screenshots in this backend step; the existing Schema UI regression still passes.

Remaining: bind these interfaces into the conversation card, repair task-scope history
paging beyond 32 operations, verify active Builder cancellation in browser, and finish
shared sample allowance/canvas/human requests/processing/review/export. Expired or
changed-registry POST retries may require GET recovery and a new explicit consent;
they never automatically re-execute. No real workspace migration/restart, Live calls,
push, remote mutation or human usability validation occurred.

### Conversation Builder card (M2, continued)

The editable Schema card now offers a real Builder consent panel, with actual model/
endpoint, no-image scope, previous spend, bounded additional calls, cumulative ceiling
and unknown cost acknowledgement. Starting calls the implemented HTTP operation;
progress reads its real Agent Session phase/next action. Stop remains directly visible.
Model work is never started by mounting, polling or reload. Polls are read-only and
locally deduplicated. A lost completion response is recovered from operation history;
known pre-admission rejection no longer leaves the screen permanently "running".

Saved outcomes show setup requirements rather than implying a runnable plan. The
existing project-scoped Pipeline details deep link carries its Draft and Session.
Builder history now reports the frozen Schema revision; changing saved labels does
not relabel an older operation as using the latest revision. Schema edits cannot be
submitted as a new build while the edit is unsaved. Starting another operation clears
the previous local Session display so its phase cannot masquerade as new progress.

Evidence: Web typecheck and **105/105** unit tests passed, strict Application/server
Clippy passed. Isolated browser **1/1**, 7.4s, `/tmp/annotagent-guided-e2e-85369`, exercises
Schema revision 3→consent checkbox→real Builder HTTP→saved outcome and scoped detail
link. It drops the completed POST response and restores via GET, verifies reload sends
zero POST requests, and verifies a held Builder POST can be stopped before admission,
settles interrupted and stays stopped after reload. Updated desktop/mobile Schema
screenshots and added `builder-cancelled.png`; mobile outcome wrapping inspected.
These are deliberately TEST goal-before-upload/setup-required results, not Live model
quality evidence. Production build retains its existing chunk-size warning.

Remaining: successful sample inference/terminal canvas in this workspace, structured
human feedback/outbox, resumable repair of existing Builder operations, native-model
shared budget, complete processing/review/export and global default rollout. The
current "another build" is explicitly a new Draft request, not a resumed repair.
Active in-flight Provider cancellation still needs dedicated browser verification.
No real workspace restart/data changes, push, remote changes or real-user usability
test occurred. The full objective remains active.

### Durable vision-call accounting adapters (M2, continued)

Added `ConversationVisionCalls` over the existing sample Provider, HTTP Vision Backend
and native Pipeline Backend wrappers. Admission uses the same SQLite task ledger as
Schema/Builder calls. Only request digests and compact receipts are added; image/mask
payloads are not duplicated into conversation evidence. Successful calls complete their
receipts; failures and dropped futures remain consumed with unknown outcome/cost.
Pre-cancelled requests now fail before admission for all existing sample wrappers.

This is accounting infrastructure, **not a new execution authorization or a connected
conversation Sample Test endpoint**. The caller still must freeze and validate exact
Draft/model/image scope and disable transport retries. Existing sandbox and Batch
paths keep their current allowance; no real workspace configuration was changed.
The next integration must compose per-test caps with this cumulative allowance and
cover late-created native adapters, not overwrite one allowance with another.

Evidence: two new isolated Rust regressions cover 12 concurrent native requests with
only two remaining calls, pre-admission cancellation, durable reopen/exhaustion,
unknown non-refunded outcomes, wrong scope and revocation. Application all-features
tests: **91 passed, 1 explicitly billable smoke ignored**. Application/server strict
all-targets/all-features Clippy passed. Existing sample allowance tests passed.
No browser changes or new screenshots this increment; no Live inference, real-user
usability validation, push or remote modification. Sample canvas, human requests/outbox
and the processing/review/export conversation flow remain incomplete.

### Compose sample and conversation allowances in the existing Runtime (M2)

`SampleExecutionControl` now accepts a server-owned optional conversation accounting
context. The existing executable sample sandbox passes it to PublishedWorkflowRuntime,
which applies one composed allowance to existing and late-created model adapters.
Both the per-test ceiling and cumulative task ledger apply; starting a new sample does
not replenish task spend. The application rejects a foreign Project accounting context,
missing frozen-scope callback and unmetered legacy fallback before execution.
The ordinary HTTP sample route still supplies no conversation context: connecting its
exact scope/consent/recovery contract to conversation tasks remains next work, not done.

New adapter regression verifies the local ceiling survives late adapter construction
and a second sample cannot reset cumulative spend. A separate isolated application
classification test uses the existing sandbox, produces one terminal candidate, keeps
formal committed annotations empty and does not publish. Its built-in mock classifier
does not invoke any model adapter, so the expected model-call count is correctly zero;
it is not claimed as metered HTTP/Live inference evidence. The latter still needs an
HTTP-fixture integration after the consent route is connected.

Verification: Application all-features **93 passed, 1 billable smoke ignored**;
Application/server all-targets/all-features Clippy passed. No browser or Live run in
this increment; user data and unrelated dirty screenshots preserved, no push.

### Conversation sample consent and history API (M2)

Extended the existing project Sample Operation POST with an optional typed conversation
consent. No parallel executor/table was introduced. A task-scoped read-only preview
combines the existing actual model/destination/image/Draft fingerprint with a frozen
conversation/task/Schema owner, operation ID and cumulative allowance. Start requires
explicit unknown-cost acknowledgement, nonexpired consent and the same scope; the
existing worker receives the composed Runtime allowance. Provider transport retries
are disabled for this consent path. Bounded HTTP prompted-segmentation adapters also
disable internal retries, so one admission cannot conceal several transport requests.

The existing operation request stores its consent alongside execution inputs, while
non-conversation request serialization remains unchanged. Exact repeated POSTs read
the original receipt before resolving credentials or advancing a grant. GET history
indexes the same Sample Operations by Project/conversation/task (latest 100; pagination
still pending). An interrupted gap between grant persistence and operation admission
can reuse the historical previous grant without increasing the ceiling again.

Evidence: Server all-features **36/36** and Storage sample-operation **4/4** passed;
strict server all-targets/all-features Clippy passed. Scope regression checks changed
fingerprints, foreign tasks, stale grants, zero inference on preview and recovery of
the authorization/admission gap. Storage regression checks consent-preserving reopen,
duplicate admission and changed-context rejection. These tests are not a successful
conversation HTTP vision golden path; that and the frontend sample/terminal canvas
integration remain required. Existing Journey browser regression is being rerun in
its isolated workspace; no paid/real workspace invocation was authorized or performed.

Journey regression completed: **1/1**, 29.1s test / 46.3s total, fresh
`/tmp/annotagent-guided-e2e-87121`. Existing actual HTTP fixture sampling, duplicate
start, changed-scope rejection, cancellation and lost-response behavior passed. This
is the existing Journey path, not evidence that the new conversation consent is wired
to the UI. Production build passed with its existing >500 kB chunk warning.

### First conversation → authorized classification sample → editable canvas (M1/M2)

Connected the task-scoped sample authorization/history API to the saved Builder card.
The card shows the exact Draft revision, actual models/destinations, image/call limits,
cumulative spend and unknown cost, requires consent, restores operation history and
offers Stop while active. A lost POST response reads the existing receipt. Pending
non-secret request envelopes survive tab reload in sessionStorage for explicit retry;
mount/GET never replays them. A saved report is not described as accurate or accepted.

The right pane now reuses SampleFeedbackEditor/AnnotationCanvas for the selected saved
test. Typed `/projects/:id/work?conversation=&draft=&test=&image=` navigation keeps the
same task and page focus identity. Project/Draft/Test ownership and image hash are
checked; missing, changed or legacy evidence cannot substitute another final box.
Only terminal projections feed the editor, with lineage duplicates removed. Original
pixels, classification/geometry editing, feedback save, dirty guard and image switching
reuse existing controls. Sample edits remain Sandbox feedback. Saved sample scope now
also freezes the bound semantic Schema; additions use those labels/kind, not a newer
unrelated Project goal. Model-call previews and source errors remain truthful.

The new real-HTTP fixture path found two backend defects: conversation classification
seeded a legacy task-provider node, and Registry binding normalization updated only
compiled nodes while leaving their authored PipelineStep unbound. Classification now
uses the existing controlled classification composition, keeps every declared category
and binds actual Registry profiles. Normalization synchronizes the authoring projection
without replacing the frozen Profile identity; an idempotency regression covers this.
Bounding-box conversation synthesis still needs its own complete golden path.

Evidence: new browser test runs goal→Schema→Builder→explicit sample consent→HTTP
classification→durable call receipt→canvas. It drops the successful sample POST response,
reloads with no POST/model work, edits 室内 to 室外, verifies leave-with-unsaved confirmation,
saves and restores the correction with an unchanged model ledger. With journal regression:
**2/2** in `/tmp/annotagent-guided-e2e-88620`; with Schema/Builder regression previously
**2/2** in `/tmp/annotagent-guided-e2e-88430`. Web typecheck and **106/106** unit tests pass.
Application **93 passed/1 billable ignored** plus the added binding normalization test
passed; Server **36/36** passed. Strict Application/server Clippy passed before final
visual-only sizing adjustment. Screenshots `sample-classification.png` and
`sample-classification-390.png` are TEST synthetic images and scripted HTTP model
outputs, not Live quality or real-user usability evidence.

Remaining: bbox/crop golden path, Human Request answer/outbox/resume, task intent routing
beyond the first message, controlled rerun/improvement, project creation/no-LLM flow,
same-workspace processing/Review/export, missing-model return and broad accessibility/
responsive/200%/race audit. Default rollout is still not enabled. No push, remote change,
real workspace restart/data change, old credential use or paid inference occurred.

Final increment recheck: Application **94 passed/1 billable ignored**, strict
Application/server Clippy passed after fixing the new test's clone-on-Copy lint.
Latest classification browser **1/1**, 3.3s / 8.4s total in
`/tmp/annotagent-guided-e2e-88838`, regenerated the two viewport screenshots after
canvas sizing adjustment. No full-workspace Rust or real-user usability claim is made.

### Conversation bbox sample and shared label detection (M1/M2 increment)

Conversation bounding-box seeds now use the same existing controlled Skill/Core
composition as classification, rather than the legacy task-provider seed. All Schema
labels survive synthesis. Per-label routes reference one normalized shared detection
configuration; goal/exclusion rules remain in supported target-description fields.
Each route retains its mandatory geometry Human Review node. Registry profile binding,
Builder planning, authorization, sample runtime and terminal projection are reused.

The three-label regression verifies one shared detector/model node and three distinct
label routes with required review gates. Browser tests now cover both classification
and bbox: actual HTTP TEST fixture inference, explicit consent, dropped successful POST
response recovery, GET-only reload, sandbox edits, dirty-guard preservation and restored
geometry. Bbox asserts exactly one inference receipt, zero ready terminal candidates
and one review candidate; it does not count intermediate geometry as another result.
The width assertion uses numeric tolerance because Rust f32 normalization produces
96.0000038 SVG pixels for a requested 96-pixel width, not an exact string `96`.

Evidence: Application all-features **95 passed / 1 billable ignored**, strict
Application all-targets/all-features Clippy passed. Browser **2/2**, 11.7s total in
`/tmp/annotagent-guided-e2e-89683`; production build passed with the existing >500 kB
chunk warning. Inspected `sample-bbox.png` and `sample-bbox-390.png`: desktop split
canvas and narrow-screen image view preserve editable geometry. These screenshots
deliberately use the existing synthetic scene and scripted `cup` output, **not a real
cup detector, Live quality validation, or real-user usability evidence**.

Human Request/outbox/resume, subsequent-message intent routing, improvement loops and
conversation processing/Review/export remain unfinished. Default rollout stays off.
No real workspace mutation/restart, paid calls, push or remote changes occurred.

Final increment checks: `cargo fmt --all --check`, Web typecheck and **106/106** Web
unit tests passed. Unrelated previously regenerated screenshots remain unstaged.

### Durable human correction and resume transaction (M2 foundation)

Added migration 31 and task-owned Human Request storage. This increment supports a
frozen existing Sample Test outcome (not an arbitrary formal annotation): sample/image
hash, outcome, expected feedback sequence, reason/question and stable resume checkpoint.
The saved sample must be linked to the exact task/conversation's Sample Operation.
Pending, Answered, Applied, Cancelled and Stale states are explicit. Duplicate request
IDs and answers are checked against their original content.

The existing `save_sample_feedback` validation and optimistic sequence boundary now
accept an internal transaction hook. Human answers use it to atomically persist the
real Sandbox feedback revision, answer state and resume outbox event. Outbox delivery
is at-least-once, with stable checkpoint/revision identity and explicit acknowledgment;
it is not new execution authorization. Ordinary feedback writes retain their previous
API. Pending human requests block new admissions to the task's shared model-call ledger
without consuming a call or preventing reads of existing receipts. Cancelled/stale
requests cannot accept late answers.

Evidence: Storage all-features **51 unit + 16 integration tests passed**, strict Storage
all-targets/all-features Clippy passed; Application conversation tests **10/10** passed.
New tests cover duplicate submission/delivery, restart, foreign owner, changed image
hash, stale feedback, incompatible correction type, malformed geometry deserialization,
late answers and no new spending while pending. A SQLite trigger deliberately fails
outbox insertion and proves feedback and request state roll back together; retry after
recovery writes exactly one revision. Existing Sample Test predictions remain unchanged.

Not yet a complete Human Request feature: no HTTP/UI admission or answer route,
terminal-projection/current-image validation at Application boundary, coordinator
delivery consumer, actual repair continuation or browser evidence in this increment.
ClarifyTask/IdentifyTarget/CompareCandidates and formal Review linkage remain pending.
The new storage API is not exposed to model tools. No real workspace was migrated or
restarted, no model was called, no push or remote changes occurred.

### Human correction Application ownership and pixel validation (M2 increment)

Added Application commands to list/create/answer the stored correction requests using
canonical Project identity. New requests and unsaved answers resolve the actual current
Project image and compare its SHA-256 with the immutable Sample Test input. Only final
or review terminal projections may be targeted; coarse/intermediate outcomes and legacy
empty projections are rejected. Storage still checks the exact Sample Operation/task
association and feedback sequence. A completed answer's identical retry restores its
original receipt even after the image changes; it does not silently edit new pixels.
No command invokes a model, publishes or writes formal annotations.

Evidence: two new Application tests cover foreign Project/Task, missing terminal
projection, intermediate candidate rejection and changed pixels. The integration test
creates a temporary Project, imports a TEST copy through the existing controlled import,
saves a sample and task request, changes the actual image file, verifies answer rejection
with zero feedback, restores the pixels, saves the answer and verifies idempotent recovery
after another file change. Its model-call history remains empty. Initial test attempts
incorrectly imported directly from outside the workspace; the existing security check
correctly rejected them, so the test now stages its copy inside the isolated workspace.

HTTP routes, Human Request cards, submit-and-continue and the durable coordinator consumer
are still pending; this is not browser or end-to-end human continuation evidence.

Increment validation: Application all-features **97 passed / 1 billable ignored**,
strict all-targets/all-features Clippy and workspace rustfmt check passed. No real
workspace changes, paid inference, push or remote changes.

### Human correction HTTP boundary and transport integration (M2 increment)

Registered task-scoped `/human-requests` GET/POST and `/:request/answer` and
`/:request/cancel` POST under the existing Project/conversation/task route. Body task
identity must match the route; Application resolves ownership and current sample
evidence. Cancellation is idempotent and cannot discard an already answered request.
The browser cannot acknowledge outbox delivery, mark an answer Applied, supply spending
authority or dispatch a coordinator/model through these endpoints. Existing same-origin
mutation middleware applies unchanged. GET restores state without model work.

Extended both classification/bbox browser fixtures with real HTTP Human Request checks
against the sample each test just produced: mismatched task rejected, foreign Origin
403, idempotent request and answer, conflicting retry rejected, exactly one additional
feedback revision, page reload and GET restoration, cancellation and late-answer
rejection. The model-call ledger remains identical before/after all human actions.
These are API calls from Playwright, **not yet clicks on a Human Request card**.

Evidence: both browser tests passed in fresh `/tmp/annotagent-guided-e2e-91026`,
**2/2**, 26.1s total, using explicitly scripted local HTTP models. Web typecheck and
**106/106** unit tests pass; strict Server all-targets/all-features Clippy passes.
Production build retains the known >500 kB chunk warning. No Human Request UI screenshot
is claimed; screenshots regenerated by the existing sample tests are left unstaged.
UI/typed request navigation, coordinator consumer and repair continuation remain next.
No live model test, real workspace change, push or remote modification occurred.

Final increment regression: Server all-features **36/36** passed after cancellation
was added; workspace rustfmt check passed.

### Human Request cards and bound canvas submission (M2 increment)

The conversation now reads saved task requests, shows their actual state/question,
opens their exact sample image and supports explicit cancellation. Typed work routes
retain `task` and `request` alongside Draft/Test/Image; missing or mismatched requests
cannot silently select another result. Reading/refreshing requests sends no mutation.
Explicit refresh currently discovers new requests; automatic coordinator notification
is still pending. Refresh is blocked while a correction is dirty; cancelling that
active request requires discarding edits explicitly.

SampleFeedbackEditor has a scoped human-submission adapter, not another annotation
editor. It preselects and locks the requested outcome, restores that object's prior
feedback, keeps normalized geometry/classification editing and Undo, and submits the
same validated feedback shape through the atomic answer command. Missing-target and
whole-image actions are not misapplied to a single-object request. The existing editor
remains unchanged outside this mode. The button says **Submit correction**, not
Submit-and-continue: there is still no durable coordinator consumer. Answered cards
truthfully say correction saved / continuation pending, and ordinary Sandbox edits
remain distinct from formal annotation acceptance.

Browser tests now click Refresh requests → Open requested result → reload exact route
→ edit the requested label/bbox → Submit correction. The test drops the successful
answer response; local edits remain, explicit retry reuses the revision key, and only
one feedback revision appears. Both classification and bbox complete, and their task
call histories remain unchanged. Existing mismatched-task, cross-origin, conflicting
retry and late-after-cancel HTTP checks remain. These requests are created explicitly
by the test, not autonomous LLM-request-generation evidence.

Inspected desktop and 390px screenshots `human-request-{classification,bbox}.png` and
`human-request-{classification,bbox}-390.png`. They show scripted TEST outputs on the
existing synthetic scene, not real cup-detection quality. Narrow-screen panels and
submit controls work; no 200% or real-user usability claim is made. Last focused run
was **2/2** in `/tmp/annotagent-guided-e2e-91904`, 13.1s total; Web unit tests **107/107**
and typecheck passed. Production build retains the known chunk-size warning.

Remaining: automatic Human Request generation/notification, answer consumer and repair
resume, other request kinds, multi-message routing, bounded improvement and full
conversation processing/Review/export. Default rollout stays off. No real workspace
restart, paid call, push or remote change occurred.

Final combined browser run `/tmp/annotagent-guided-e2e-92062`: **3/4 passed** (both
sample/request tests plus Schema/Builder). The journal case hit the real shared-server
mutation-rate window; its route handler was waiting on `mutation_rate_limited` while
the UI assertion expired after 10s. Fresh isolated journal rerun in
`/tmp/annotagent-guided-e2e-92142`: **1/1 passed**, 836ms / 5.9s total. Production rate
limits were not changed. This does not claim a fully green combined suite; test-server
isolation/rate-window coordination remains a harness limitation. Final Web checks:
typecheck, **107/107** unit tests and `git diff --check` passed.

### Deterministic correction resume service and exact feedback scope (M2 increment)

Added an Application continuation command for an Answered/Applied Human Request. It
uses the saved checkpoint UUID as the existing sample-plan copy's idempotency key,
prepares an editable Workflow Draft and only then acknowledges the durable resume
event. The original Draft and Sample Test remain unchanged. This stage **prepares
repair context**, not an improved/tested plan; it does not call Builder, a vision
model, Publish or Run. HTTP automatic delivery and the UI link to the resulting Draft
are still pending, so existing pages continue to state that continuation is pending.

The existing copy service collected every saved correction from the Sample Test.
Added a scoped variant selecting the exact answer revision so later/unrelated edits
cannot silently widen this request's evidence. Existing general copy callers retain
their behavior. A retry using the same copy ID but a different answer revision fails.
The actual existing sample-plan evidence store remains the sole repair-context source.

Application tests now simulate changed current pixels (copy refused, event retained),
then restore pixels, add a later correction, create the copy, close/reopen the real
temporary Application and resume the still-undelivered event. The same copy is returned,
exactly the original answer is included, another delivery is idempotent, the original
Draft is byte-equivalent as a deserialized object and model-call history remains empty.
This verifies the copy/ack crash gap; it does not prove automatic daemon delivery or
LLM repair. Targeted correction tests **2/2**, Storage all-features **51 unit + 16
integration** passed; strict Application/Storage Clippy passed. No real workspace
restart/mutation, paid call, push or remote change occurred.

Final resume-service regression: Application all-features **97 passed / 1 billable
ignored**; workspace rustfmt and diff checks passed.

### Automatic local continuation, durable failures and explicit retry (M2 increment)

Migration 32 records the prepared Draft ID or bounded failure reason. Successful
outbox acknowledgment and result reference now commit in the same SQLite transaction;
the result must carry the checkpoint's exact feedback revision. A late competing
delivery failure cannot replace success. Request GET includes these saved fields.

The human-answer POST first saves the correction, then invokes the existing local
resume service. Startup delivers previously interrupted, unanswered-delivery outbox
items once; attempts with saved failure reasons require an explicit task-scoped
`/:request/resume` POST, not repeated background retries. This performs only the
authorized local copy/context preparation—no model, Publish or Run. Startup failure
records retain the stable owner even when the Project is no longer available.

UI explains the local Draft preparation before submission and reports either the
prepared revision or a saved correction plus failure reason and retry action. It does
not claim the copied plan was rebuilt, tested or improved. An explicit advanced
inspection button targets the existing Pipeline page; same-workspace Builder repair
remains pending. GET/refresh never performs continuation.

Evidence: Application test verifies automatic startup completion of the copy/ack crash
gap, persistent failure after pixels change, no repeat attempt across restart, and
successful explicit retry after restoring pixels. Storage rejects acknowledgment
against unrelated Draft evidence with full transaction rollback. Both HTTP/browser
paths now assert Applied + exact Draft ID, one frozen feedback item, repeat resume
idempotency and unchanged model ledger. Latest isolated run **2/2** in
`/tmp/annotagent-guided-e2e-93186`, 38.3s total. Application all-features **97 passed /
1 billable ignored**, Storage **51 unit + 16 integration**, Server **36/36**, Web
**107/107** and typecheck passed. Production retains its chunk-size warning.

No Live quality, real-user usability or complete LLM repair claim is made. Human
request generation, automatic UI notification, request kinds beyond existing sample
correction, bounded model repair, processing/Review/export and remaining M4 checks
are unfinished. No real workspace restart/mutation, paid calls, push or remote change.

### Repair evidence uses feedback-scoped terminal results (M3 prerequisite)

Code inspection found that the existing sample-plan Repair Builder supplied raw
`sample.outcomes` from every baseline image. Those can contain intermediate boxes,
and unrelated images could enter a local correction's analysis. Replaced that
observation assembly with the existing terminal projection, filtered by the saved
feedback's image and outcome identities. Missing-target feedback includes that
image's terminal subjects; intermediate/legacy IDs are explicitly reported absent,
never substituted with a coarse detection. Duplicate terminal IDs are collapsed.

The bounded payload retains bbox/classification values, lineage, semantic confidence
and separate geometry/status evidence. It explicitly says no image pixels were
supplied; unsupported geometry values are marked omitted. Candidate truncation is
reported with counts, no-target remains explicit, and mismatched input/result counts
or foreign feedback are rejected rather than silently paired. This changes the actual
existing Repair prompt assembly, not only a test helper. It does not yet connect a
conversation repair authorization/command or claim that the model improved quality.

Regression: Application all-features **98 passed / 1 billable ignored**, strict
Application Clippy passed. Added terminal-vs-intermediate, unrelated image/object,
classification, deduplication, whole-image feedback, foreign scope, malformed report,
candidate-limit and no-target assertions. Workspace formatting and diff checks pass.
No UI changed, so no new screenshot claim. Existing generated screenshot changes are
left unstaged. No real workspace change, paid call, push or remote modification.

### Authorized preserved-plan repair command (M3 increment)

Conversation Builder now accepts an optional exact repair reference: Human Request,
prepared Draft ID, revision and content hash. The Application resolves only Applied
requests from the owned task, verifies the copy/checkpoint identity and exact saved
answer evidence, rejects published/archived plans, and checks revision/hash again
when loading. Its existing Builder loop starts in `RepairDraft`, preserving the
authored plan and frozen Schema rather than regenerating a FromScratch seed. Model
calls still use the cumulative task ledger; dry runs remain disabled in this command.
The existing repair management lease now uses the actual input revision instead of
hard-coded revision 1. History resolves Schema from the session's working Draft,
whose identity legitimately differs from the operation UUID during repair.

The existing preview/launch HTTP endpoints accept `repair_request_id`; preview seals
the resulting exact repair binding into scope and describes saved feedback/terminal
metadata as outgoing text. Launch requires that returned `repair` binding in consent,
checks it against current state, and retains the normal idempotent operation receipt.
No new executor or automatic authorization is introduced. Existing UI is unchanged
this increment: its in-conversation repair button and browser verification are next.

Extended the Application's scripted Provider regression through a saved Sample Test,
scoped answer/copy, manual edit to revision >1, authorized actual Repair Builder loop,
and duplicate retry. Assertions verify the original Draft is unchanged, the session
uses the prepared copy, the request contains scoped terminal feedback, only two
additional text calls occur, and stale/foreign/pending references fail before calls.
This verifies execution semantics, not a measured improvement or real vision quality.
Application **98 passed / 1 billable ignored**; Server **36/36** existing regressions
passed; strict Application/Server Clippy and formatting/diff checks passed. New HTTP
repair flow still needs dedicated browser coverage. No Live calls, real workspace
mutation/restart, screenshot change for this increment, push or remote change.

### Conversation repair card and separately authorized comparison sample (M3 increment)

An opened Applied human request now restores its frozen Sample Schema into the existing
Builder card, scoped to the prepared repair Draft. It previews the actual repair text
data destination/allowance, submits the sealed repair binding, and shows the saved
Builder outcome plus the existing sample authorization card. Initial-build history
and repair-copy history are selected separately. Reads/mount do not start inference;
dirty sample edits disable repair. Requests and the original sample remain accessible.
Preparation-status wording is historical: it no longer falsely says that no later
model repair/test occurred after the local copy was created.

Browser evidence uses the actual Rust server and explicitly TEST HTTP transport in
`/tmp/annotagent-guided-e2e-95181`: classification and bbox **2/2**, 17.6s. Each covers
goal/Schema/Builder/sample, editable correction with lost-response retry, local resume,
explicit repair consent, <=8 additional text calls, exact copy and RepairDraft mode,
duplicate repair POST without extra calls, and reload restoration. Each then separately
authorizes a new sample of the repaired copy, opens its new Sample Test ID in the same
workspace, and reloads without changing call history. No quality improvement is claimed.
The first attempted E2E exposed a wrong test listener URL (not a product failure); it
was corrected to the existing builder-operations endpoint before the passing runs.

Screenshots inspected: `conversational-workspace/repair-bbox.png` and
`conversational-workspace/repair-classification.png`. Synthetic source pixels and TEST
labels are intentionally not real semantic accuracy evidence. Web typecheck and
**107/107** unit tests passed; production build passed with its existing >500kB chunk
warning. No Rust change in this increment. No live Provider, real workspace mutation,
push or remote change; no real-user usability test.

Remaining: automatic request generation/notification, broader request kinds, early
operation-reservation restoration races, concise conversation layout, origin context
when opening a comparison sample, full processing/Review/export and M4 verification.
The workspace remains opt-in; this milestone is not the complete default experience.

### Comparison sample origin and exact correction return (M3/M4 increment)

The existing work route now retains task/request IDs while opening a related repaired
sample and switching its images. A small typed relation helper distinguishes the
original requested subject, another baseline image, the prepared copy's comparison
sample, and unrelated context. Only the original subject receives the Human Request
submission context; a new test cannot accidentally reuse the old answer target.
Related sample views offer “Return to original correction,” preserving the original
test/image/request. Unrelated request/Draft pairs show an explicit unavailable-context
message instead of silently replacing the target.

The shared feedback editor accepts an initial outcome reference. Returning to a
completed request restores that subject and its relevant saved feedback, even if a
later whole-image note exists. This only selects the initial object; ordinary canvas
selection remains available after the request has been answered. Existing dirty guards
and saved Sandbox feedback remain authoritative; no new annotation store or mutation
on GET was introduced.

Evidence: Web **108/108**, typecheck and production build passed (same chunk warning).
Latest isolated browser suite `/tmp/annotagent-guided-e2e-95772`: classification and
bbox **2/2**, 17.8s. Both verify comparison URL retains task/request, reload restores
its origin, returning selects the original image/outcome despite a later unrelated
note, browser Back restores the comparison, and no model calls are added by navigation.
Inspected screenshots: `conversational-workspace/comparison-origin-bbox.png` and
`conversational-workspace/comparison-origin-classification.png`. These are TEST transport
and synthetic pixels, not Live quality or real-user usability evidence. No real
workspace change, push or remote modification. Automatic request generation, broader
conversation intent, formal processing/Review/export, and remaining M4 work continue.

### Evidence-driven human-request preparation command (M3 increment)

Added an Application command that derives human requests from an owned conversation
Sample Operation and saved Sandbox report. It selects only terminal `needs_review`
bbox/classification values, never raw intermediate outcomes, missing geometry or
ready-to-accept results. No-target and unsupported geometry remain explicit gaps;
this command does not invent a final object or a fake editable request.

Preparation is bounded to one request per image (up to ten images). This avoids
multiple pending answers competing for the same image's optimistic feedback sequence.
Request and resume checkpoint identities are deterministic, existing requests are
restored rather than recreated, and current pixel hashes/ownership are checked by
the existing creation service. The request explains that human review is needed,
not that the model has been proved inaccurate; no inference or formal acceptance
occurs. Its expected sequence comes from the saved feedback history.

The existing real temporary-workspace correction/resume test now begins with this
command, asserts duplicate preparation and foreign-task rejection, then exercises the
same answer/outbox/restart flow. A new selection regression covers bbox, classification,
ready results, absent values and no-target/intermediate-only reports. Application
**99 passed / 1 billable ignored**, strict Application Clippy and diff checks passed.
No UI/server completion hook is enabled yet: durable completion delivery/restart
recovery must be connected next, without backfilling historical user samples on GET.
No screenshot or automatic browser-request claim for this backend prerequisite.
No Live model, real workspace mutation, push or remote change.

### Durable sample-completion assistance delivery (M3 increment)

Migration 33 adds a local assistance queue keyed by the existing Sample Operation.
Only a new operation whose conversation consent contains `human_review: true` queues
work, in the same reservation transaction. No backfill is performed. Duplicate
reservation retains the same queue item and receipt. Delivery requires both a saved
Sample Test and successful operation; cancellation/unfinished reports do not trigger
new requests.

The existing sample worker invokes local assistance delivery after settling its
operation. Application startup performs the same delivery after ordinary sample
recovery, closing the report/notification crash gap without repeating inference.
The deterministic preparation command supports partial-delivery replay. Completed or
failed deliveries leave the pending set; bounded errors persist separately from the
successful sample report, and the scoped Sample Operation GET includes `assistance`.
It does not claim that a report failed merely because human-request creation failed.
Failed delivery currently requires a future explicit retry control, not a restart loop.

Regression: Application correction test now opts in, simulates stopping after saved
report/operation completion, reopens the real isolated Application, verifies generated
request/completed queue state, then continues the existing correction/outbox tests.
Storage injects queue-insert failure and verifies reservation rollback, confirms old
operations remain unqueued, and verifies failed delivery survives reopening without
looping. Storage all-features (**52 unit plus existing integrations**), Application
**99 passed / 1 billable ignored**, Server **36/36**, and strict Clippy passed.

Frontend opt-in and automatic notification are not enabled in this increment; current
browser sample requests omit the new flag. This deliberately avoids silently changing
existing saved request bodies or real user history. Next: wire the flag and visible
delivery outcomes into the conversation's sample card, then test automatic request
creation/answer in both browser paths. No real server restart/mutation, Live calls,
push or remote change. No new UI screenshot claim.

### Automatic assistance surfaced in the conversation (M3 increment)

New sample consents explicitly opt into local human-review request preparation. Existing
saved consent bodies remain unchanged. Sample cards poll only pending execution/delivery,
restore the saved outcome, and refresh the owned request list on completed delivery without
automatically changing the selected image. Delivery failure is separate from report failure.
Opening a request expands its relevant correction editor; answering remains Sandbox-only.

Browser regression: `conversation-samples.spec.ts` **2/2 passed** in isolated TEST workspace
`/tmp/annotagent-guided-e2e-97797`. Bbox completion produces a visible automatic request,
opens its exact result, saves a correction and restores it after reload without extra model
calls. Ready classification results correctly produce no request. Web typecheck, **108/108**
unit tests, production build, fixture strict Clippy, Rust formatting and diff checks passed.
Inspected `conversational-workspace/automatic-human-request.png`: correction fields are
visible; the tall editor/sticky composer still needs M4 layout refinement. This screenshot
uses synthetic TEST pixels, not Live quality evidence or a usability claim.

An additional low-confidence classification experiment exposed a separate real defect:
in `/tmp/annotagent-guided-e2e-97049`, confidence 0.4 selected `review`, but the controlled
classification plan connected the gate directly to Commit, which correctly rejected an
unvalidated artifact (`unsafe_commit_input`). There was no terminal candidate to review.
The fixture now supports an explicit `-review` model scenario for reproducing this. It is
NOT fixed by the UI change; next work must introduce conditional review routing while
preserving the direct high-confidence path and Commit safety. No real workspace changes,
Live calls, push or remote modification; broader goal and M3/M4 requirements remain open.

### Conditional classification review and branch joins (M3 increment)

Fixed the controlled classification composition: Classifier → Confidence Gate sends `pass`
directly toward Commit and `review` into Human Review. Both converge at the same required
Commit input. Low-confidence classifications no longer fail unsafe Commit or disappear as
no-target; high-confidence classifications are not forced into Review. Runtime approval and
Commit validation are unchanged. Existing saved/PUBLISHED drafts are not migrated or mutated.

Core authoring now preserves generic `routed_step` and `any_of_steps` sources through JSON,
compile/save and static validation. A branch join compiles alternative edges into one required,
multi-source port, using the existing DAG readiness/routing semantics. Empty joins, mixed
types, nonlocal/nested join members, unknown outputs and malformed route names block validation.
No specific model or Label branch was added to Core. The advanced editor retains and describes
these inputs. The iterative Advisor's intentional invalid-Draft exercise now disconnects and
restores all Commit inputs, because removing only one alternative no longer makes a join invalid.

Regression-driven iterations caught and corrected an initially incompatible two-Commit plan
and an incorrect `accept` route name (the existing confidence gate emits `pass`). A subsequent
TEST scenario initially selected an earlier enabled compatible model; browser setup now disables
only previous models bearing this test's explicit display-name prefix and verifies the actual
model in the sample trace. No real Registry is changed by these isolated tests.

Final checks: Core **117/117**, Application **99 passed / 1 billable ignored**, Runtime **33 unit
+ 15 integration**, Web **108/108**, typecheck, production build, strict Core/Application Clippy,
format and diff checks passed. Browser **3/3 passed**, isolated
`/tmp/annotagent-guided-e2e-99100`: high classification, bbox, and explicit confidence-0.4
classification, including authorization, lost-response recovery, automatic assistance,
correction, plan repair and retest. The low-confidence saved trace shows the exact TEST model,
gate route `review` and `confidence_review: awaitingreview`, not an unsafe Commit failure.
Inspected `conversational-workspace/sample-classification-review.png`; synthetic pixels and
scripted labels prove transport/UI behavior only, not quality or真人可用性. Known chunk warning
and tall conversation editor remain. No push, remote modification, real workspace mutation,
Live inference or new formal annotation. Formal processing integration and other M3/M4 work
remain unfinished; the goal is still active.

### Formal processing evidence linked to conversation tasks (M4 increment)

Reused the existing `processing-preview` / `processing-operations` publication-and-Batch
service. Its preview now derives conversation/task/message identity from the saved Sample
Operation and resolves the exact owned Schema revision frozen in the tested Workflow Draft.
It rejects missing conversation provenance, wrong Project/Draft/task and altered Schema
snapshots; later Schema revisions do not reinterpret this revision. Legacy non-conversation
plans retain their existing path. The actual Schema goal replaces the empty Project goal in
this confirmation summary, and the complete context participates in the authorization hash.

Existing processing receipts persist that context. A read-only, task-owned history endpoint
filters those receipts (bounded to 100); it creates no duplicate execution entity, does not
backfill old history, and cannot infer owner from the active UI. The conversation displays
saved processing cards and a link to the actual Project Batch, without claiming that the
historical `started` receipt is current completion status or moving the selected image.

Regression covers a persisted Schema revision 1 after revision 2 exists, missing sample
command, altered snapshot, foreign owner, and legacy fallback. Storage verifies exact
Project/conversation/task filtering after reopening, including a published/start-failed
receipt. Browser scenarios additionally invoke the real existing confirmation API on ONE
isolated TEST image, retry the same key, assert one linked Batch, restore its card, open
canonical processing results and Back to the exact sample URL. High classification reaches
`completed`; bbox and confidence-0.4 classification reach `awaiting_review`. An initial test
incorrectly expected `completed` for all three; it was corrected to require those distinct
states, not by accepting or hiding pending reviews.

Checks: Application **99 passed / 1 billable ignored**, Server **36/36**, targeted Storage
processing tests **3/3**, strict Storage/Application/Server Clippy, Web typecheck and **108/108**
unit tests passed. Production build retains the known chunk warning. Browser **3/3 passed**
in `/tmp/annotagent-guided-e2e-221`; final screenshot recapture disables transitions (the first
result captures caught the existing route fade-in and were not suitable final evidence).
Final recapture: **3/3 passed** in `/tmp/annotagent-guided-e2e-312`. Inspected
`processing-linked-bbox.png`, `processing-results-bbox.png` and
`processing-results-classification.png` under `conversational-workspace/`; results distinguish
Review from completed classification. This uses scripted HTTP TEST
responses and synthetic pixels, not real model quality, billable validation or真人可用性.

Still incomplete: initiating formal processing from the conversation itself, formal-call
allocation/accounting within the shared task budget, current Batch status in the conversation,
embedded formal result/Review/export integration, and broader M4/default-experience work.
The UI explicitly states that starting processing there is not connected; no fake disabled
confirmation control was added. This increment proves the existing service can execute the
conversation's frozen plan and be recovered from its task, not the entire requested journey.
No real workspace mutation/restart, push or remote change. Goal remains active.

### Conversation confirmation and cumulative task accounting (M4 increment)

The saved-sample workspace now opens the existing `JourneyConfirm` component in the
conversation column. It shows the tested Schema/plan revision, image range, model destination,
unknown price, prior task usage, additional bounded allocation and Sandbox-feedback warning.
The explicit checkbox/button invokes the existing publication/Batch service. Success stays
on the same image and receipt; opening processing results remains a separate visible action.
The `processing` query carries preview/operation identity through refresh and Back. The
standalone confirmation route still uses the same component and existing navigation.

Accounting reuses—not replaces—the existing cumulative planning/sample grant and atomic
Batch allowances. An owned task ledger view sums planning reservations and all explicitly
confirmed processing allocations/reservations, including failed/in-doubt planning requests.
Each phase keeps its enforced sub-cap; new processing consent adds a bounded allocation,
never resets prior spend or grants each nested call an independent budget. This is call-count
accounting, not a monetary estimate or a new Project-wide lifetime spending limit. The latter
must not be inferred from this task-level view. Ledger scope participates in confirmation
fingerprinting, so changes before first admission require renewed scope review.

The Batch allowance is created with the Batch transaction. Ledger reads join by its durable
operation ID, even if a crash prevented saving the final started receipt. Confirmation retries
first recover an already-created Batch after validating the original request key/body, without
reauthorizing against a now-changed usage snapshot, restarting it or publishing again. A new
server regression deliberately leaves a pending Batch and a published receipt with missing
current Draft data: retry restores its receipt and leaves the Batch pending. It also checks
conflicting bodies and preserved exhausted allowance. Conversation processing freezes Provider
transport retries to zero; Runtime retry attempts and local plugins still pass through the
existing allowance. Legacy non-conversation processing behavior is unchanged.

UI checks caught an oversized inherited confirmation heading and composer overlap; embedded
heading/spacing were reduced and the composer is temporarily hidden while confirmation is
active (unsent text state is retained; Back returns to the sample conversation). The mobile
confirmation remains scrollable with visible scope/checkbox/actions, no horizontal overflow.
Long historical card stacks still need the broader conversation-density refinement.

Final evidence: `conversation-samples.spec.ts` **3/3 passed** in
`/tmp/annotagent-guided-e2e-1951`, now using the actual UI confirmation for bbox, normal
classification and review classification. It loses the POST response intentionally, reloads
the same receipt, retries without a second Batch, verifies exactly one added processing call
and preserved planning spend, checks frozen transport retries = 0, then opens results and
returns to the exact workspace URL. `journey-ready.spec.ts` **1/1 passed** for the pre-existing
standalone flow. Storage **53 unit + 16 integrations**, Application **99 passed / 1 billable
ignored**, Server **37/37**, strict relevant Clippy, Web typecheck, **108/108** unit tests,
production build, formatting and diff checks passed (known production chunk warning remains).
Inspected `processing-confirm-bbox.png` and `processing-confirm-390.png` in
`conversational-workspace/`. TEST transport/synthetic image evidence is not Live quality or
真人可用性 evidence; neither was executed. No push, remote change or real workspace mutation.

Next: bring live Batch status/control and formal Review/export back into the conversation,
finish broader intent/reference/setup handling, and complete default-entry and M4 audits.
This closes the explicit start action, not the entire Conversational Workspace objective.

### M3 continuation — current Batch status in the conversation (2026-09-08)

Conversation processing receipts now display the owned Batch's current persisted status,
separate completed/review/failed/cancelled counts and direct supported controls. The current
confirmation shows this immediately after start, while older tasks show it in saved history;
the same Batch is not mounted twice. Reads use the existing query cache and non-overlapping
polling for pending/running/paused/awaiting-review tasks. Unmount stops observation, not the
Batch. Ownership mismatch does not render another Project's progress or controls. Read errors
remain visible with explicit reload; no GET, mount or refresh invokes an execution action.

Extracted `BatchControls` from the existing results page for both presentations. Actions use
the original coordinator endpoint and a synchronous click lock, reload authoritative status
after success, and preserve visible errors after failure. Supported actions are unchanged:
running = pause/cancel; paused/pending = resume/cancel. Review and terminal states do not expose
misleading restart controls. Review users still follow the canonical results link; embedding
formal Review/export in the conversation remains unfinished. Leaving the task and cancelling
are explicitly distinguished, including the limit that sent remote requests cannot be undone.

Validation: Web typecheck, **110/110 unit tests**, production build, Rust formatting and diff
checks passed. Combined isolated browser run in `/tmp/annotagent-guided-e2e-2720` passed
**4/4** (three conversation scenarios plus the existing standalone ready journey). Actual TEST
HTTP model workflows verify completed vs awaiting-review states and retained image/test URL.
An additional explicitly intercepted browser harness verifies pause failure, pause, resume,
cancel and refresh without unsolicited control POSTs; it does **not** claim real executor
pause/resume coverage and sends no control mutations to the completed fixture Batch.

Screenshot review caught adjacent unspaced status paragraphs; added readable spacing and
rendered state names without underscores. Final screenshot: `processing-current-status.png`.
Final styled conversation rerun passed **3/3** in `/tmp/annotagent-guided-e2e-2865`.
No Live inference, real Workspace changes, push or remote changes. Full-workspace Rust
regressions, real control lifecycle browser evidence, default entry, broader conversation
intent/reference handling, formal Review/export embedding and M4 usability audits remain.

### M3 continuation — formal results on the conversation canvas (2026-09-08)

`Open processing results` now replaces the workspace's right-hand sample canvas with the
existing `JourneyBatch` terminal results presentation. It does not navigate to global history
or create another result engine. The same result projection, original/results switch, status
filters, annotation list, geometry-risk notice and supported controls are retained. An explicit
banner distinguishes formal dataset candidates from Sandbox sample corrections. Actual Review
and Export remain the canonical existing pages, one action away; the editor is not yet embedded
and full formal acceptance/export round-trip is still required.

The typed conversation route carries separate Batch result context (Batch, result image, filter,
selected candidate, original/results view), preserving original sample image, Draft, Test, task
and request keys. Returning to the sample removes only result context. Switching result views
does not steal page heading focus. The wrapper requires the Batch to be linked to this owned
conversation's processing receipts, then checks server Project identity; invalid links show an
explicit unavailable message, not another canvas. Formal missing-annotation editor guards compose
with the conversation dirty guard. Messages cannot silently refer to the sample while formal
results are displayed: without an explicitly selected formal image they are Project-level.

The confirmation/history observer yields while its Batch is on the canvas, avoiding duplicate
controls/pollers. Existing query cache, result services and coordinator remain authoritative.
No model call, publish, control mutation or acceptance is caused by viewing/reloading results.

Tests: Web typecheck, **112/112 unit tests**, production build and diff/format checks passed.
Final isolated TEST browser run `/tmp/annotagent-guided-e2e-3353`: **3/3 passed** for normal
classification, bbox and review classification. It verifies formal results stay on `/work`,
same-image original/results restoration on reload, Review/Export navigation plus Back to the
exact result URL, invalid Batch link rejection, and return to the original sample URL. Reviewed
`processing-results-bbox.png` showing the existing final projection and quality risks. Synthetic
fixture evidence has no semantic-quality meaning; no Live model or human usability test ran.
Long conversation stacks and viewport density remain M4 work. No push or real Workspace changes.
Next: formal Review/save/return and export completion, remaining conversation orchestration and
default entry, then full acceptance audit. The overall objective is not yet complete.

### M3 continuation — saved Review detail and real export verification (2026-09-08)

Following the formal bbox result through the real Review API exposed a recovery bug: after
acceptance, `reviews(state, Some(id))` filtered the requested annotation out because it was no
longer `needs_review`. The UI could display the POST result but refreshing its stable detail
returned 404. Explicit detail lookup now retains decided annotations; the unscoped pending
queue branch and summary queue still filter pending results. Owner validation remains in place.
The server regression checks readable accepted detail, absence from the pending queue, and
rejection through another Project's detail route. No annotation status was relaxed or rewritten.

Review Save/Accept now share a synchronous mutation lock as well as disabled controls; Save
cannot race with a decision before React's next render. The existing revise and decide services
remain the only writers. Failed revisions leave unsaved edits intact, failed decisions do not
advance, and retries clear the old error before reporting a fresh outcome.

Final browser evidence `/tmp/annotagent-guided-e2e-4127`: **3/3 passed**. The bbox scenario now
injects one decision failure, moves the actual box one pixel with its keyboard control, injects
one revision failure, verifies server geometry unchanged and Save still available, then accepts
through the real server. It verifies changed geometry/human_accepted on detail, reload restoration,
zero unresolved reviews and export readiness. It performs an actual Native export in the TEST
workspace and reads the reported JSON file: exactly the accepted object's ID, corrected geometry
and human_accepted state are present. It returns via Review to the exact conversation result
context, then the original sample. Inspected `formal-export.png`. This verifies bbox delivery,
not Live geometry quality; the classification scenarios do not yet include formal editing/export.

Server **37/37**, relevant strict Clippy, Web **112/112 unit tests**, typecheck/production build,
format and diff checks passed. No Live/paid inference, real Workspace mutation, remote changes
or push. Formal Review/export remain canonical pages (not yet embedded with an explicit workspace
return link); full classification delivery, orchestration/default-entry and M4 audits remain.

### M3 continuation — explicit workspace return and classification delivery (2026-09-08)

Review and Export deep links now preserve a typed `workspace_return`. Only a canonical `/work`
route belonging to the same Project is accepted; external/protocol-relative URLs, another Project,
fragments and unrelated management routes are discarded. The Focus Header provides a visible
Back to annotation workspace action after refresh. Review queue navigation and Continue to export
retain the return context. Returning revalidates owned conversation/Batch data through the existing
workspace loader, not local preferences. Original sample and formal image/candidate keys remain
separate. Advanced canonical pages remain unique; no duplicated Review or Export implementation.

Classification Review exposed another semantic gap: editing only `annotation.label` left
`value.labels` unchanged. A shared input adapter now edits classification values and display label
together, preserves multi-category typing, and leaves final validation to Core. Guided and advanced
Review use the same adapter. Classification correction text no longer claims a geometry measurement.

Final isolated browser run `/tmp/annotagent-guided-e2e-4869`: **3/3 passed**. Both bbox and uncertain
classification now exercise decision failure, revision failure with retained edits, actual acceptance,
detail refresh, actual Native export and reading the exported JSON to match corrected value, ID and
human_accepted status. All scenarios return using the explicit header button after page reload.
Initial classification test timed out on a label locator for the select; using its accessible combobox
role and displayed Wrong label option resolved the test locator, without changing backend behavior.
Inspected `formal-export-classification.png`. Web **114/114** units, typecheck, production build and
format/diff checks passed; known large bundle warning remains. No Rust production changes this step.

No Live model, human usability test, real Workspace mutation, push or remote change. Remaining work
includes broader message intent/reference handling, conditional human requests beyond corrections,
setup returns, continuous orchestration, default workspace rollout, actual control lifecycle tests,
multi-tab/recovery/accessibility/responsive audits and full-workspace regressions. This closes the
tested bbox/classification formal correction/export path, not the complete conversational objective.

### M2 continuation — independent goals from later messages (2026-09-08)

Removed the first-message-only binding in the workspace. Each saved message can explicitly become
an annotation goal through the existing `beginConversationTask` service. The service's unique
conversation/source-message identity restores an existing task instead of duplicating it. The URL
stores the selected task, and the Schema/Builder card resolves its actual source message; a missing
task shows an error rather than silently falling back to message one. Sample navigation now retains
that task context. Selecting a goal clears unrelated Draft/Test/request/result selection, does not
edit other tasks or reset their grants, and invokes no model. Unsaved message/Schema/sample edits
must be saved or undone before switching. Task changes abort/reload the task history observer to
prevent an older task-list response from overriding the new selection.

This is an explicit independent-goal control, not yet automatic intent handling for questions,
stop commands, SchemaPatch or same-task feedback. Those still require the planned coordinator;
the UI does not pretend that every saved message has been acted on.

Evidence: new `conversation-goals.spec.ts` passes in `/tmp/annotagent-guided-e2e-5489` (and earlier
5349/5414): two saved messages, two distinct tasks, selected-goal reload, repeat selection without
a third task, zero authorized/reserved model calls in both ledgers, and invalid-task non-substitution.
The first test fixture omitted required Review config and was corrected; no production validation
was weakened. The existing three conversation sample/correction/formal-delivery scenarios passed
in `/tmp/annotagent-guided-e2e-5198`. Web **114/114** units, typecheck/build and diff checks passed.
Inspected `independent-goals.png`; added visible Current annotation goal text after noticing that
aria-pressed alone did not clearly distinguish selection. No Live tests, real data changes or push.
Full automatic conversation orchestration, setup, richer references, default entry and M4 remain.

### M4 regression checkpoint and production Advisor honesty (2026-09-08)

Re-read all 789 lines of the governing Conversational Workspace request and ran the complete
repository commands. The current vertical slices do not yet satisfy the whole product objective:
automatic continuous coordination, broader frozen references, non-correction Human Requests,
conversation-owned setup return, default entry and additional performance/accessibility work are
still required. Existing full-suite coverage of older routes is not proof of those new capabilities.

One code audit correction: the offline Advisor used by the ordinary TUI/demo entry deliberately
disconnected a valid Commit and reconnected it to demonstrate repair. Fault injection and its
fabricated step records are now confined to the Application crate's test build. Normal builds
validate the actual proposal without manufacturing a failure. Validation observations report the
actual Draft revision. A new integration test compiles Application as an ordinary dependency,
executes the isolated offline fixture path, and verifies no disconnect/connect steps, valid
validation observations and retained human publication approval. Existing unit tests can still
deliberately inject the invalid graph to test repair. Production GUI already gated its scripted
Advisor route to server tests; no real Provider policy or quality guard was weakened.

Full Rust commands after the change: `cargo fmt --all --check`, strict all-workspace/all-targets/
all-features Clippy, all-features build, and all-workspace/all-features tests passed. Aggregated
test output: **551 passed, 0 failed, 5 ignored**. The ignored cases are the explicitly billable
Provider smoke plus legal real-weight PIDNet, RF-DETR, SAM and YOLOX tests; these are NOT Live
model evidence. Web typecheck and **114/114 unit tests** passed. Production build passed with
the existing >500 kB chunk warning.

First full browser run in `/tmp/annotagent-guided-e2e-6020`: 58 passed, one outdated classification
Review test failed, and its serial group's 11 later tests did not run. It still expected a generic
Label field and a geometry-quality claim for classification. Updated those two assertions to
the real Categories control and truthful category-revision text; IME, failed-save preservation,
keyboard decision and refresh assertions remain intact. Final complete rerun in
`/tmp/annotagent-guided-e2e-7057`: **70/70 passed (3.8 minutes)**, including management lifecycle,
scoped navigation, setup, exports, SSE recovery, keyboard/IME, responsive reflow and protocol truth.
The reflow-boundary checks are not a substitute for real browser 200% zoom or human usability.
No push, remote change, real Workspace mutation or old credential use. Real-human usability and
Live quality remain unexecuted; all model transports used here are explicitly TEST fixtures.

### M2 continuation — conversation-owned model setup return (2026-09-08)

The Schema/task card now exposes Review model setup, including after a missing-model preview
error. It opens the existing Provider Registry, not another setup implementation. A typed
settings return context retains the original Project/conversation/task, image, Draft/Test,
human request and formal result selection where present. Providers, Models, Expert Model
Plugins and other existing Settings tabs retain that context. A visible return panel survives
reload and permits leaving setup without making changes. Only same-Project canonical `/work`
targets are accepted; external targets and owner mismatches are stripped. The source Project
name comes from the existing loaded inventory, not a new ownership heuristic.

Returning remounts the saved task and fetches current settings through the existing preview.
It does not automatically restart a model call, accept a consent checkbox or update a Published
Version. Draft/sample dirty guards still apply before leaving. Existing Provider credential,
probe, model binding and plugin permission implementations remain unchanged. This is the real
setup return surface, not yet a persisted ConfigureCapability Human Request or automatic Agent
resume on registry events; those remain part of broader coordination work.

Tests: **115/115 Web unit tests**, typecheck/build/diff checks passed. Combined isolated browser
run `/tmp/annotagent-guided-e2e-7991`: **9/9 passed** (independent goals/setup, three sample and
formal delivery paths, five Provider Registry regressions). Final setup rerun in
`/tmp/annotagent-guided-e2e-8145`: **1/1 passed**, including 390×844 no-horizontal-overflow check.
The setup test first cancels through Models/Plugins back to the original task; then encounters
missing model configuration, creates a TEST Provider and credential through the existing UI,
creates a Model Profile, explicitly consents to its TEST active probe, saves the default planner,
reloads and returns. The task now previews that model, but Generate remains disabled until new
explicit Schema consent, and task reserved calls stay zero. Probe traffic is a separately
consented Registry operation, not falsely counted as a free model request. Initial test used a
region selector for a labelled div; corrected the selector without changing execution behavior.

Inspected `task-model-setup.png` and `task-model-setup-390.png`. No Live/paid Provider, real
Workspace mutation, old credential use, push or remote change. No Rust changes this step.
Remaining: structured no-LLM fallback, continuous bounded coordination, richer references and
human requests, default entry, and final whole-product acceptance/performance/a11y audit.

### M2 continuation — human Schema provenance foundation (2026-09-08)

The existing Schema Draft required a completed model call, so a no-LLM entry could not truthfully
use it. Migration 34 now allows exactly one provenance source: a real model-call ID or a human
request ID. It transactionally rebuilds the metadata and revision tables with foreign keys kept
enabled, copies all IDs/revisions/request keys unchanged, verifies their foreign keys and records
the migration once. No synthetic model call or parallel Schema store was introduced.

Storage now supports idempotent human-authored creation and the existing versioned edits/reloads.
The application validates the same bounded classification/bbox decision and derives the goal
from the task's original saved message. Human creation neither requires a Provider/grant nor
changes Project YAML, permissions, Workflows, published versions or formal annotations. Owner,
payload-conflict and unsupported-output checks remain enforced. Web transport typing reflects
nullable model provenance and explicit human request provenance. The API and visible structured
entry are not connected yet; this is a tested persistence/application foundation, not a claim
that the user-facing no-LLM journey is complete.

Evidence: all 71 Storage tests passed, including new human zero-call/restart/idempotency tests and
a genuine old-format Schema migration test with two revisions. That migration test checks exact
model provenance, old edit-request replay, enforced child foreign keys, exclusive provenance and
second restart. All 5 Schema application tests passed, covering both human output types, stable
saved goal, invalid labels, owner mismatch, unchanged Project goal and empty model-call history.
The first test incorrectly unwrapped an unsupported polygon enum; fixed the test to assert its
earlier deserialization rejection. Storage/Application all-target/all-feature clippy and Web
typecheck passed. No real Workspace was opened or migrated; all test databases were temporary.
No browser screenshot is claimed for this backend-only step. No push or remote change.
The complete Application all-feature rerun also passed: 100 unit tests plus one integration
test; one paid Provider smoke test remained intentionally ignored. Rust formatting passed.

### M2 continuation — visible no-LLM label definition and recovery (2026-09-08)

Connected the human Schema foundation to an owned GET/POST task endpoint and the conversation
goal card. Define labels myself creates/reuses the saved message's task, without consulting a
Provider. Users choose object boxes or whole-image categories, enter labels and optional boundary
rules, and save with a frozen idempotency key. Unknown outcomes retain the input and retry the
same request. Validation rejections permit correction. On reload, the task's saved human draft
is read and the existing Schema editor/Builder card is reused. Human provenance is visible;
there is no fabricated model receipt. Original Project Schema and formal annotations are not
changed. Builder/model authorization remains separate; this does not implement a no-LLM pipeline
planner or the still-pending continuous task coordinator. Unsaved fields participate in the
existing navigation guard. GET/mount/restoration performs no model or write action.

Browser evidence initially exposed the existing desktop sticky composer covering the restored
Schema card. Removed the overlay positioning and prevented flex children from being compressed;
the conversation now scrolls in normal document order. Added viewport and elementFromPoint checks
that the saved draft's edit button is genuinely unobscured. The composer remains reachable by
scrolling; a more compact continuous-conversation layout remains part of the overall UX work.

Isolated E2E `/tmp/annotagent-guided-e2e-9139`: both new paths passed. Each simulates a successful
server save followed by lost response, retries to exactly one draft, refreshes, edits to revision
2, refreshes again, and verifies zero grants/calls, preserved Chinese labels/output type, rejected
foreign conversation lookup and rejected extra authorization fields. Combined regression
`/tmp/annotagent-guided-e2e-9359`: 4/4 passed including existing independent goals/setup and live-HTTP
TEST Schema proposal (not a real Provider). After the composer fix, `/tmp/annotagent-guided-e2e-9477`
passed 3/3 with the new unobscured-control assertions. Web 115 unit tests, typecheck/build, Rust
formatting, Server all-target/all-feature clippy, five Application Schema tests and two Storage
Schema tests passed. Initial TS check caught a missing explicit optional ref type; corrected.

Inspected real isolated-browser `human-schema-bounding_box-390.png` and
`human-schema-classification.png`; desktop screenshots scroll to the saved draft, mobile captures
the input form at 390×844. These empty TEST projects prove goal-before-upload behavior, not model
quality, image import or complete no-LLM-to-results delivery. No real user data, old credentials,
paid models, push or remote changes. Real-human usability remains unexecuted.
Additional post-layout regression `/tmp/annotagent-guided-e2e-9581`: 5/5 passed, including both
human entry paths and all three existing classification/bbox/classification-review sample,
processing, Review and export journeys. Those latter scenarios still use explicitly authorized
TEST transports; they do not prove a human-authored Schema has completed the Builder/sample chain.
Final type/output-summary and zero-authorized-budget checks: typecheck and both browser paths
passed in `/tmp/annotagent-guided-e2e-9694`. Final desktop/classification-mobile screenshots inspected.

### M2 continuation — human Schema to authorized Builder and real delivery (2026-09-08)

End-to-end inspection found a real integration gap: Builder preview required an existing task
grant, which had previously always come from the Schema model call. A legitimate human Schema
therefore could not proceed. Builder now distinguishes preview, first explicit authorization,
and continuation of an existing authorization. With no grant, preview reports zero previous
spend and at most eight Builder calls; GET creates nothing. Only explicit consent creates the
initial grant through the existing transactional call ledger. Subsequent phases retain the
original cumulative rules. A repeated initial authorization cannot replace another grant or
reset spend. Existing persisted operation receipts retain their original replay path. No fake
Schema completion/grant, extra executor, new Provider or automatic authorization was added.

Expanded the existing full sample/delivery E2E with human-classification and human-bbox variants,
not a separate simplified pipeline. Each uploads a TEST image, manually saves semantics, reloads,
confirms the zero-spend/eight-call Builder preview, authorizes the existing HTTP TEST planner,
authorizes samples, receives terminal results, then follows the same conditional assistance,
formal confirmation, Batch results, Review and native-export assertions as the model-authored
scenarios. Both passed in `/tmp/annotagent-guided-e2e-10145` (29.4 seconds total). The tests reject
a new request attempting to reuse `previous_grant_id:null` after authorization and verify that
the complete task budget is unchanged. Model-call history contains no Schema proposal receipt.

Application tests additionally cover absent initial budget, owner rejection, duplicate initial
grant, one recorded call, rejected fresh initial grant, explicit cumulative continuation and
retrying the old grant without changing current scope or spend. Five Schema tests and two call
ledger tests passed; Server all-target/all-feature clippy, Web typecheck and 115 unit tests passed.
Initial test run had a local `human` variable shadowing the scenario flag; renamed the flag.
Clippy requested an explicit three-case authorization enum and Copy semantics; both corrected.
These are TEST transport/control-flow results, not real model accuracy or human usability.
No real Workspace, paid model, old key, push or remote change.
Combined post-fix delivery regression in `/tmp/annotagent-guided-e2e-10367`: all five scenarios
passed (1.6 minutes), including existing model-authored classification, bbox and uncertain
classification. Inspected `sample-human-classification.png` and `sample-human-bbox.png` from the
isolated browser. Their synthetic image/fixture predictions illustrate transport and editable
terminal-result handling only: the fixture cup bbox is deliberately not real semantic evidence.
Final targeted rerun `/tmp/annotagent-guided-e2e-10515`: both human delivery paths passed, including
asserting the exact authorization-changed rejection for attempted budget reset (not merely any
HTTP failure). Overall coordinator, Project-lifetime budgets, richer conversation references and
the remaining whole-product acceptance matrix are still open.

### M2 continuation — frozen candidate reference boundary (2026-09-08)

The journal previously froze only an image UUID/hash. Added an optional typed `sample_candidate`
reference retaining task ID/base Project Schema revision, Sample Test ID, exact Workflow Draft
revision, terminal candidate ID and source Artifact UUID. Project/conversation remain the owned
journal envelope, and image/hash remain the existing frozen input. Old message JSON omits this
optional field and continues to deserialize/equal-replay unchanged. No new database or parallel
message store is needed. Existing Rust message fixtures explicitly retain no selection.

Application admission verifies the task and immutable sample operation/report, owner, Draft
revision, matching image/hash and exactly one terminal candidate/Artifact pair. Intermediate or
made-up candidates, foreign tasks and mismatched revisions fail. Storage rechecks task ownership
and image hash in the append transaction. Identical retry restores the original saved reference
even after current image state changes; changing the same message ID's reference fails. Neither
admission nor retry authorizes inference, edits feedback, changes labels or accepts annotations.

A selected-object message cannot be admitted as a new project-wide goal. The existing task
admission rejects that widening explicitly, and the journal replaces its Use as goal button with
a visible candidate-only scope note. This prevents loss of scope while richer task coordination
is still being built. It is not yet automatic natural-language local correction: the canvas
selection → composer chip → send integration, corrected-feedback revision references, formal
annotation references and model-context propagation are still pending. Current references identify
the saved terminal sample outcome, not a later unsaved canvas edit.

Evidence: new persistent journal restart/scope-widening test, two existing message tests and two
task-admission tests passed. Complete Application all-feature suite: 100 unit + one integration
passed, one paid test ignored. Workspace all-target check, Server all-target/all-feature clippy,
Web typecheck and 115 unit tests passed. Existing full HTTP TEST human-classification and human-bbox
journeys were expanded with reference POST/read/idempotency/invalid-reference/budget assertions;
both passed in `/tmp/annotagent-guided-e2e-11021`, then again in
`/tmp/annotagent-guided-e2e-11172` with the project-goal widening guard. No live/paid model, real
Workspace mutation, old key, push or remote changes. This is a selection-boundary foundation,
not a claim that the complete conversational pointing interaction is delivered.
Final browser rerun `/tmp/annotagent-guided-e2e-11452`: 2/2 passed, including reload visibility of
the candidate-only scope note and absence of a project-wide goal action for that message.

### M2 continuation — canvas selection to frozen message input (2026-09-08)

The existing sample editor now exposes Reference saved candidate in message for one selected
terminal outcome. It is unavailable for unsaved edits, image-only/before views or human-added
objects without an original prediction. The label explicitly refers to the original saved
prediction, not later feedback geometry. ConversationSampleCanvas resolves the terminal projection
and supplies exact Test/Draft revision, candidate/Artifact identity and image/hash to the same
journal input. The composer shows a pinned candidate-only reference, permits removal before
submission and retains the reference when the visible image changes. Focus moves to the message
field (including the mobile conversation panel); submitting uses the existing frozen message ID.
Uncertain-response retries lock the same text/reference, and successful save clears the composer
selection. Project/conversation/task changes clear an unsent selection. No model is automatically
called and no annotation is changed by this action.

The first browser pass found the processing confirmation context hid the composer. Referencing
a candidate now opens it in place while preserving processing state. A later run accidentally
used an already-loaded older identical-image fixture and correctly encountered import dedupe;
verified the isolated database, stopped only that known test process, and reran with a distinct
bundled icon as the second local image (never sent to inference). Another assertion incorrectly
expected the first sample ID after the existing repair journey; it now checks the actual selected
Test in the URL. Both UI journeys passed in `/tmp/annotagent-guided-e2e-12358`: choose candidate,
focus input, switch to a different image, keep original reference, lose a successful POST response,
retry, reload one saved message and verify unchanged call budget. This proves the binding, not
semantic accuracy. Initial TS callback narrowing issue was corrected by capturing the validated
projection. Typecheck and 115 Web unit tests passed.

Screenshot inspection also revealed an unrelated-looking but real status error: the human Schema
card interpreted subsequent Builder/sample receipts as a failed Schema proposal. Human mode now
renders its own actual draft and no model-proposal failure/cancellation controls. Existing Builder
and sample history remain intact. The still-long left history is recorded as unfinished UX work,
not accepted as the final compact conversational experience. Corrected-feedback/formal annotation
references, model-context propagation and continuous bounded coordination remain pending.
Final combined regression `/tmp/annotagent-guided-e2e-12512`: six scenarios passed, covering all
five existing/extended sample-to-delivery journeys plus the Schema HTTP consent/receipt path.
Inspected the 390px candidate-message screenshot and both `candidate-reference-chip-*.png` crops:
scope, image, candidate and revision wrap without horizontal overflow; removal remains visible.
The full mobile history remains long and is not claimed as finished compact UX. Existing TEST
fixture predictions are not accuracy evidence. No Rust changes in this step, no push, no remote
change and no real Workspace/paid Provider/old credential use. Real-human usability unexecuted.

### M2 continuation — reopen frozen candidate messages (2026-09-08)

Saved candidate-scoped messages now open the original terminal prediction in the existing
conversation canvas. The canonical work URL stores the message ID together with its exact
conversation/task/Draft/Test/image context. The loaded owned journal must match that context;
the saved sample must also match the Draft revision, image hash and unique candidate/Artifact
pair. Invalid references show an error without selecting another result. Refresh and model setup
return preserve the same reference. Opening the current sample corrections deliberately removes
the historical-message context. None of those navigation actions writes feedback or calls a model.

The historical view uses the shared AnnotationCanvas in read-only mode, with the original
terminal outcome rather than later sample feedback. A shared outcome-to-annotation converter
keeps the existing editable sample path and historical bbox/classification rendering consistent.
The original-prediction explanation and current-corrections action make this boundary explicit.
Screenshots revealed an unnecessarily tall historical canvas; it now uses a bounded responsive
height without changing image pixels. This does not complete corrected-feedback references or
the model's interpretation of a candidate-scoped message; both remain outstanding.

Typecheck and 117 Web unit tests passed. The initial two human-schema browser journeys passed
in `/tmp/annotagent-guided-e2e-13067`. Expanded tests verify original bbox geometry/classification,
reload, forged message and image rejection, setup-return context and zero navigation mutations.
A subsequent combined suite exposed a TEST harness defect: its successful-response-loss simulation
discarded a real 429 mutation-rate rejection. The existing TEST-only pacing helper now checks
pre-execution rate-limit responses before simulating loss, and asserts success. Production limits
are unchanged. A first retry then timed out at the default 10-second UI assertion while the helper
was legitimately waiting for the rate window; the assertion now allows the helper's bounded wait.
The full combined rerun result is recorded below when available. No Live/paid Provider, real user
data changes, old key use, push or remote modification. Real-human usability remains unexecuted.
Final combined rerun `/tmp/annotagent-guided-e2e-13684`: 6/6 passed (2.2 minutes), including
five full sample/delivery scenarios and independent goals/model setup. Typecheck and diff hygiene
passed after the changes. Inspected both `message-reference-reopen-*.png` screenshots: the
bounded canvas now fits the complete source image without distorting it; desktop panel scrolling
still places the current-corrections button below the screenshot crop, and the test operates that
button successfully. These synthetic classifications and boxes remain explicitly TEST evidence,
not real-world model quality evidence. The production-build large-chunk warning remains open.

### M2/M4 continuation — Project conversation usage visibility (2026-09-08)

The existing owned task-budget endpoint now also reports Project-wide conversation allocations
and reserved calls. It derives ownership from stable Project identity and aggregates planning
grants/call receipts separately from linked Batch allowances; it does not multiply grants by a
receipt join, count authorization revisions repeatedly, or depend on a late `started` receipt.
Failed and indeterminate reservations remain charged. New conversations and independent goals
retain Project totals while their own task totals correctly begin at zero. Another Project's
ledger is excluded. Processing confirmation displays the aggregate as call counts, explicitly
not money, an estimate or a new authorization. Existing saved receipts without these optional
Web fields do not invent a zero Project total.

Important remaining boundary: this is persistent cumulative visibility, NOT a configurable
Project-wide spending cap. Existing task/Batch transactional admission caps still apply; no
arbitrary 128-call Project cap was introduced. Project-wide explicit ceiling configuration,
atomic admission across tasks and early planning authorization presentation remain to be built.
The original goal's no-budget-reset requirement is not declared complete by this step.

Expanded storage regression covers planning failures, Batch spend, restart, new conversation,
new task, additional authorization and foreign-Project isolation. All Storage tests passed
(56 unit + 16 integration), and all-target/all-feature Storage clippy passed. Web typecheck and
the full human-classification TEST browser delivery path passed in
`/tmp/annotagent-guided-e2e-14099` (1/1), including displayed Project totals and unchanged task/
Project counts after formal processing. This run rebuilt the real application against the
isolated HTTP TEST transport. No Live inference/accuracy or human usability claim, no real data
mutation, old credential use, push or remote modification.

### M2/M4 continuation — explicit shared Project conversation ceiling (2026-09-08)

Migration 35 adds immutable, optimistic-revision-checked Project ceiling changes. A stable Project
identity, not the active task or a client-supplied owner, selects the ledger. Same-ID retries
validate the original input and return current state without undoing a newer ceiling. A stale
new request fails; a ceiling below already-reserved usage fails. A zero ceiling before any spend
is valid. Reads, reloads and opening a conversation never configure or raise a ceiling.

Both conversation call admission and linked Dataset Batch model-call admission now check the
shared cumulative ceiling inside their existing reservation transactions. No new executor is
introduced. The test races independent SQLite connections (planning versus Batch), proves only
one call fits a one-call Project allowance, then verifies restart, a second task's rejection,
explicit expansion and exhaustion again. Existing phase-specific authorization is still required;
setting a Project limit alone cannot dispatch a model request. Failures and unknown reservations
are not refunded. Previously admitted receipt retries keep their original non-executing behavior.

The workspace has an on-demand Project call limit panel with exact cumulative input and explicit
confirmation. Lost-response retries freeze the same command; reload discards unsaved input only
after confirmation and reads the latest revision. Unsaved budget edits participate in the parent
dirty guard. The displayed usage is labelled a saved snapshot, not live progress. Inspected
`project-call-limit-390.png`: form, consent and save/reload actions wrap within the narrow panel.

Scope/remaining limitation: this ceiling is optional for compatibility with already-authorized
work. An unconfigured Project retains existing task/Batch limits; no hidden default cap or budget
expansion was added. It covers conversation-owned planners, samples and linked processing, not
unrelated legacy workflows, Provider probes or currency spend. That boundary is explicit in UI.
The future continuous coordinator still needs to include this ceiling in its compact initial
authorization/context and budget-exhaustion recovery flow; full conversational experience is not
declared finished by adding this panel.

Validation: 57 Storage unit and 16 integration tests passed; all-target/all-feature Server clippy,
Rust formatting, Web typecheck and 117 Web units passed. Isolated HTTP TEST browser run
`/tmp/annotagent-guided-e2e-14640` passed 2/2: save/reload/lost response/stale revision/foreign
Project/extra authorization-field rejection, plus actual Schema transport admission blocked at
zero, explicitly expanded and then exhausted again by a new task. Final run
`/tmp/annotagent-guided-e2e-14878` passed 2/2: the budget UI and full human-classification journey
through Builder, sample repair, formal processing, Review/export and candidate references with
an explicit 64-call shared ceiling. The Project count matches planning plus actual Batch use.
No real Workspace mutations, paid/Live Provider calls, old keys, push or remote changes. No
Live accuracy or real-human usability validation; production-build chunk-size warning remains.

### M2 continuation — show shared budget before new inference (2026-09-08)

Schema, Builder (including repair) and sample authorization previews now include an owned
Project ceiling snapshot. It is presentation metadata, not another model grant or a replacement
for transactional admission. Shared UI distinguishes an unconfigured ceiling, remaining calls,
exhaustion and unavailable/malformed accounting. New call actions are disabled at an exhausted
or unavailable snapshot; a partial remaining allowance warns that the step may stop at the limit.
Uncertain already-submitted request recovery remains separate. Refresh explicitly re-reads the
authorization; it does not raise the ceiling or start inference. Existing saved receipts are not
rewritten with current usage. The no-ceiling state still explains that task limits apply.

Browser testing exposed a real refresh race: a previously checked consent could be clicked again
while a preview was refreshing, then cleared by the response. All three cards now clear consent
at refresh start and disable it while busy. Builder Back is also disabled during refresh so a
late response cannot reopen a card the user just closed. A held-response browser assertion checks
the unchecked/disabled consent before releasing the new preview. The combined run's cancellation
check failed twice. Trace inspection proved repeated pre-execution 429 responses: the TEST helper
stopped after 45 seconds, shorter than the real 60-second rate window. Its bound is now 65 seconds,
and this combined test's observation/overall limits allow that wait. Production limits are unchanged.
Stop actions sharing this general write limiter remain a separate responsiveness concern to assess.
The exhausted notice also opens the existing Project limit panel in place and focuses its numeric
input; it does not navigate away or change the limit. Saving and refreshing remain explicit actions.

119 Web units, typecheck, Server all-target/all-feature clippy and formatting passed. Expanded
browser paths exercise zero-budget Builder, sample and Schema previews, explicit external ceiling
updates, authorization refresh/reconfirmation and continuing the existing TEST sample-to-delivery
journey. Inspected `sample-project-budget-exhausted.png`: cumulative usage, non-authorizing scope
explanation and refresh action are visible without overflow. Final combined result follows below.

Still open: if another task consumes the last call after preview but before Schema admission,
the first grant may already be saved when the reservation fails. The UI must preserve the exact
original consent for recovery instead of creating a new grant identity. This is not resolved by
the preview gate and remains a required next step. Continuous coordination, richer human requests,
default compact layout and the broader original acceptance matrix remain unfinished. No paid/Live
Provider, original Workspace changes, old credentials, push or remote modifications in this step.
Combined run `/tmp/annotagent-guided-e2e-16089` passed 2/2 (1.1 minutes), including the held
Schema-preview response and cancelled-request recovery. Testing the new in-place budget entry
then exposed another real race: Reload saved limit kept the old revision and editable input on
screen while its GET was pending. A late read could overwrite the new number before Save.
The budget editor now explicitly marks loading, hides the old snapshot and disables editing,
confirmation and Save until the read finishes. Final full human-classification delivery run
`/tmp/annotagent-guided-e2e-16340` passed 1/1 with actual in-page ceiling editing, focus restoration,
refresh and resumed Builder/sample actions. Web typecheck, 119 units and diff hygiene passed again.

### M2/M4 continuation — recover original Schema consent after admission rejection (2026-09-08)

Migration 36 persists the original Schema consent alongside its initial task authorization in
one transaction. It stores model identity, call ID, scope digest, original expiry and explicit
unknown-price consent, without credentials. The shared initial-grant implementation retains
historical idempotency behavior; adding the consent cannot replace the grant or reset usage.
A forced envelope-insert failure test proves that no partial authorization survives the rollback.
Conflicting model identity or request ID is rejected. The pending read is Project/task-owned and
returns only a current authorization with no call reservation and no saved cancellation.

The GUI restores that server-side consent after reload, displays “saved, but no model call was
admitted” rather than running, and offers explicit review/retry/cancel. Budget refresh uses the
original model and checks its scope digest; the actual retry sends the original expiry and call
ID, not fields regenerated by the latest preview. It neither renews expired consent nor bypasses
current model/data checks. Missing or changed models remain a real failure. After an uncertain
retry response, the GUI reads both pending authorization and actual call receipts; a completed
call is displayed as completed and clears the transient transport error, not left stuck pending.

Validation: new restart/idempotency/rollback/foreign ownership test passed, followed by all
58 Storage unit + 16 integration tests and Server all-target/all-feature clippy. Web typecheck
and 119 units passed. The first browser run caught the completed-but-lost-response state bug
above; after the fix, `/tmp/annotagent-guided-e2e-17018` passed the extended Schema scenario.
It verifies server budget rejection before dispatch, exact pending consent, reload, disabled
retry at zero, explicit ceiling increase, same-envelope retry, lost response and one saved model
result. The combined regression with human-classification delivery is recorded below when done.

Compatibility boundary: no synthetic original consent was backfilled for pre-migration failed
grants that never saved a model identity. Their historical budget remains intact, but automatic
UI restoration of an unknown original envelope is not claimed. Continuous coordination and the
remaining full product requirements are still open. Tests use only isolated HTTP TEST transport;
no real Workspace mutation, old key, paid/Live model, push or remote change. No human usability test.
Combined `/tmp/annotagent-guided-e2e-17188` passed 2/2 (1.1 minutes), including full
human-classification delivery and the expanded Schema transport/cancellation scenario. Final
independent `/tmp/annotagent-guided-e2e-17309` passed 1/1 after removing the opaque model UUID
from the default pending card; exact model details remain in the verified authorization preview.
Inspected `schema-pending-authorization.png` and `schema-original-consent-recovered.png` for
saved-versus-running wording and clear recovery controls. Typecheck and diff hygiene passed.

### M2 continuation — answer a Schema clarification in the original task

The existing Schema service could return `Clarify`, but the GUI displayed the question without
an answer path. The workspace now offers an explicit structured answer for output type, labels
and boundary rules. It reuses the human-authored versioned Schema Draft, then the existing
Builder authorization panel. Saving the answer itself makes no model request, changes no
Project YAML, publishes nothing and writes no formal annotation.

The immutable completed Schema call is the request source, with owned conversation/task/message
and original Schema revision. Migration 37 stores only the answer-to-Draft linkage. Answer and
Schema creation share one transaction; conflicting repeat answers and stale references fail
closed. Exact retries survive response loss and database reopen. An unanswered clarification
blocks new task model-call admission; replaying an existing receipt remains read-only. Answering
does not reset the previously reserved call count or grant further inference permissions.

Validation: all 59 Storage unit and 16 integration tests passed, including injected answer-link
failure rollback, foreign ownership, stale revision, same-ID restart retry, pending-call gate and
post-answer usage preservation. Server and HTTP fixture all-target/all-feature clippy passed.
Web typecheck and 119 unit tests passed (`npm --prefix web test`; `test:unit` does not exist).
The isolated HTTP TEST browser scenario passed 1/1 in `/tmp/annotagent-guided-e2e-17753`:
real Schema transport returns a scripted clarification, Chinese labels/rules are saved, the
successful response is deliberately lost, explicit same-request retry restores one Draft,
refresh retains it and opening Builder authorization leaves the budget unchanged at one call.
Production build ran through this browser harness; the existing large-chunk warning remains.
Inspected `conversational-workspace/clarification-answer-restored.png` for the saved revision,
labels and explicit next-phase consent. The card still has dense budget explanatory text;
this is functional recovery evidence, not final compact-layout or human-usability acceptance.

Scope limit: this implements the annotation-semantics clarification only, not all HumanRequest
types or free-text interpretation. The browser scenario stops at Builder authorization rather
than making a second model request; other shared human-Schema delivery tests remain separate
evidence. Deferral/cancellation UI for this request type and continuous coordination remain open.
No Live/paid model or real user Workspace was used, no old key, no push or remote change.
Full-workspace regression and real-human usability have not been repeated for this increment.

### M2 continuation — execute the clarified task through its sample canvas

Extended the clarification browser scenario beyond the authorization panel: the saved answer now
goes through the real Builder orchestration, compatible model binding, static validation,
separately authorized sample operation, terminal projection and the existing editable canvas.
The TEST transport handles both the initial scripted text clarification and subsequent bounded
Builder/classifier protocol requests; image pixels are not sent by the Schema phase. The test
uploads the repository synthetic image to its isolated Project, explicitly binds the TEST model,
asserts an actual classifier node with that model identity and nonempty terminal projection,
then reloads and reopens the canvas without writes or additional usage. All operations remain
under the original task. This supersedes the previous increment's authorization-only browser
limitation, not the remaining product-wide limitations.

Added a compact saved-answer provenance block with an on-demand original question. It is shown
only after the owned clarification API confirms `applied` and the exact linked Schema Draft ID;
the client does not infer that an arbitrary human Draft answered a nearby model question.
Unavailable evidence cannot produce a false “answered” claim. The original model question is
read-only; editing labels continues to create revisions through the existing Schema editor.

Validation: Web typecheck, 119 unit tests and production build passed. The extended isolated
browser test passed in `/tmp/annotagent-guided-e2e-18611` (1/1, 8.1 seconds including build/start).
Inspected `conversational-workspace/clarification-sample-result.png`: the original synthetic
image and a terminal classification are visible in the real workspace canvas, explicitly
labelled evaluation-only. Its scripted “室内” output is not a quality assessment of that image.
The project header identifies TEST data; no Live accuracy or human usability claim is made.
Combined human-Schema delivery regression is recorded below after completion.

No Rust production change in this increment; prior targeted Rust validation is retained rather
than represented as a new full-workspace run. No real Workspace, old key, push or remote change.
The left-hand historical cards remain too long and the default continuous coordinator is still
incomplete. Deferral/cancellation for clarification and the other HumanRequest types remain open.
Combined `/tmp/annotagent-guided-e2e-18683` initially failed the next scenario's model-identity
assertion: the new compatible TEST profile was not included in the suite's existing scenario
retirement prefix. Fixed the isolated fixture profile naming and prior-scenario retirement,
without altering production model selection. Re-run `/tmp/annotagent-guided-e2e-18775` passed
2/2 (17.6 seconds), including the existing human-classification full delivery regression.

### M3/M4 continuation — fold completed planning history, not active controls

The default conversation previously continued to show Builder introduction, used revision,
rebuild request, outcome explanation and advanced Pipeline link alongside the current sample
task. These completed planning controls now live under one native, keyboard-operable “Builder
outcome saved / View build details” disclosure. It reuses the same component state, API and
advanced link; no second execution path or management page was created. Sample authorization,
results, assistance failures and active stop controls remain outside this disclosure.

Folding is conditional on a completed operation with a real Draft, `draft_ready_for_human_review`,
no unresolved bindings/errors/cancellation, no active request or editor, and exact current Schema
revision. Blocked/failed/interrupted/unknown results remain exposed. Editing labels to a newer
revision restores the visible stale-plan warning rather than hiding it in saved history. The
disclosure does not imply model quality, publication, annotation acceptance or new consent.

Validation: Web typecheck and 119 units passed. Initial isolated combined clarification plus
human-classification delivery passed 2/2 in `/tmp/annotagent-guided-e2e-19039` (17.6 seconds).
Added explicit keyboard Enter open/close, advanced link visibility, directly visible next sample
operation and label-revision-change uncollapse checks. The final Schema cancellation/transport
regression is recorded below when finished. Inspected `clarification-sample-result.png`: the
saved-plan controls are folded while the terminal canvas and sample actions remain visible.
This reduces one historical interaction region, not a claim that the entire default journey or
all long cards have been simplified. Continuous orchestration and other open requirements remain.
Only isolated HTTP TEST transport and synthetic data; no Live or human-usability validation,
real Workspace writes, old credential use, push or remote modification.
Final `/tmp/annotagent-guided-e2e-19253` passed 2/2 (11.2 seconds): clarification/sample
continuation including stale-revision exposure, plus original Schema response-loss, restoration,
advanced-link and cancellation tests. The preceding run caught a test expecting the advanced
link to be exposed without opening the new disclosure; the test now opens the real details
control before verifying its href. The link itself and route were not removed or substituted.
Current typecheck and diff hygiene passed. Production build passed via the harness, retaining
the known large-chunk warning. No new Rust changes or claim of a new full Rust regression.

### M2/M4 continuation — durable cancellation of unanswered Schema clarification

An unanswered clarification now has an explicit Cancel action before or during form entry.
The confirmation states that unsaved input is discarded while the question and consumed calls
remain saved. The API validates exact Project/conversation/task/call ownership and expected
Schema revision, then reuses the existing durable call-cancellation table. No migration, new
executor, grant reset or history deletion is involved. Already-applied answers cannot be
cancelled through this action; cancellation and answer creation serialize in their existing
SQLite transactions. A cancelled question rejects a late answer without creating a Draft and
continues to block further model admission from this unanswered task. Starting another goal
uses the existing independent task mechanism and cannot erase the Project-wide usage total.

Cancellation has no optimistic success label: on uncertain transport failure the entered form
remains with an explicit unconfirmed message; refresh reads the server's saved cancellation.
Answer-save-in-flight/unknown states disable the local Cancel action rather than silently
discarding an uncertain successful answer. The cancelled view retains the original question
with a cancelled title, no answer action, and an explanation of how to start a separate goal.
This is cancellation, not temporary deferral or completed human review.

Validation: all 60 Storage units + 16 integrations passed, including stale/foreign rejection,
restart and duplicate cancellation, late-answer rejection, preserved call count, blocked new
admission and refusal to cancel an applied answer. Server all-target/all-feature clippy passed.
Web typecheck and 119 units passed. Isolated `/tmp/annotagent-guided-e2e-19606` passed both answer
and cancel paths (2/2, 20.2 seconds including build/start): the cancellation response was lost
after the server accepted it, refresh recovered the cancelled state, API retry was idempotent,
late answer produced no human Schema Draft and usage stayed unchanged. Final wording/screenshot
verification is recorded below. No real Workspace mutation, old key, Live model, push or remote
change. Broader coordinator work, other HumanRequest types and real-human usability remain open.
Final `/tmp/annotagent-guided-e2e-19837` passed 2/2 (8.9 seconds). Inspected
`conversational-workspace/clarification-cancelled.png` with the cancellation explanation and
original question visible together. Production build passed with the existing chunk-size warning;
fmt and diff hygiene passed. No full-workspace Rust or full-browser-suite pass is claimed here.

### M4 checkpoint — full regression after clarification and disclosure changes

Re-ran the requested Rust workspace commands against `c85fcea` production code:
`cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`, and
`cargo build --workspace --all-features`. The chained command completed with exit 0.
Five explicitly ignored tests remain excluded: billable Pipeline Builder Provider smoke and
real-weight process workflows for PIDNet, RF-DETR, SAM and YOLOX. This verifies the available
workspace regression, not those Live model/weight scenarios or real-human usability.

Started all 77 browser scenarios in `/tmp/annotagent-guided-e2e-20012` using TEST-only servers.
The independent-goals/model-setup scenario exposed a shared-suite precondition: it expected no
default planner, but the new preceding clarification scenario left a compatible TEST planner.
Updated the test to explicitly clear the isolated registry's planner default and explicitly
choose its newly created TEST Provider when adding its model. It no longer assumes either an
empty default or the first Provider option. Production registry selection is unchanged.
The original run is allowed to finish; its final results and subsequent verification follow.
The full run completed 76/77 in 5.1 minutes, with only that missing-planner precondition failure.
All remaining scenarios passed, including sample delivery, ownership/deep-link recovery,
Review/Run navigation, immutable publication, export, history restore/delete safety, provider
and plugin setup, keyboard and responsive layouts. The 200-percent test explicitly simulates
the 640 CSS-pixel reflow boundary, not native browser zoom; its result is not stronger than that.

The first focused replay exposed an exact-label test selector mismatch on the wrapped Provider
select. The accessibility snapshot showed a correctly named Provider combobox; the test now
uses that role/name instead of wrapped-label text. Final `/tmp/annotagent-guided-e2e-20771`
passed the three consecutive clarification-answer, clarification-cancel and missing-model
setup/return scenarios (3/3, 11 seconds). Web typecheck and 119 units also passed. No production
code or test assertion was weakened to bypass the missing-model state. This is 76/77 plus a
passing focused correction, not a claimed fresh 77/77 full run. The known production chunk-size
warning remains. No Live model or human-usability test; no real Workspace, remote or push changes.

### M1/M2 continuation — first goal proceeds directly to authorization preparation

The empty conversation's primary action is now “Save goal and prepare labels”. It persists the
message with its frozen image reference, creates or restores the same message-owned task via
the existing admission API, selects its canonical workspace URL and reads the existing Schema
model authorization preview. This removes the separate first-message goal selection/preparation
clicks without dispatching inference. “Save message” remains a secondary journal-only operation.
Later notes and candidate-scoped messages retain their previous behavior; they are not silently
promoted into independent goals or project-wide corrections.

The explicit preparation intent is retained through same-message retries. If message persistence
succeeds but task preparation has an uncertain result, the UI distinguishes those outcomes and
keeps the frozen message; retry first restores any existing task rather than resetting its budget.
Page mount/reload does not replay that mutation or authorize inference. The preview is requested
only after a saved task exists and still requires explicit model/data/unknown-price consent.
Missing-model failures retain the saved goal and existing setup/manual-label alternatives.

The new browser path deliberately loses a successful task-admission response. Its first run
caught a real component race: the Schema panel had loaded before the task existed and did not
reread it after the explicit preparation completed. Fixed that read dependency using the explicit
preparation request, keeping the old request abort and read-only mount behavior. The corrected
joint run `/tmp/annotagent-guided-e2e-21176` passed 4/4 (11.9 seconds): first-goal preparation
through clarified samples, clarification cancellation, independent goals/setup-return and frozen
journal response-loss recovery. It verifies one persisted message/task and zero model calls at
the first authorization boundary. No Rust engine, publication or formal annotation logic changed.
Final typecheck/units and screenshot replay follow below. This improves the first transition;
it does not complete the remaining continuous coordinator or all structured HumanRequest types.
No Live model, real Workspace, old key, push or remote change; no human usability test.
Final `/tmp/annotagent-guided-e2e-21308` passed 2/2 (9.2 seconds), Web typecheck and 119 units
passed. Inspected `conversational-workspace/first-goal-authorization.png`: real TEST model name,
destination, one-call limit, text-only scope and unknown-price consent precede the disabled
Generate action. Production build passed with the known large-chunk warning. Diff hygiene passed.

### M1/M3 continuation — make the conversation workspace the actual project entry

Audited the active App callbacks against the latest specification's replacement of the old
default guided journey. The baseline browser test failed because newly uploaded projects still
navigated to `/task/goal`. New Project upload completion and project-inventory selection now use
the existing `projectWorkPath` route builder and `/projects/:id/work`. The same Project/image
importer, persisted journal, Schema and runtime are reused. No workflow is copied or automatically
executed. The first-goal action from the previous increment is now reachable through normal
creation/inventory entry, not only a hand-entered URL.

`Back to project` still opens the original management overview. Project-menu management, explicit
Build/Journey, Run/Review/Export deep links and their return callbacks are unchanged. No redirect
was added to an already-open specific task. The legacy journey tests now deliberately navigate
to their existing goal-editor deep links after verifying new creation lands in `/work`; their
model setup, goal persistence, cancellation and sample authorization contracts remain tested.
This changes the default entry, not the underlying management implementations.

The entry screenshot review caught a misleading transient empty-images prompt while the Project
inventory was still loading. That state now says “Loading saved images…”; the true empty-state
import instructions appear only once the saved inventory has loaded. A delayed-images browser
check verifies the distinction. The entry test also verifies one uploaded image, management return,
inventory reopening and no API mutations on refresh/navigation. Synthetic TEST names are unique
so the scenario does not select another Project with the same fixture filename.

Validation: baseline `/tmp/annotagent-guided-e2e-21586` failed at the old `/task/goal` destination;
initial corrected entry `/tmp/annotagent-guided-e2e-21663` passed. Combined default-entry and
retained legacy image/goal/model/local-model/ready-sample flows passed 6/6 in
`/tmp/annotagent-guided-e2e-21784` (46.1 seconds). Web typecheck and 119 units passed. Final
delayed-image/clarification replay and settled screenshot inspection follow below. No Rust engine
change, real Workspace mutation, old key, Live model, push or remote change. Existing task deep
links retain exact context; inventory-based selection of the user's last task still needs a
durable resume policy rather than guessing from a local active-project preference. The full
coordinator and other open product requirements, including human usability, remain unfinished.
Final `/tmp/annotagent-guided-e2e-21917` passed 3/3 (9.9 seconds), covering clarification answer,
cancellation and entry with delayed image restoration. Inspected the settled
`conversational-workspace/default-project-entry.png`: uploaded TEST pixels are visible beside
the first-goal composer with no global sidebar. Build/typecheck passed; the existing chunk-size
warning remains. This is not a new full-browser-suite or native 200-percent-zoom result.

### M1/M4 continuation — durable task selection without replaying execution

Project inventory entry previously restored the journal but could return to its first goal.
Migration 38 now persists explicit task selections in the canonical conversation, using stable
Project ownership, an immutable request ID and an expected selection revision. Storage rejects
foreign tasks and stale writes. An exact retry returns the latest selection, rather than replaying
an old selection over a newer one. Reopening SQLite preserves the selection and does not reset
task usage. This is navigation state only, not a new execution service or permission grant.

The existing workspace remembers explicit goal selection and the combined first-goal action.
Root workspace entry reads the pointer; explicit task/Draft/Sample/Review/processing/result links
take precedence. GET, mount and refresh never save selections or call a model. Loading disables
the message composer until saved context is known. Route context changes cancel pending restore
reads; dirty edits prevent automatic restoration from navigating away. A failed selection response
keeps the original command for retry and does not falsely navigate to a confirmed selection.

Validation: storage tests passed (61 unit plus 16 integration); server-target Clippy with all
targets/features and warnings denied passed; cargo format check passed. Web typecheck, 119 unit
tests and production build passed (existing chunk-size warning remains). Initial focused browser
run `/tmp/annotagent-guided-e2e-22805` passed 4/4. Final run
`/tmp/annotagent-guided-e2e-23049` passed 4/4 in 12.4 seconds, including server-success/response-loss
selection retry without a new revision, root restoration, explicit-link precedence, delayed root
read followed by a same-document task switch, zero restoration mutations, zero task model usage,
model-settings return, clarification answer/cancel and default project entry. Storage checks
also reject changed request reuse and stale revision writes after a newer selection.

Inspected `conversational-workspace/task-selection-restored.png`: the second saved goal is marked
current after root entry, with the actual TEST empty-image state. This is not model quality or
human usability evidence. Only the selected task is persisted by this increment; exact image,
candidate and mode still rely on existing deep links, not a new last-view checkpoint. Restoration
uses the existing navigation callback; complete browser-history/replace semantics and a full
multi-tab usability pass remain open. No live 8787 workspace changes, real Provider calls, old
keys, push or remote changes. The broader coordinator and remaining HumanRequest types are not
complete. Unrelated regenerated screenshot changes are left unstaged.

### M3/M4 continuation — task-scoped human help instead of a flat request inventory

The workspace previously rendered all requests across all saved goals as equal active cards,
including cancelled and applied history. Extracted the existing request presentation (same APIs,
request IDs, answer and resume service) into `ConversationHumanRequests`. Current-goal pending and
answered requests stay directly visible; the explicitly opened request stays visible after it is
applied/cancelled, so saving does not remove the object being inspected. Other goals and closed
history remain in one keyboard-operable native disclosure, with an outstanding-other-goals count
and action-needed requests first. Opening a historical request still uses its exact saved task,
Sample Test and image. No request is deleted, automatically answered or converted to a new goal.
Loading hides stale request controls until the owned request snapshot arrives. Cancel/retry buttons
show saving state and disable while the existing asynchronous command is pending. No new engine,
authorization, model call or automatic navigation was added.

Regression-first component contract initially failed because the extracted component did not yet
exist; three rendering tests then verified current-vs-other ownership, closed history, explicitly
selected completion and loading. The first real browser run `/tmp/annotagent-guided-e2e-23491`
failed both selected flows: legacy conversation entry can omit URL task while its saved goal has
an existing server task. Using only URL task hid that current goal's requests in history. Fixed
the component input to reuse the existing `referenceTask` resolved by saved goal source-message ID,
not a task name, local active-project preference or arbitrary first pending request.

Corrected `/tmp/annotagent-guided-e2e-23741` passed classification and bbox full sample/correction/
continuation/formal-delivery flows, 2/2 in 22.6 seconds. The bbox scenario adds another independent
saved goal, verifies the first goal's request is not presented as current, opens the native history
with Enter and returns through the saved request to its exact task/image. Existing response-loss
answer, revision repair and formal export checks remain in that scenario. Typecheck, 122 Web units
and production build passed, with the existing chunk warning. A final screenshot/order check is
recorded below. No Rust business logic changed in this increment. All browser data/model transports
were isolated TEST fixtures; no Live model quality or human usability claim, real workspace changes,
push or remote changes. Structured request types beyond the implemented clarification/correction,
bounded continuous coordination and full acceptance matrix remain incomplete.
Final bbox repeat `/tmp/annotagent-guided-e2e-23838` passed 1/1 in 14.5 seconds after history ordering
and screenshot positioning. Inspected `conversational-workspace/request-task-history.png`: the
pending other-goal request appears before cancelled history, with separate, wrapping open/cancel
actions and unchanged TEST image pixels. Keyboard focus remains visible on the disclosure. Web
typecheck and 122 units passed again. This is not a native-zoom or full browser-suite result.

### M4 continuation — delayed request navigation cannot restore an obsolete task

Added a failing real-browser case before changing navigation: click an existing help request,
hold its read-only Sample Operation response, then switch the same document to another goal while
keeping the same image. Baseline `/tmp/annotagent-guided-e2e-24119` failed: the old response added
its request/Draft/Test to the user's newer task-only URL. The existing navigation generation only
changed for image/Draft/Test and therefore missed task-only changes and result modes.

Navigation invalidation now uses the existing canonical workspace route builder over the complete
Project/conversation/task/image/request/Draft/Test/message/processing/result context. Explicit goal
selection and result switches also invalidate pending sample navigation immediately. Both sample
and human-request lookup failures check the same generation before presenting an error. This does
not cancel a running model or mutate a server task; these are read-only navigation lookups, whose
obsolete results/errors are ignored. No new route or alternate task state was introduced.

Corrected classification and bbox browser flows `/tmp/annotagent-guided-e2e-24245` passed 2/2 in
22.7 seconds. Final bbox run `/tmp/annotagent-guided-e2e-24319` passed 1/1 in 14.6 seconds with both
a delayed success and delayed 503 lookup, verifying task-only navigation remains unchanged and
no stale error appears before completing the existing correction/continuation/formal delivery
scenario. Web typecheck, 122 unit tests, production build and diff hygiene passed (existing large
chunk warning remains). No Rust business behavior changed; this is not a new all-workspace or
all-browser-suite claim. Test-only workspace/HTTP fixture, no Live quality or human usability
validation, no real Workspace mutation, no push/remote change. Full coordinator and other recorded
acceptance gaps remain open.

### M3 continuation — durable defer/reopen for unanswered sample requests

Added explicit “Do this later” / “Reopen request” through the existing Rust request service.
Migration 39 stores immutable, versioned deferral commands against the existing request ID.
The answer status stays pending for admission purposes: deferral is an unfinished scheduling
choice, not cancellation, acceptance, an answer or a resume event. No model call or budget grant
is created or reset. Independent tasks retain their existing budgets; this increment does not
relax the existing same-task pending-request spending block or implement independent-image
scheduling inside that task. The API returns `deferred` and `deferral_revision` alongside existing
status, which the GUI distinguishes as “Deferred · not reviewed or completed”.

Stable Project/task/conversation ownership is checked by Application; storage uses expected
revision and immutable command ID in one transaction. Duplicate delivery returns the latest
saved state and cannot re-defer a request after a newer reopen. Changed/stale commands and
deferring an answered request are rejected. Answer validation checks deferral inside the same
feedback transaction, so a late answer cannot write feedback or enqueue continuation while
deferred. Existing source revision/geometry/answer checks still apply after reopening. Requests
can still be cancelled. Deferred current history is explicitly counted as unfinished.

GUI retries retain the original command after uncertain responses. Dirty corrections must first
be saved or undone. The deferred canvas is read-only and reuses `SampleFeedbackEditor`'s persisted
correction projection, not a separate annotation store or an original-prediction substitute.
Reopening restores the normal request editor without running inference. A screenshot inspection
of the first draft caught the original-overlay substitution; the read-only editor reuse fixes it.

Storage contract initially failed to compile before the new deferral API existed. Final storage
suite passed 62 unit plus 16 integration tests, including reopen-after-SQLite-restart, exact retry,
stale replay, foreign owner, denied deferred answer and no resume outbox. Server-target all-feature
Clippy with warnings denied and format check passed. Classification and bbox browser run
`/tmp/annotagent-guided-e2e-24802` passed 2/2 in 35.3 seconds, including lost deferral response,
idempotent retry, refresh, denied late answer, reopening and unchanged task budget before the
existing correction/continuation/formal-export flow. Web typecheck, 123 units and production build
passed (existing chunk warning remains). The additional geometry assertion in
`/tmp/annotagent-guided-e2e-25086` initially compared SVG units to fixed 640px image units; corrected
it to check normalized width against the actual viewBox, matching the stored 0.15 correction.
Final repeat and screenshot inspection follow below. TEST-only transport/workspace, no old keys,
Live quality, real Workspace mutation, push or remote changes; no human usability testing.
This implements deferred sample-correction requests, not every requested HumanRequest kind or
the remaining bounded automatic coordinator and scope-change requirements.
Final `/tmp/annotagent-guided-e2e-25213` passed 1/1 in 15.0 seconds. Inspected
`conversational-workspace/deferred-request.png`: deferred/uncompleted status, read-only canvas and
the saved corrected box are shown. The browser verifies normalized width 0.15 after refresh,
not an assumed SVG pixel scale. The synthetic TEST image and cup result are protocol fixtures,
not representative detection evidence. Diff hygiene passed; unrelated screenshot changes remain
unstaged. No fresh full Rust workspace suite or native browser zoom validation is claimed here.

### M4 regression checkpoint — full workspace after durable task/request restoration

At `c13f92d`, ran the complete Rust chain: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`, and `cargo build --workspace --all-features`.
The chain completed with exit 0. Live-conditional/ignored tests remain excluded: legal externally
supplied YOLOX/PIDNet/RF-DETR/SAM model-process weights and any explicitly ignored paid-provider
smoke are not verified by this result. No total test count is inferred from truncated output.

Full Chromium run `/tmp/annotagent-guided-e2e-25398` exercised 78 scenarios. It exposed shared
TEST-server mutation pacing failures in the classification repair sample, human-classification
Builder and Schema cancellation waits. Retained Playwright network traces show repeated POST 429s
to the corresponding sample-operations, builder-operations and cancel endpoints. The fixture was
still within its existing 65-second bounded pre-execution retry window while UI expectations
expired after 10 seconds. This was not evidence of an admitted model loop or a missing receipt.

Only the two long conversation sample/Schema test files now use 75-second UI expectation windows
and a 180-second overall test limit. Assertions and production behavior remain unchanged: the
fixture still retries only proven pre-execution rate-limit rejections, never provider errors or
unknown execution outcomes; production rate limits, Agent steps and call budgets are not raised.
Cancellation still shares the production mutation limiter; prompt cancellation under saturation
is a product limitation, not solved by increasing a TEST timeout. Final full-browser outcome and
rerun evidence follow below. Unrelated regenerated screenshots are not staged.
First run finished 75/78 (5.3 minutes). The second full run
`/tmp/annotagent-guided-e2e-26029` finished 77/78 (5.2 minutes): all five sample scenarios passed,
but Schema response-loss injection used raw `route.fetch`, bypassing the bounded 429 handling.
It asserted success on a pre-execution rejection. Updated Schema save/Builder and sample response
loss interceptors to use the same bounded helper, and changed held Builder delivery to route
fallback so the fixture can handle the real rejection. This preserves the intended test: only a
confirmed successful server write has its browser response deliberately discarded. A third full
run is required before claiming 78/78; the prior two runs do not prove that result. No production
code changes were made for these test harness corrections.
Final full Chromium run `/tmp/annotagent-guided-e2e-26472` passed **78/78 in 5.3 minutes**,
including all five conversation sample scenarios, Schema cancellation/recovery, management
delete/restore, Review/Export, settings/Provider/plugin protocols, ownership, keyboard, responsive
reflow and security tests. This supersedes the earlier failed runs for the current harness; it
does not erase them or claim they passed. Web typecheck, 123 unit tests and production build also
passed; the existing large JavaScript chunk warning remains. The browser's 200-percent test is
the documented CSS reflow boundary, not a verified native browser zoom operation. Five ignored
Rust tests (paid Builder provider smoke and four externally supplied model-process weights) remain
unexecuted. All evidence is isolated TEST transport/fixtures, not Live inference quality or real
human novice usability. The overall goal remains active with the gaps listed at the top.

### M4 continuation — bounded stop/pause capacity under ordinary mutation saturation

The full-regression investigation identified a real limitation beyond test pacing: ordinary
workspace changes could exhaust both the rate window and concurrent mutation permits used by
stop controls. Added an executable middleware baseline test filling all 120 mutation-window
entries and holding all 16 mutation permits. A valid authenticated Run cancel returned 429
instead of reaching its handler; the test failed before the implementation changed.

`LocalSecurity` now gives an exact allowlist of existing POST pause/cancel routes its own
30/minute window and four concurrent permits. The allowlist covers Run/Batch pause/cancel,
AgentSession cancel, Project Sample Operation cancel, conversation call cancel, clarification
cancel and human-request cancel, with UUID route identities. Start, resume, authorization,
installation, management/deletion and arbitrary `.../cancel` suffixes do not qualify. This is
not an unbounded exemption: exhausting either control limit returns a distinct structured 429.
Host/origin, local session, CSRF, JSON bounds and applicable privileged confirmation checks remain
in the same middleware; Application still validates object ownership and command semantics.
The change admits the stop command under ordinary write saturation; it does not guarantee an
external provider will physically stop an already-issued request or refund its charge.

Three added tests cover saturation survival, unchanged resume rejection, CSRF/cross-origin
denial, strict allowlist matching and independent control rate/concurrency bounds. All 40 server
tests passed with all features, including original security, management and runtime API tests;
server all-target/all-feature Clippy with denied warnings passed after merging identical match
arms, and format/diff checks passed. Browser `/tmp/annotagent-guided-e2e-27135` passed 2/2 in 15.4
seconds: actual Schema/Builder cancellation/recovery and local API security. Saturation itself is
verified in the middleware test, not claimed as a real-user browser load test. No Web behavior or
model budget was relaxed; the prior TEST pacing fixes remain appropriate for ordinary writes.
This supersedes the ordinary-write-starvation limitation noted in the preceding checkpoint,
while finite control-lane saturation and cooperative provider cancellation remain explicit.
No Live model quality/human usability validation, real Workspace changes, old keys, push or remote
changes. The prior 78/78 full-browser result predates this change; no new full-suite claim here.

### M2 continuation — a clear authorized Schema proposal becomes an editable Draft automatically

Baseline application assertion proved the extra product handoff: after a successful authorized
Schema model response, `conversation_schema_for_call` still returned None. The browser required
“Save as editable Schema Draft” before any further work. The first test edit used an incorrect
method name and did not compile; after correcting it to the existing API the behavioral assertion
failed as intended. Clear goal execution now materializes its valid saved proposal through the
existing Application/Storage Schema service before returning. No new engine or store, no Project
YAML update, no publication, no implicit Builder/image call or new grant.

Only an actually completed draft decision qualifies; materialization reparses and validates the
persisted tool response and checks task/source ownership as before. Clarifications, invalid tool
outputs and unknown/cancelled provider attempts are not fake editable Drafts. Exact POST replay
can recover local materialization from its existing receipt without another model call and uses
the existing idempotent source-call identity. It does not reset subsequent human edits. GET,
history, mount and refresh remain read-only. Receipt and Draft are still separate transactions:
if local materialization fails/crashes after saving the receipt, the saved proposal remains
recoverable through the existing explicit local-save action; no automatic provider retry or
crash-atomic two-record guarantee is claimed.

The GUI now goes directly to saved editable labels and the existing Builder authorization entry.
Model consent explains this limited local-save consequence, and Draft loading is distinguished
from an unsaved proposal. The legacy recovery action remains when a saved response genuinely
lacks its Draft. This removes one unnecessary local confirmation, not the still-open full
bounded Schema→Builder→sample coordinator. Later models/data scope still need matching consent.

Five Schema application tests and server-target all-feature/all-target Clippy passed; the
existing test checks initial auto-materialization, one provider request, restart, editing,
immutable snapshots and duplicate receipt preservation, with an added post-replay assertion that
edited labels remain unchanged. Web typecheck, 123 unit tests and production build passed with
the known chunk-size warning. Focused eight-scenario browser run and final screenshot evidence
follow below. TEST fixtures only, no real Workspace mutation, old keys, Live quality, human
usability, push or remote changes.
The combined `/tmp/annotagent-guided-e2e-27524` run passed 7/8: clarification answer/cancel and
all five sample scenarios passed; the remaining Schema API test still asserted the old pre-save
Draft was null. Replaced that obsolete assertion with persisted revision/source-call checks and
exact equality for the legacy save endpoint replay. Corrected Schema + bbox run
`/tmp/annotagent-guided-e2e-27795` passed 2/2 in 28.5 seconds. No broader passing count is inferred.
Inspected `conversational-workspace/automatic-label-draft.png`: editable saved labels are directly
available with no extra local-save button. The screenshot exposed a stale global-sounding status
“No model has been called” after a later child inference. Message/upload/selection statuses now
describe only their own non-inference action rather than the entire task's call history. The
duplicate original-proposal/detail presentation remains a separate compaction gap.

Final bbox + journal browser run `/tmp/annotagent-guided-e2e-27934` passed 2/2 in 15.9
seconds. Re-inspected the regenerated automatic-label-draft screenshot: the bottom status now
correctly describes message saving only; saved revision 1 and label editing are immediately
available. Final Web typecheck and 123 unit tests passed, as did server Clippy and format checks.
These are isolated TEST results, not Live inference quality or human usability evidence.

### M2 continuation — keep original Schema evidence available without duplicating the working labels

The inspected automatic-label-draft screenshot showed the same labels twice: the original
model proposal and the current editable revision. Only completed draft proposals now put their
original output/rationale in a native same-page details disclosure. The editable saved labels,
authorization entry and any real clarification question remain directly visible. This is not a
new modal, route, execution or store; folding history does not hide a question awaiting an answer.
Native summary keeps keyboard activation/focus and uses the established color/font tokens.
The Schema browser test checks collapsed content after refresh, Enter to open/close, restored
summary focus, then continues actual editing/Builder/cancellation. Existing clarification and
five terminal sample scenarios are being rerun in isolation. Web typecheck, 123 unit tests and
production build passed; the existing >500 kB chunk warning remains.

Combined `/tmp/annotagent-guided-e2e-28294` passed 7/8 in 2.4 minutes: both clarification
paths and all five sample scenarios passed. The final Schema test's held cancellation request
used `route.continue`, bypassing TEST-only mutation pacing, and received a pre-execution 429
instead of the expected cancellation 400. Changed that interceptor to `route.fallback`, like
the existing held Builder test; production limits/retry semantics are unchanged. The native
proposal disclosure also explicitly checks no API writes on keyboard open/close. Inspected the
updated automatic-label-draft screenshot: the duplicate proposal is folded, with editable labels
visible. It also exposes a separate legacy message→Schema task synchronization gap: the parent
request panel can still say “Select an annotation goal” after the child creates a task.

Schema-only `/tmp/annotagent-guided-e2e-28518` passed 1/1 in 8.1 seconds. Final combined
rerun `/tmp/annotagent-guided-e2e-28593` passed all 8/8 in 2.4 minutes, including the final
held-request fallback and keyboard/no-write checks. Format/diff checks passed. No full Rust
suite was rerun for this presentation-only follow-up; the preceding Schema Rust tests and
Clippy evidence remain scoped to `54ae0d5`. No Live/real-human validation, user data mutation,
push or remote change. The full coordinator and other gaps listed at the top remain unfinished.

### M1/M2 continuation — synchronize the saved-message task with its parent workspace

At `2ce64f0`, a new bbox browser assertion in `/tmp/annotagent-guided-e2e-28887` failed:
after “Prepare label proposal”, the parent request panel still asked the user to select a goal.
The child had created the durable task, but the parent owned-task/request snapshot predated it.
The newer combined composer action already updates that snapshot; the legacy journal-first
entry did not. On explicit child task creation, reuse its existing assistance-change callback
to invalidate/reload the parent owned snapshot. No new task, navigation, provider call or
mount-time mutation; aborted old parent reads cannot replace the refreshed result.
The same callback runs for manual label definition, before the human/LLM branches diverge.
Added assertions for both paths before inference or label saving, then continue their existing
full sample/correction/formal workflows. This is context synchronization, not a claim that the
bounded multi-stage coordinator is complete.

Fixed bbox + human-bbox `/tmp/annotagent-guided-e2e-29029` passed 2/2 in 26.0 seconds;
Web typecheck, 123 unit tests and production build passed (known chunk-size warning remains).
The regenerated automatic-label-draft screenshot now shows the current goal's request status
alongside its saved labels, not the obsolete selection prompt. TEST only; no Live inference,
real-human usability, real workspace writes, push or remote changes.

Coordinator boundary rechecked after this fix: `conversation_builder.rs` in Server explicitly
authorizes text-only goal/Schema/Registry metadata and excludes sample inference. In
`sample_operations.rs`, `conversation_scope` seals the resolved Draft fingerprint, task,
request and previous grant; `validate_scope` requires the authorized revision/model/image
fingerprint and exact first 1–3-image selection. Thus an automatic front-end click using the
Builder grant would not constitute the requested bounded coordinator. The next integrated
authorization must explicitly cover permitted image recipients/selection and cumulative calls,
then validate the actually generated Draft against that envelope before sample admission.
The existing distinct consent boundaries remain intact until that mechanism is implemented.

### M2 recovery continuation — unknown sample outcomes are not a new test authorization

Extended the real HTTP TEST bbox flow to lose both the successful sample POST response and
the browser's immediate receipt GET (503). The server independently completes its original
operation. Before refresh, assert the pending session envelope equals the submitted request;
after refresh, recover its saved report, clear the local pending envelope and assert zero API
writes. Initial recovery-only `/tmp/annotagent-guided-e2e-29370` passed 1/1 in 14.0 seconds.
Adding an assertion that the ordinary “Test these samples” action is disabled exposed an actual
UI failure at `/tmp/annotagent-guided-e2e-29439`: that button remained enabled next to the exact
retry action. Its handler already reused the frozen envelope, so this does not demonstrate a
duplicate charged call, but the action semantics were misleading.

Disable ordinary start during unknown outcome, and display a scoped recovery explanation:
the server may already be running, no automatic retry, refresh reads saved state, explicit
retry retains request ID/images/revision/authorization rather than renewing permission.
Explicit retry also waits for initial history loading to finish. This reuses existing server
receipts and idempotency; no new executor or grant. Pending pre-acknowledgement browser state
still uses sessionStorage, not a cross-device server pending-envelope guarantee.

Fixed classification + bbox `/tmp/annotagent-guided-e2e-29607` passed 2/2 in 22.1 seconds.
Final bbox `/tmp/annotagent-guided-e2e-29712` passed 1/1 in 14.1 seconds, adding a held
history GET after reload and verifying retry remains disabled until that read settles.
Captured and inspected `conversational-workspace/sample-outcome-unknown.png` for the honest
unknown-state explanation and disabled ordinary start. Web typecheck, 123 unit tests and
production build passed, with the known chunk warning. Diff check passed. No Rust behavior
changed; no new full-Rust regression claim. No Live model quality/human usability validation,
real Workspace mutations, old keys, push or remote changes. Main coordinator scope remains open.

Screenshot correction: the initial whole-sample-region capture visibly included an occluding
fixed header and clipped leading content. It was not a clean full-panel evidence image.
Narrowed this asset to the complete unknown-outcome notice itself; the disabled ordinary action
is proven by the browser assertion, not by that cropped notice. No UI behavior changed.
Final capture rerun `/tmp/annotagent-guided-e2e-29840` passed 1/1 in 14.0 seconds; the
notice is fully visible without the fixed-header occlusion.

### M2 coordinator foundation — immutable combined consent and concrete sample sealing

Started the missing Builder→sample authorization mechanism, not another presentation cleanup.
Migration 40 introduces a task-owned immutable consent envelope: exact Builder scope/operation,
Schema identity/revision/digest, ordered image IDs/content hashes, allowed model binding digests,
bounded phase call ceilings, explicit unknown-price acceptance, expiry and fixed sample operation.
The eventual Application adapter must derive these hashes from actual Registry/destination and
permission snapshots, not trust LLM or client assertions. That adapter and HTTP/UI execution
integration are NOT implemented by this foundation and remain required work.

The stored sample seal fixes the actually generated Draft/revision/fingerprint only if it matches
the original Schema and image selection and uses a subset of permitted exact model bindings within
the sample call bound. One consent seals at most one such continuation. Changing a destination's
binding digest, image bytes, Schema, operation identity, call bound or sealed Draft is rejected.
Explicit revocation is durable and idempotent; replaying the original save does not un-revoke it
or extend its lifetime. Read/retry preserves receipts after expiry but is not permission to execute.
All reads/writes check stable Project→Conversation→Task ownership. No keys are stored in the new
record. No model/annotation/Project YAML mutations or independent inference ledger are introduced.
Existing task and Project call admission, pending HumanRequest gates, current Registry/data
validation, cancellation and immutable Sample Operation admission remain mandatory at execution.

Tests cover fresh/invalid/duplicate/changed envelopes, expiry/unknown-cost/limit validation,
restart preservation, exact sample retry, out-of-scope rejection, ownership and revocation. The
initial 3 tests passed; all 65 storage unit + 16 integration tests and all-target/all-feature
Clippy passed. A separate-connection concurrent conflicting-Draft seal test was then added.
This is not yet an executable combined journey, nor an authorization UI/product claim. It is
deliberately not exposed as a fake ready action. No new browser screenshot is relevant until
integration; existing actual-page evidence remains scoped to its prior commits.

Hardened the foundation before commit: Schema lookup now verifies an actual persisted definition
owned by the task at the exact revision and compares its SHA-256, with checked SQL integer
conversion (the first direct u64 SQL parameter did not compile and was corrected). Builder and
sample operation IDs are unique in consent storage, preventing a new consent ID from silently
rebinding the same execution identities. A two-connection race admits only one concrete Draft
seal; revoking that saved seal also rejects late continuation. Final storage counts are 66 unit
and 16 integration tests. Server-target all-feature/all-target Clippy additionally compiles the
Application/Server dependency chain. This migration was exercised only in fresh temporary test
databases, not the user's running workspace. Live and human tests remain unexecuted.

### M2 coordinator integration — derive and revalidate actual Application snapshots

Added Application services over the stored consent. The data scope reads this task's current
saved Schema and the existing bounded sampler's first 1–3 live images with stable IDs and fresh
content hashes. Explicit model selections resolve to actual frozen Registry Model Profiles or
installed native model snapshots. Remote binding hashes include frozen model semantics plus
destination/routing/connection policy and credential reference (never credential bytes); ordinary
health-check timestamps do not invalidate consent. Native selections reuse publication's exact
Plugin/Model Instance readiness/package/asset checks, with declared Plugin permissions included
in the scope and display data. This does not install anything or grant Plugin permissions.

`seal_conversation_journey_draft` now checks that the proposed Draft is this consent's actually
completed Builder result, with unchanged revision/content hash, before deriving its current
image/model/Schema scope and using the atomic storage seal. Builder receipts now persist those
exact Draft revision/hash fields. Old receipts lacking the identity cannot silently qualify for
automatic execution; their existing manual sample path is unchanged. Runtime static checks,
supported-binding checks, server-computed sample fingerprint, call admission and scope seals
still belong to the existing execution path and are required before a sample can start.

The new test uses an isolated no-network TEST Registry and synthetic image. It proves actual
snapshot resolution, missing/duplicate/foreign bindings rejection, endpoint/availability/image
bytes/Schema-revision invalidation and stable health timestamps. A scope-only Draft/Builder receipt
fixture checks successful sealing and rejection after editing; it is explicitly not inference or
quality evidence. Initial disabled-Provider test data violated the existing Disabled-health rule;
corrected that fixture. A later test used nonexistent `Settings::default`; corrected to the
existing embedded-default loader, without reading real user config or credentials.

Application all-feature tests passed 101 unit + 1 integration, with the paid Provider smoke
explicitly ignored. The added final scope-seal test and Application all-target/all-feature Clippy
passed. Remote and native publication/runtime regressions were included in the Application suite,
but no real weights/Live inference claim is made. Browser classification/bbox paths now assert the
real HTTP TEST Builder receipt matches its saved Draft revision/hash before any sample execution.
HTTP joint-consent routes, coordinator dispatch/recovery and GUI consent are still not connected;
these Application methods do not yet replace the manual phase cards. Full objective remains open.

Browser `/tmp/annotagent-guided-e2e-31402` passed classification + bbox 2/2 in 34.6 seconds,
including actual Builder revision/hash receipts, normal sample execution and unknown-response
recovery. Its production Web build passed with the existing chunk-size warning. No visual layout
changed in this layer, so regenerated unrelated screenshots are not part of the implementation
commit. No real workspace, old keys, Live/human tests, push or remote changes.

### M2 coordinator integration — HTTP preview, consent persistence, read and revocation

Added task-scoped journey-preview and journey-consent routes to the existing workspace router.
Preview derives the real Builder scope and Application image/model/Schema snapshots; it reports
unknown cost as null and returns an unaccepted envelope. POST requires explicit unknown-cost
acceptance, validates exact planning model/prior grant/call bound and rechecks current data scope
before saving. GET and exact POST replay return saved records without re-resolving a now-changed
Provider or dispatching any model. Revocation persists a one-way permission withdrawal, not a
claim that an already-issued remote request was physically cancelled.

The envelope now also preserves exact planner Model Profile ID and previous grant ID so future
dispatch/recovery need not reconstruct them from today's defaults. Optional decoding retains
historical readability; a fresh consent cannot omit the planner identity. This does not add a
call allowance, renew expiry, publish, accept annotations, or start a Sample Operation. Runtime
dispatch/stop propagation/restart recovery and the unified GUI action remain to be implemented;
the consent-only API is not presented as an already-working automatic pipeline.

Isolated browser `/tmp/annotagent-guided-e2e-31872` passed 1/1 in 32.8 seconds. Added checks
exercise honest unknown-price preview, missing acceptance, tampered binding hash, exact save/read
replay, changed-expiry rejection, foreign task denial and irreversible revoke/replay. Call lists
remain byte-for-byte equal and no Builder operation exists after these consent-only requests.
The ordinary explicit bbox pipeline still completes afterwards. Final
`/tmp/annotagent-guided-e2e-32058` passed 1/1 in 28.3 seconds, also verifying missing CSRF returns
403 and a smuggled auto_publish field returns 422. Existing protections were not bypassed.
40 Server tests, 66 storage unit + 16 storage integration tests, the focused Application scope
test, all-target/all-feature Server Clippy and format/diff checks passed. Browser production build
retains the known chunk warning. Cargo lock waits completed normally; no service was restarted
because of a lock/observation timeout. No real workspace migration, old keys, Live/human tests,
push or remote changes. No new UI screenshot claim for this API-only stage.

### M2 coordinator integration — explicit bounded Builder-to-sample execution

Added GET/POST journey execution on the existing task-scoped consent resource. GET only
reads the owned consent and child receipts. Explicit POST uses the original Builder grant,
Schema revision and model selection, then seals the actual completed Builder Draft revision/hash
and reuses the existing Sample Operation service. Images and actual model bindings must fit the
saved joint scope; no fresh allowance is inferred from a retry. Unknown/interrupted Builder
receipts are returned rather than silently restarting inference. No publication or formal
annotation acceptance is included.

Sample requests optionally carry the joint consent identity. Admission verifies original grant,
expiry, sample ID and sealed Draft; sample scope checks reject revoked/expired joint consent.
Revocation uses the existing durable Builder cancellation and Sample Operation cancellation
paths, with sample task/conversation ownership verified before cancellation. It is admitted
through the bounded stop lane, not the inference lane. In-flight remote requests may still bill.
Already-completed samples remain saved and repeat execution reads their receipt without work.

Evidence: isolated `/tmp/annotagent-guided-e2e-32939` passed the new joint classification
scenario (1/1, 19.3 s including build). Final `/tmp/annotagent-guided-e2e-33299` passed 1/1
in 5.5 s with simultaneous execution POSTs, one Builder receipt, one fixed sample ID, successful
sample completion, stable call history on replay, GET without calls, CSRF/extra-field rejection,
and preservation of successful results after revoke. `/tmp/annotagent-guided-e2e-33085` passed
the original bbox workflow (1/1, 40.8 s), including rejection of execution after consent revoke
and the existing lost-response recovery/edit/review/export path. All use explicit TEST HTTP
transports and synthetic input; no Live quality or real-user usability evidence is implied.

Server 40 tests, focused Application scope test (1), Storage journey tests (4), Server
all-target/all-feature Clippy passed. Initial Clippy caught an oversized inline Builder future;
boxing the reused handler future resolved it without suppressing the lint. Production Web
build passed with the existing chunk-size warning. This layer has no new visual UI to screenshot.
Unrelated regenerated screenshots remain unstaged.

Remaining: POST currently awaits the existing Builder handler before background sample dispatch.
This is NOT yet a durable background coordinator: a lost Builder handler can leave an interrupted
receipt, and there is no automatic restart after process loss. The default GUI still uses phase
cards; unified consent presentation and background dispatch/recovery are subsequent work.
No real workspace migration/restart, old keys, push or remote changes. Goal remains incomplete.

### M2 coordinator integration — background dispatch and durable interruption receipts

Journey execution POST now claims a durable dispatch attempt and returns immediately, while a
bounded server worker advances the existing Builder/sample chain independently of page lifetime.
Migration 41 adds a consent-owned dispatch row, running/settled/interrupted state, attempt identity
and error. Claim is atomic; a second concurrent POST reads the same running work. Worker settlement
uses attempt CAS, so a stale worker cannot finish a newer attempt. Eight background planning
workers are allowed per server; capacity rejection occurs before dispatch admission. Existing
HTTP security, Project/task call ledgers and child-operation checks remain in force.

Startup marks orphaned dispatches interrupted and preserves child receipts; it does not issue
model requests. Explicit retry uses the original consent and child operation identities. A
completed Builder can continue to its unstarted sample after checking the saved scope, but a
Builder with uncertain/interrupted model execution is not automatically replayed. This is safe
interruption reporting, not a claim of resumable arbitrary LLM token streams. Sample workers
continue using their existing durable receipt/restart behavior. Exceptions settle a visible
dispatch error; no automatic retry is introduced.

Storage regression uses independent database connections to verify exclusive claims, foreign
ownership rejection, restart marking, a subsequent explicit claim, stale-worker CAS protection,
persisted failure and revoked-consent rejection. All 67 Storage unit + 16 integration tests and
40 Server tests passed. All-target/all-feature Server and E2E-fixture Clippy, format/diff checks
passed. Initial compile used the wrong futures crate path; corrected to the already-installed
futures dependency without adding a package.

Isolated `/tmp/annotagent-guided-e2e-33899` passed 1/1 (30.9 s including compile) with a named
slow TEST planner: execution returns running with no sample yet, page navigates to Projects,
then the original Builder and sample complete. Final `/tmp/annotagent-guided-e2e-34025` passed
2/2 in 13.1 s, including simultaneous POSTs/replay and revocation while Builder is reserved:
the latter settles with no sample and further execution is denied without additional calls.
The earlier fast-fixture pass in `/tmp/annotagent-guided-e2e-33775` was superseded by this timing
evidence. Production Web build passed with its known chunk-size warning.

No new UI has been connected in this stage, so there is no new visual screenshot claim. Next:
unified consent/action/status presentation, saved journey discovery, and frontend recovery without
mount-triggered execution. Full Schema-to-results default experience, broader HumanRequest types,
Live quality, native zoom/a11y matrix and real-human usability remain incomplete. No real
Workspace restart/migration, old keys, push or remote modifications; unrelated screenshots kept.

### M2 default interface — one saved-label build/sample authorization and recovery

The default saved-label card now offers one build-and-sample action. It enumerates available
image Model Profiles (excluding Mock Providers) and selectable installed model instances,
allows an explicit selection change, and obtains the server-derived preview before acceptance.
The preview lists planner/image recipients, sample count, call bounds, expiry, unknown cost,
Project budget and expandable permission evidence. Changed selections discard the old preview.
Starting saves exact consent before the existing background POST. No new executor, automatic
publication or annotation acceptance is introduced.

Task-owned GET history restores the last saved journey and child receipts. This is currently
a bounded latest-50 history, not a completed pagination/performance implementation. Unknown
save/execute outcomes retain the original envelope in session storage until a server receipt
is found. Restoring a saved but undispatched consent shows an explicit Continue action; mount
never executes it. Running work exposes Stop and explains possible in-flight billing. Saved
results open the existing sample canvas and use the existing URL/feedback/Review boundaries.
Assistance delivery status is included and polled so review-request preparation is not lost
when sample inference finishes first. Failure remains visible; no automatic inference retry.

The legacy phase-by-phase Builder is a secondary visible action. Historical non-journey
operations restore that existing UI so their stop/results controls are not stranded. Repair
flows remain unchanged. Completed default journeys prioritize viewing results; creating a
different plan is secondary and requires fresh authorization. Settings errors link through the
existing validated conversation return context, not an arbitrary return URL.

Browser evidence: initial `/tmp/annotagent-guided-e2e-34965` passed the default UI 1/1 in
13.8 s. `/tmp/annotagent-guided-e2e-35241` passed normal + lost-save-ack UI 2/2 in 27.9 s:
the server had saved consent, the browser lost its response, reload found exactly one consent
without model calls, and explicit Continue used that same request. Final
`/tmp/annotagent-guided-e2e-35484` passed 4/4 in 53.3 s: both UI scenarios, simultaneous
joint execution and the existing bbox edit/review/export workflow. UI tests refresh during
the slow TEST Builder, retain Stop, open results, refresh the exact result URL and record zero
new execution POSTs from reload/navigation. Native/Live models are not exercised by this test.

Screenshots inspected: `conversational-workspace/joint-consent.png`, `joint-result.png`,
`joint-result-390.png`. Initial card screenshot was occluded by the sticky header, so the
final authorization screenshot captures the complete consent region. The initial mobile capture
showed the Conversation tab, so the final test explicitly opens Images and captures the actual
classification canvas. These use a synthetic image and scripted TEST classification, not quality
evidence or a claim that a synthetic football field is indoors. Existing CSS/fonts/tokens reused;
checkbox spacing and flexible wrapping added for model choices. No unrelated screenshots staged.

123 Web unit tests, Web typecheck/production build, Server 40 tests, focused Storage journey
tests (5), final Server all-target/all-feature Clippy and format/diff checks passed. Existing
production chunk warning remains. Top-level checkpoint updated without declaring completion.
Still needed: initial Schema/goal coordination under one suitable consent, advanced HumanRequest
types and scoped message interpretation, comprehensive multi-history/context and a11y audit,
full final regression, Live quality and real-human usability. No push, remote changes, user data
mutation, real workspace restart or old API keys.

### M2 initial-goal integration prerequisite — Schema execution survives client disconnect

Rechecked the task's initial-goal/Schema and authorization sections. Full first-message-to-sample
consent still needs to bind an as-yet-unknown Schema safely; it must not reuse a text-only Schema
permission as permission to transmit images. Before that integration, fixed a lifecycle mismatch:
the Schema UI said leaving does not cancel, while its server handler directly owned the inference
future. The handler now awaits a separately spawned, bounded worker. Dropping its HTTP wait
detaches the worker rather than dropping an admitted model call. Existing Application call
receipts, cancellation tokens, Project/task budget, expiry, automatic Draft materialization and
startup unknown-outcome recovery remain authoritative. The shared eight-worker planning bound
is acquired before dispatch; capacity rejection preserves the original saved authorization.
No new Schema executor, migration, retry policy or expanded image permission was added.

New isolated browser test uses a named slow TEST Schema model and an actual browser fetch with
AbortController (not merely dropping an already-completed response). It waits for the model-call
receipt to become reserved, aborts the client, navigates away, then verifies one completed call,
the automatically saved classification Schema revision and restoration in the original task.
A second task explicitly cancels the worker while the request is in flight and verifies there
is no Schema Draft and no extra call from returning to the task.

Initial `/tmp/annotagent-guided-e2e-38632` failed because the TEST expected cancellation to mean
`failed`. Inspection of the saved receipt and existing Application contract showed `in_doubt`
is correct once the Provider may have received the request: remote completion and cost are
unknown. Corrected the test, not production evidence semantics. Final
`/tmp/annotagent-guided-e2e-38877` passed the new lifecycle case and existing Schema authorization,
budget/retry/ownership/Draft test (2/2 in 15.6 s). Server 40 tests, focused Application Schema
tests (5), Server/E2E-fixture all-target/all-feature Clippy, format/diff checks passed. Web
production build passed with the existing chunk-size warning. No changed layout or new screenshot
claim; all transport is explicit TEST, not Live quality evidence.

Initial Schema-to-Builder/sample consent integration remains outstanding, as do the other
incomplete objective items. No real workspace restart/data changes, old keys, push or remote
changes. The lifecycle correction is a prerequisite, not a redefinition of the full goal.

### M2 first-goal service chain — immutable authorization before Schema exists

Extended the existing journey envelope with an optional, explicit text-only Schema proposal.
For this source only, nil Schema ID/revision zero denote unresolved output; no placeholder Schema
is created. The envelope freezes the original Project goal revision, planner scope, one Schema
call ID, Builder/sample IDs, image digests, exact allowed model bindings, limits and expiry.
Both unknown-cost acceptances must be explicit. Existing saved-Schema envelopes remain readable.

Migration 42 stores one immutable resolved consent after the actual authorized Schema call
produces its valid first Draft. Original consent is never rewritten. Resolution verifies task,
source call, completed draft decision, Schema digest/revision, unchanged images/model set/call
limits/expiry, and original goal/data state after planning. Another Schema, a later human edit,
or a different resolved Builder scope cannot replace that seal. This is authorization evidence,
not another Schema/Workflow implementation or extra budget grant.

The existing bounded background journey now invokes the same Schema service and then Builder
and Sample Operation. Nested Schema execution reuses the already-owned worker permit rather
than attempting a second admission. Clarification, invalid output and in-doubt receipts return
saved state without Builder/image execution. Revoking an initial journey also cancels its Schema
call. No automatic model retry, publication or formal annotation acceptance. Clarification still
uses the existing answer UI/service; automatic continuation after an answer is not yet claimed.

Initial `/tmp/annotagent-guided-e2e-39528` passed classification and clarification, but bbox was
correctly blocked by the sample scope guard: the Builder had selected an earlier TEST project's
model. Inspected its persisted Draft binding to confirm the mismatch. Fixed the planning context,
not the permission check: journey Builders now receive only authorized Model Profiles, matching
expert manifests/vision descriptors and Provider summaries. Existing execution-time sealing is
retained as a second check. The test intentionally keeps earlier TEST models registered to cover
that regression. `/tmp/annotagent-guided-e2e-39917` then passed all three scenarios.

Final `/tmp/annotagent-guided-e2e-40408` passed 5/5 in 30.9 s: initial classification, bbox,
clarification, invalid-empty-label Schema, plus the existing saved-label joint UI. Each initial
test checks original consent equality, one Schema receipt, real resolved revision and report,
or absence of Builder/sample for blocked Schema outcomes; explicit replay does not add calls.
Storage resolution tests cover foreign ownership, unrelated human Schema rejection, changed
limits, immutable replay/replacement, restart persistence and unchanged call count. Six focused
Storage journey tests passed. Server 40 tests, Application 101 unit + 1 integration tests passed
(paid Provider smoke remains explicitly ignored), final Server/E2E-fixture Clippy and format/diff
checks passed. Production Web build retains the known chunk warning.

This stage exposes the service path, not the new first-goal UI. The current default frontend
still begins joint execution after saved labels; it needs initial-envelope display, Schema
progress/clarification handoff and resolved-revision restoration. No new UI screenshot claim here;
existing UI scenario ran as regression and its regenerated screenshots remain unstaged. No Live
quality/human-usability claim, old keys, user Workspace migration/restart, push or remote changes.

### M2 first-goal UI — one explicit authorization through saved sample results

The existing Journey card now accepts an initial goal as well as a saved Schema. The default
composer prepares an initial envelope with one Schema call plus the bounded Builder/sample
calls. The acceptance includes both unknown-cost acknowledgements and freezes the original
envelope before execution. Preview and restoration remain read-only. The UI shows Schema
progress with direct Stop, hands a completed valid proposal to the existing editable Schema
card, and restores the resolved revision and existing sample result rather than asking for a
second phase consent. Manual labels and explicit phase-by-phase execution remain available.

Clarification and invalid output stop before image inference. A saved clarification answer
returns to its actual human Schema; the unresolved initial record is not presented as a plan
that can be resumed against unrelated labels. Same-envelope continuation after answering is
still incomplete: the user currently reviews a new bounded request. Existing data/history are
retained; this filter does not delete the original authorization. No automatic publish or formal
annotation acceptance was added.

Isolated `/tmp/annotagent-guided-e2e-42100`: all six initial-journey cases passed (13.6 s),
including real browser goal submission, initial authorization, reload during slow Schema,
classification results in the existing canvas, clarification answer save/reload, backend bbox,
and invalid Schema. Refresh/navigation adds no execution POST; original envelope stays equal,
one Schema call is recorded, and blocked outcomes create no Builder/sample. Typecheck and all
123 Web unit tests passed; production build passed with the known chunk-size warning.
Format and diff checks passed. This frontend stage does not claim a new full Rust regression.

Reviewed screenshots: `initial-goal-consent.png` (bounded scope and unknown cost),
`initial-goal-result.png` (existing terminal classification canvas), and
`initial-goal-clarification.png` (actual clarification controls, cropped to that panel).
The full-page screenshot originally clipped the scrollable clarification panel; the panel
capture replaces it. TEST synthetic images/HTTP fixture outputs are not Live quality evidence.

The first legacy regression run `/tmp/annotagent-guided-e2e-42189` passed four of six cases.
One test still expected Schema-only authorization from the now-joint default composer; it now
explicitly selects the retained phase-by-phase action. The second failure exposed a real
restoration defect: a pre-admission cancelled Schema with no call receipt looked like an unused
goal. The initial-mode check now includes saved cancellation records, preserving the cancelled
screen after refresh. Final rerun results are recorded below.

Final `/tmp/annotagent-guided-e2e-42349`: 6/6 passed (1.1 min), covering clarification
answer/cancel, both no-LLM manual label types, aborted-client Schema execution/explicit stop,
and the existing complete Schema authorization/budget/recovery regression. The latter verifies
cancelled status after refresh and absence of a new proposal action. No user Workspace access,
new inference authorization, push or remote changes. Other regenerated historical screenshots
remain unstaged. Goal remains active; this is an initial-goal UI milestone, not final acceptance.

### M2 clarification continuation — same bounded consent, linked human semantics

New initial envelopes explicitly opt into `continue_after_clarification`; deserializing old
envelopes defaults to false and omits that field on serialization. The consent UI explains that
saving a clarification answer may continue only within the frozen images/models, original
call limits and expiry. No budget renewal, model substitution or second Schema model call.

Human answer submission can carry that exact journey ID. The server checks its owned task and
original clarification call, persists the existing human Schema/answer first, and only then
requests background continuation. A failed continuation admission returns a saved Schema plus
a distinct continuation error; it does not falsely report a failed label save. Unknown response
retry keeps the same human request, journey, Builder and sample IDs. The old answer-only API and
phase-by-phase flow remain supported; they do not implicitly gain new authorization.

Resolution accepts only revision one of the human Schema linked to the authorized completed
clarification, not another Schema in the same task or an edited answer. The original goal,
Registry binding snapshots, image bytes, scope hash and expiry are rechecked before inference.
Scope changes remain blocked after the answer is safely saved. GET, refresh and restart do not
start continuation. Schema-only consent from an older envelope cannot queue the new operation.

Migration 43 records an explicit answer-continuation intent. This is needed for the narrow race
where the answer arrives while the original worker is settling its clarification result. Worker
settlement checks the intent and unresolved Schema seal in one transaction: either it retains
the same bounded worker, or the answer handler sees a settled dispatch and claims the next
attempt. Stale workers cannot settle a new attempt. No polling loop repeats a failed/unknown
model call: errors settle normally, resolved Schema ends this continuation check, and all child
IDs still use the existing idempotent receipts. Restart marks dispatch interrupted and does not
consume queued intent without an explicit action. Answer routes retain expensive-action
admission, CSRF/ownership and concurrency limits; they cannot use the Stop control bypass.

Tests and evidence:

- Storage resolution coverage: explicit opt-in, unchanged legacy serialization, wrong same-task
  Schema rejection, exact answer linkage, edited revision rejection, revoked permission,
  immutable replay, and the answer/settlement race including a stale attempt. All seven focused
  journey tests passed; subsequent full Storage tests passed (69 unit + 16 integration).
- Application, Storage and Server all-feature tests passed; the existing paid Provider smoke
  remains ignored. Final Server run passed 41 tests, including new expensive-route coverage.
  Server all-target/all-feature Clippy and final format/diff checks passed. Initial Clippy
  reported the serde reference predicate and redundant `continue`; both were corrected.
- Web typecheck, 123 unit tests and production build passed; known chunk-size warning remains.
- `/tmp/annotagent-guided-e2e-43240`: 9/9 initial-journey cases, including automatic linked
  continuation, legacy denial and a disabled model that preserves labels but blocks inference.
- `/tmp/annotagent-guided-e2e-44069`: 11/11 (1.8 min), including old answer/cancel flows and a
  browser-aborted saved-answer response followed by exact retry. No additional journey POST
  comes from refresh/navigation, consent remains byte-equivalent JSON, and there is one Schema
  receipt. Same-answer replay adds no model receipts.
- `/tmp/annotagent-guided-e2e-44666`: final UI clarification rerun passed after screenshot framing
  adjustment. `initial-clarification-continuation.png` shows the real saved-scope explanation and
  answer controls; `initial-clarification-result.png` shows the existing terminal result canvas.
  The answer-panel capture uses a 1280×1100 viewport to avoid sticky-header occlusion; result
  uses 1280×800. This is not native 200% zoom or novice-usability evidence.

Only isolated TEST HTTP models and synthetic data were used. No user data changes, old API keys,
real inference, real Workspace restart/migration, push or remote edits. No claim of Live model
quality or human usability. Scoped conversational-feedback interpretation and the remaining
objective items are still incomplete; goal stays active.

### M3 candidate-message interpretation — bounded Rust contract and existing human requests

Re-read the active attachment and current code; previous goal turn was progress (`cb0e3ad`).
Read-only parallel audit confirmed exact message references and Sandbox correction/repair
services, and identified the existing pending-human spending block. This stage deliberately
preserves that block, including Deferred requests; an interpreter may not cancel or answer a
pending request to obtain another model call.

Added a single-call, text-only feedback proposal contract. It accepts only a correction question
(`poor_boundary`, `wrong_label`, `wrong_target`) or a scope-clarification question. No arbitrary
tools, coordinates, IDs, label patches or human-verification flags are accepted. Boundary intent
requires a bbox subject; unsupported types require clarification. Ambiguous removal is instructed
to ask about scope, not delete a candidate or project class. Prompt and strict JSON validation
are complementary: tests prove protocol/scope behavior, not a real model's semantic reliability.

The Application resolves the exact persisted message, task, sample revision, image hash,
candidate plus Artifact pair and image-level feedback sequence. It sends bounded structured
terminal evidence, not image pixels. It rejects missing references, foreign owners, changed
pixels or a changed feedback sequence before spending. Historical receipt reads can succeed
after pixels change, but new human work revalidates the live subject. Two identical outcome IDs
on different Artifacts cannot select the wrong input; request creation rejects that ambiguous
outcome ID because the existing human-answer API addresses outcomes rather than Artifact pairs.

Reuse of `ConversationTaskProvider` adds a fixed feedback call ID and frozen context evidence.
The existing task/Project grant, expiry, cancellation, pending-human and call accounting checks
remain authoritative. Raw response/usage and invalid output are saved; transport uncertainty
stays consumed and is never automatically retried. Duplicate admission cannot dispatch again.
Both initial and post-admission receipt recovery compare frozen contexts, including concurrent
conflicts. Cancellation registration belongs only to the admitted call. Successful cleanup
unregisters without cancelling the caller token; abandoned work cancels and stays indeterminate.

An explicit Application command prepares/reuses the existing human correction request; it never
submits feedback, creates a repair Draft, accepts annotations or starts inference. Predictable
request IDs require exact frozen-input equality on retry. A new exclusive storage admission
checks pending work and the feedback sequence in the same transaction, so concurrent proposals
cannot leave two pending requests at the same image revision. The legacy general creation API
keeps its behavior; this new path uses the explicit exclusive policy and shared subject validator.

Review-driven fixes before commit included cancellation being accidentally converted back into
a valid proposal, duplicate candidate IDs selecting the wrong Artifact, a concurrent loser
reading another request's receipt, predictable-ID conflicts, normal cleanup cancelling a caller
token, and the pending-request creation race. Regression tests cover these cases rather than
only the successful message path. Initial Clippy failures were documentation formatting and a
single-match branch; corrected before final verification.

Verification:

- `cargo test -p annotagent-application conversation_feedback --all-features`: 18/18 passed,
  including pure proposal validation and temp-workspace Application/SQLite tests. They exercise
  explicit grants, no-authority/no-reference rejection, changed feedback/pixels, exact replay
  after restart, overlapping execution, pending/deferred work, request identity conflicts,
  concurrent request creation, cancellation and preservation of the original sample/feedback.
- `cargo test -p annotagent-application -p annotagent-storage -p annotagent-server --all-features`:
  passed, including existing correction, authorization, geometry, management, export and
  transport tests. The pre-existing paid Provider smoke remains ignored. Storage had 69 unit
  and 16 integration tests; Server had 41 tests.
- Server all-target/all-feature Clippy passed; final format/diff checks passed.
- No frontend changes or browser screenshot claim in this service-only stage. No Web check
  rerun is represented as evidence for a new UI. All new model calls are explicit in-process
  TEST Providers against disposable data, not Live or human-usability evidence.

Next required integration: owned HTTP authorization and durable pending-call context, background
execution/recovery, a message-linked UI card and structured scope-answer handling. Existing
pending human work needs a clear direct route in that UI, not an unannounced spending exemption.
No user Workspace access/restart/migration, old credentials, push or remote edits. Unrelated
generated screenshots remain unstaged. Full goal remains active.

### M3 candidate feedback — durable authorization, chat card and correction-canvas handoff

Continued from `18089c8`; this is another bounded integration checkpoint, not full-goal acceptance.
Added owned feedback preview, consent, receipt, execution and human-request routes to the existing
Project/Conversation/Task API. A preview is read-only. Explicit confirmation saves the exact message,
terminal candidate/Artifact, image hash, sample/Draft revision, image feedback sequence, model binding,
scope digest, expiry and original destination summary before any Provider request is sent.

Migration 44 adds an immutable feedback-authorization record, not a second executor or annotation
store. Its insert and the existing initial/next-phase cumulative grant update share one transaction.
Each saved message can have only one feedback call identity, including concurrent tabs. Changed
retries, superseded grants, changed pixels/feedback, pending or deferred human work, cancelled calls,
invalid owners, exhausted Project/task limits and colliding operation IDs fail closed. Historical
reads/replayed acknowledgement restore original metadata without renewing expiry or rereading a
changed Registry profile. The old call admission and accounting remain authoritative at dispatch.

The HTTP action uses the existing bounded background-worker pool and fixed-ID feedback service.
An admitted request continues when its browser response is aborted. An existing receipt is read
before model/credential/live-data checks on execute retry; an unknown remote outcome cannot be sent
again. Generic startup `in_doubt` receipts are paired with the separately saved feedback authorization,
so the selected subject and cost uncertainty survive even when no model response was recovered.
An authorization without a receipt means no call is recorded: explicit same-ID continuation is
available, but reload never starts it. There is no new background auto-retry or restart executor.
Execution retains expensive-action admission; Stop uses the existing narrow cancellation lane.

The chat message now exposes one compact feedback card. It can review the actual one-text-call
destination/unknown-price scope, save authorization, continue that same request, stop and recover
saved results. The text model receives terminal metadata only, not pixels, and the card says so.
While an execute acknowledgement is pending, GET polling distinguishes checking admission from a
real reserved call. Late status responses cannot erase a terminal receipt or cancellation. Saving
the message, mounting, reloading, selecting an image and reopening the card do not call a model.

For a correction proposal, an explicit canvas action prepares/reuses the existing Human Request;
it does not apply an answer, change geometry, create a repair Draft or accept formal annotations.
The existing canvas answer/repair services are unchanged. A Pending or Deferred request is offered
directly instead of silently cancelled or bypassed to fund another interpretation. Opening that
request restores its real image and outcome. New model results do not navigate or steal canvas focus.
An ambiguous scope response is displayed as a question without deletion or Schema changes; a typed
scope-answer transaction and future-rule patch are still required in the next integration stage.

Review and test-driven corrections in this stage:

- A feedback authorization with no receipt was initially absent from the old cancellation owner
  check. Added ownership validation so a different task cannot pre-cancel that saved call.
- Original model/destination display is persisted; old permission is not described using today's
  possibly changed model settings. Consent replay also preserves its original expiry.
- Added a semantic authorization region and a real submitting/admission state; no false running
  percentage or success state. Stop remains directly reachable while the POST is unresolved.
- Fixed a Storage test's identical scope fixtures so the historical-grant regression actually
  exercises different scopes. Fixed all-workspace Clippy findings, including a lexical lock scope
  in prior feedback tests (rather than retaining a guard across an await).
- Initial browser run: 7/9 passed, then fixed the missing authorization landmark and separated
  coordinator completion, sample completion and human-request delivery in the test helper.
- Initial combined legacy regression: 18/21 passed. SQLite evidence in the TEST workspace showed
  old initial-journey TEST model profiles were chosen by later phase-by-phase fixtures. The test
  setup now isolates enabled `e2e-conversation-*` profiles only at the exact local TEST Provider.
  Production selection and the exact-model assertions were not weakened.
- A subsequent combined run passed those model assertions but exposed a test-transport nonce
  expiry: trace evidence showed one privileged confirmation followed by 31 seconds of explicit
  pre-execution `mutation_rate_limited` responses, then `privileged_confirmation_required`.
  The test request wrapper now renews that short-lived confirmation only after the proven
  pre-execution rate rejection and retains its original 65-second retry deadline. Ten in-memory
  fake-clock tests cover nonce expiry, the shared deadline and refusal to retry 403, Provider
  failures, unrelated 429s or uncertain outcomes. Production security limits remain unchanged.

Verification so far:

- All-workspace `cargo fmt --all --check`, Clippy (all targets/features, warnings denied),
  `cargo test --workspace --all-features` and `cargo build --workspace --all-features` passed after
  corrections. Five pre-existing explicit Live tests remain ignored: paid Builder and legal-weight
  SAM, PIDNet, RF-DETR and YOLOX process smoke tests. Their absence is not real-model evidence.
- The three affected Rust service/storage packages passed their all-feature suites; focused
  feedback Application tests passed 21 cases and Storage passed six groups. New checks cover
  durable pre-dispatch recovery, interrupted-call evidence, scoped cancellation, transaction
  rollback, concurrent tabs, original summaries and no automatic human answer.
- New feedback E2E: 9/9 passed in `/tmp/annotagent-guided-e2e-48468` (1.6 min). It exercises real
  Rust services with TEST HTTP transport, lost acknowledgement, same-ID recovery, browser abort,
  explicit Stop, invalid/unknown output, scope/model changes, CSRF/owner checks and canvas handoff.
- Capture/narrow-screen rerun: 1/1 passed in `/tmp/annotagent-guided-e2e-49163`. New assets:
  `conversational-workspace/candidate-feedback-authorization.png`, `candidate-feedback-correction.png`,
  and `candidate-feedback-390.png`. Inspected the actual screenshots. The authorization crop uses
  a taller viewport to expose the original model/destination below the sticky header. Desktop
  correction is a scrolled form view; mobile is a full-page capture. They are UI/scope evidence
  using a clearly synthetic image, not model quality, native 200% zoom or novice-usability proof.
- Combined legacy regression after both TEST-only fixes: **21/21 passed**, 4.4 minutes in
  `/tmp/annotagent-guided-e2e-50134`. This covers phase-by-phase samples, initial goal journeys,
  linked clarification and Schema background-disconnect recovery, without weakening the exact
  model, budget, correction or cancellation assertions.
- Web typecheck, all **141 unit tests (32 files)** and production build passed. The existing
  main-bundle size warning remains; a successful build is not a performance acceptance claim.

Final review identified and corrected additional edges:

- The correction button captures the workspace navigation ticket before its POST. A late result
  can save the real request but cannot pull the canvas back after a newer image/task selection.
- A definitive authorization refusal releases an unaccepted local envelope only after a read
  confirms there is no saved authorization. Network uncertainty, 408/5xx, failed status reads
  and rejection after authorization acknowledgement retain the original identity.
- Stop before authorization reaches the server uses the real, separately persisted cancellation
  receipt. Refresh restores that receipt using the original call identity; it does not fabricate
  an inference receipt, restart the request or renew permission.
- Creation of a correction request now rechecks its exact source receipt and cancellation inside
  the insertion transaction. If cancellation wins, no request is created. If creation commits
  first, later cancellation does not delete an already saved human request. General correction
  creation retains its existing behavior.

After the frontend edge fixes, Web typecheck, **144/144 tests** and production build passed.
The bundle-size warning remains. After the transaction guard, the complete Rust format, strict
Clippy, all-feature workspace tests and build were rerun and passed. Storage now has 79 unit tests
plus its 16 integration tests; the four new guarded-request groups cover rollback, changed source
and cancellation ordering. The first final Clippy pass caught two missing semicolons in test
match arms; those were corrected, with no lint suppressions. The same five Live tests remain
explicitly ignored.

Final feedback browser rerun: **14/14 passed**, 2.2 minutes in
`/tmp/annotagent-guided-e2e-51736`. In addition to the original nine cases, it now verifies a
late correction acknowledgement after image/task navigation, definitive authorization refusal
versus unknown acknowledgement, and Stop before authorization reaches the server. The original
identity, no-extra-call assertions and persisted request/cancellation records are checked through
the actual Rust API. The three scoped screenshots were regenerated; the desktop correction image
is still explicitly a scrolled form capture, not a claim that the full image and every control fit
simultaneously. Final staged diff checks passed. No live Provider or original workspace was used.

This integration checkpoint is ready for its own local commit. Structured scope answers, broader request kinds, global Schema
patches, stop-text disambiguation, large-history performance and final accessibility/live-conditional
acceptance remain incomplete. No real Workspace mutations/restart, old keys, push or remote edits.

Next bounded slice: persist a controlled answer to `clarify_scope` against the original call and
context. Distinguish current candidate, current-image class and future Project rule intent without
rewriting the model receipt, treating scope as rejection, or authorizing bulk edits. Saving that
answer must not spend budget or modify annotations/Schema. Only an explicit supported correction
can prepare the existing human request; broader rule changes still need a separate versioned patch.
Cancellation, concurrent answers and exact historical replay must share the same durable boundaries.

### M3 structured feedback scope answers — current implementation and verification

The previous goal turn made concrete progress and committed `7fcb67f`; this turn continues that
boundary, without changing the full goal. Added migration 45 and a controlled scope answer tied to
the original call, task, conversation and canonical persisted-context digest. A human explicitly
selects current candidate, current-image class or future Project rule scope. A candidate answer
also requires a correction reason; boundary feedback is valid only for a saved bounding box.

Saving an answer is not an annotation edit, false-positive rejection, model call, new allowance,
Schema change or continuation event. The immutable answer has one command and one slot per call.
The original model receipt remains `clarify_scope`; human intent does not masquerade as a model
`request_correction`. The first save atomically checks the completed source, ownership, cancellation,
exact saved authorization/message, image hash and feedback sequence. Exact retry reads its original
answer before current-context/expiry checks and cannot overwrite a different answer.

Only a saved current-candidate answer enables the explicit existing canvas-request action. That
action reuses terminal selection, live file checks, same-image exclusivity, stable request IDs and
the existing correction/resume services. Larger scopes remain visibly recorded-but-not-applied;
the later versioned Schema/rule implementation is still required, not replaced by an intent record.

The chat form has no preselected scope or inferred rejection. Local choices and a pending command
are kept separately from server-saved answers. An unknown save keeps its exact command; refresh is
read-only, and a different saved answer cannot silently replace local choices. A late snapshot
cannot erase an already saved answer. Browser-storage failure uses the existing workspace dirty
guard. Scope cancellation uses the actual call cancellation receipt; it does not retract an answer
or an already created human request.

Test-driven findings so far:

- The new interfaces were initially missing; the first targeted compile failed before a scope
  answer could be saved. Storage tests then found that internally tagged Serde unit variants
  ignored extra fields despite enum-level `deny_unknown_fields`. A strict intermediate empty
  struct-variant parser now rejects hidden IDs, geometry and reasons in broader choices.
- A frontend regression first reproduced a late GET with a null answer erasing a saved answer.
  The status merge now preserves the immutable saved record.
- The existing Application TEST consent omitted `remote_model` from its historical summary.
  The stricter scope-source check correctly rejected all four new tests until the fixture recorded
  the actual TEST remote identity. The production source check was not relaxed.
- Review found the pre-existing Application cancellation check ran before exact human-request
  recovery. The Application now reconstructs only the immutable proposal from the original
  response and frozen context, then restores an existing request only if its full input matches.
  Actionability is checked before every new-request path, including unanswered clarification.
  Regression tests cover create-first / cancel-later / changed pixels / restart / retry for both
  direct correction proposals and answered scope questions. Neither recovery reactivates the
  model proposal; fresh work after cancellation still fails closed.

The final Rust command chain passed: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`, and `cargo build --workspace --all-features`.
Application has 129 unit tests (27 feedback tests); Storage has 86 unit tests plus 16 integration
tests. The same five explicitly Live-dependent tests remain ignored, not claimed as verified.
The first intermediate lint runs caught missing test semicolons, public helper `must_use`/doc
annotations and an unnecessarily owned test argument; all were fixed without suppressions.

Web typecheck, 152 unit tests and production build passed after an initially failing render test
caught cancelled answers still telling the user to open a correction. Cancelled answers now retain
their status without that actionable instruction.

The follow-up integration audit found a distinct local-state bug: cancelling an unanswered scope
question unmounted its form. Persisted tab choices became invisible, and with browser storage
unavailable the in-memory selection and its dirty guard were lost. Two failing visibility tests
preceded a fix that keeps the same scope component alive and shows cancelled local choices
read-only. Unrelated feedback does not gain a fake scope question. All 154 Web unit tests and
typecheck passed after this change; actual storage-failure/navigation checks are being added.

The first complete feedback browser suite passed **22/22**, 3.6 minutes in
`/tmp/annotagent-guided-e2e-53801`. The original 14 cases still pass, along with three scope
choices, cancellation, bbox-only boundary intent, same-command retry after lost acknowledgement,
read-only reload, exact candidate handoff, and a cancelled saved answer. API snapshots verify
scope saving itself did not call a model, create feedback/human work, or change Drafts or Schema.
The first targeted rerun passed 9/10, including both new unsaved-choice cancellation cases:
390px tab-storage recovery after reload and in-memory choices plus a rejecting leave guard when
scope-key storage writes fail. Its lost-acknowledgement test exposed a test timing flaw: the form
changes to its frozen-command retry label before the first browser POST completes preparation.
Reloading at that label could abort before the route observed a request, so the test had not
actually simulated its claimed first send. The test now waits for the original route/POST and an
enabled retry action before reload, retaining the two-equal-command and no-extra-model assertions.
This is a test synchronization correction, not a relaxed admission or retry guarantee.

The final scope rerun passed **10/10**, 1.2 minutes in
`/tmp/annotagent-guided-e2e-54882`, with the current production build. The feedback spec now has
24 cases in total: the first 22 passed as a complete suite; the later 10-case scope run includes
both newly added cancellation/storage cases and reruns all eight scope cases. This is scoped
browser evidence, not a claim that the entire repository browser suite was rerun this turn.
Final root Web typecheck and all **154/154 unit tests (34 files)** passed; Rust fmt and diff
checks also passed again. No production API key or user workspace was used.

Screenshots use the same explicit TEST transport and synthetic input, and were visually inspected:

- `conversational-workspace/candidate-scope-answer-form.png`: actual scope/correction-reason
  form, with explicit choices and its not-submitted status (component capture).
- `conversational-workspace/candidate-scope-cancelled-390.png`: actual form within a 390×844
  viewport, not a whole-page screenshot. Disabled checked options and the not-submitted/read-only
  notice remain visible after cancellation/reload.
- `conversational-workspace/candidate-scope-answer-canvas.png`: the actual classification
  candidate, image, label control, evaluation-only scope and Submit correction. The optional
  feedback form is collapsed using its real UI control. A 1280×1600 viewport is intentionally
  used to show image and form together; this does not establish that all controls fit within a
  720px-high viewport or resolve the remaining long-history layout work. The original cropped
  1200px capture was not treated as proof that its offscreen submit action was visible.

The final candidate-only browser check passed again in
`/tmp/annotagent-guided-e2e-55133` after tightening the actual filename/viewport assertions.
The screenshot was captured from that real TEST UI, not rebuilt as a mockup. The test servers
on 8791/8796 exited afterward; the user's 8787 service was not restarted. This milestone includes
only its three new screenshots. Previously modified historical screenshots are left untouched
and outside this commit. Branch remains `main`; both remotes are unchanged, and nothing is pushed.

This checkpoint implements controlled intent and current-candidate handoff, not global label
editing. Current-image class/future-rule changes still need a separately versioned proposal and
impact confirmation. Stop-text disambiguation, other visual requests, history performance and
remaining accessibility/context recovery acceptance are still outstanding. Live model quality,
native 200% zoom and real-human novice usability were not tested. The overall goal stays active.
The existing bundle-size warning is not a performance pass. No real workspace, paid Provider,
old credentials, push or remote edits were used.

### M3 continuation: future-only human Schema forks (verified checkpoint)

The previous scope-answer checkpoint is committed as `245db54`. This slice turns the explicitly
saved `project_future_rule` intention into a separate editable Schema Draft. It is a human-authored
fallback and versioning substrate; it does **not** yet implement LLM-authored Schema patches,
project-wide application, selected-image migration or automatic reinterpretation of old results.
The overall Conversational Workspace goal remains active.

The source is the candidate message's original Sample Test `sample_scope_seals.annotation_schema`,
including Schema ID, revision, complete TaskConfig, goal and boundary rules. An unsealed legacy
test cannot silently borrow the current Draft's Schema. A newer source Schema revision requires
review/test, not an implicit rebase. Fresh writes require the exact saved future-scope answer,
owned task, unchanged frozen candidate/image/feedback context and uncancelled original proposal.

Migration 46 stores the feedback/source/command link and creates the new Schema in one SQLite
transaction using the existing human-Schema insert. The fork has a new Schema ID at revision 1,
but retains the same task identity. Old Schema/Workflow/Test/Published Version/Run/annotations
remain unchanged. No model grant, model call, human-correction request, publish or run is created
by this action. An exact saved semantic command restores the link before live-context checks,
including after cancellation and subsequent edits to the new Draft. The model-style `rationale`
is explanatory input, not part of the persisted Schema definition; provenance remains explicitly
human-authored. Concurrent conflicting requests cannot create two forks from the same answer.

The existing chat feedback card exposes the future-rule editor; extracted Schema fields show
goal, supported output type, labels and boundary rules with an actual before/after semantic diff.
Multi-label behavior and attributes are retained. A separate existing SchemaEditor/Builder flow
handles later edits and fresh authorization. Browser tab state retains unsaved fields and the
original pending command; GET/mount/reload do not send that command or authorize inference.

Integration findings being verified:

- Builder history previously paired a completed task with only a Schema revision number. Different
  Schema IDs both at revision 1 could therefore display an old plan/test as complete. The API now
  returns the frozen operation's ID/revision pair; UI Builder/Journey matching requires both.
  Initial clarification continuations receive the owned original creation revision, never latest.
- The initial human-Schema list also returned future forks; its latest-first lookup could replace
  the original goal card after reload. Future forks must remain in their source feedback card,
  not impersonate the original goal's answer.
- A TypeScript projection does not remove actual TaskConfig fields at runtime. Spreading a full
  server TaskConfig into a bounded decision adds unsupported fields and is rejected correctly.
  The UI serializer must select actual allowed fields, and saved-record matching uses the real
  Storage wire shape rather than assuming it is identical to the POST request.
- The first new Application fixture test incorrectly tried to create a second conversation;
  that API intentionally returns the project's canonical conversation. A distinct foreign ID now
  tests the intended ownership rejection. All 30 focused Application feedback checks then passed.

This checkpoint uses isolated TEST transport only; no Live quality or human usability claim is
made. No real workspace/server, old key, push or remote change is used in this continuation.

The follow-up admission audit found an important companion to exact-ID history filtering:
between reserving a Builder and persisting its seed, no Schema identity was previously readable.
The new scoped reservation writes the owned Schema ID/revision in the admission transaction.
Settlement, Guard interruption and restart keep that pair; conflicting/partial replacement is
rejected. The original unscoped API remains available for legacy records, but a scoped retry
cannot invent a missing old identity. A failing admission test preceded the implementation;
the Application regression now checks history before any seed/session exists and after failure.

Final Rust `cargo fmt --all --check`, strict all-target/all-feature workspace Clippy,
`cargo test --workspace --all-features -q`, and all-feature workspace build pass. Application
contains 132 unit tests (131 pass, one explicitly Live-dependent test ignored); Storage has
97 passing unit tests plus its existing integration suites. Four other explicit expert-weight
Live tests remain ignored, for five ignored Live-dependent tests overall. Web typecheck,
163 unit tests across 36 files and production build pass. The existing >500 KB bundle warning
remains: this is not a history-performance or bundle-performance pass.

The first complete new browser slice passed 5/5 in `/tmp/annotagent-guided-e2e-60242`:
classification/bbox human forks, unchanged old objects, an actual newly authorized Builder,
lost save acknowledgement with equal retried commands, read-only refresh, original-goal-card
preservation, exact new authorization and rejected unrelated/cancelled scope. The HTTP tests
initially requested an unavailable Draft GET route (405); they now read the existing Project
Draft list by stable ID. No production endpoint or data-protection check was relaxed.

The broader browser run additionally tests a real separately authorized Sample Test from each
new fork, with the new exact sealed Schema and actual terminal labels. One old clarification assertion expected a revision-1 Builder to remain displayed
under revised Schema 2. That is precisely the obsolete association removed here: its replacement
asserts saved Schema 2 plus fresh authorization, no inherited completed Builder, unchanged
old Builder history/sample objects, and the original sample still visible in the canvas.
The 43-case combined run finished in 7.8 minutes in `/tmp/annotagent-guided-e2e-62147`:
42 passed and that one obsolete clarification assertion failed. The passing cases include all
24 existing feedback cases, five future-schema cases (both new Builder plus single-image Sample
chains), nine initial-journey cases, two human-Schema cases, the Schema transport case and
clarification cancellation. The corrected clarification-answer test and two UI screenshot cases
then passed **3/3** in 12 seconds in `/tmp/annotagent-guided-e2e-63341`. All 43 scenarios have
passing evidence across these runs; this is not a claim of one 43/43 green run or a full-repository
E2E sweep. No production admission or validation was weakened to satisfy the old assertion.

The final screenshots were captured by the existing browser tests and visually inspected:

- `conversational-workspace/future-schema-classification-form.png`: entered future goal, output
  type, labels/rules and actual semantic differences alongside the unchanged original sample.
- `conversational-workspace/future-schema-bbox-saved.png`: saved independent human Schema,
  fresh authorization action, preserved original bbox and sample-only confirmation scope.
- `conversational-workspace/future-schema-classification-saved.png`: the equivalent saved
  classification case; old labels in the canvas are intentionally not reinterpreted.
- `conversational-workspace/future-schema-bbox-390-form.png`: a full component capture taken
  in a 390×844 viewport, showing wrapped fields/diff/actions; not a whole-screen 844px fit claim.

Desktop evidence uses a real 1280×1800 viewport and keyboard-operated 50% conversation splitter.
This high evidence viewport does not establish short-height usability, native 200% zoom or
performance. The initial component capture was clipped by internal scrolling and was replaced
by real viewport captures, not CSS injection or image editing. A fresh workspace leaves only
2–3 actual TEST model profiles in the pictured inventory. Synthetic predictions prove protocol
and state isolation only, not successful cup recognition or indoor/outdoor classification.

Test services 8791/8796 stopped afterward. Historical modified screenshots are excluded from
this checkpoint; only the four new future-rule images are included. Branch remains `main`, both
remotes are unchanged and nothing is pushed. Remaining work includes automatic bounded
LLM-authored Schema patches, image-class scope application, disambiguated chat stop commands,
other visual requests, long-history/performance and accessibility/context acceptance. Real
model quality, native 200% zoom and real-human novice usability remain unverified. The overall
goal is still active; a human-edited fork is not presented as completion of those features.

### M3 continuation: explicit chat stop commands

The preceding human-authored future-Schema checkpoint is committed as `7fa067c`.
The active goal is unchanged. This continuation implements deterministic, persisted stop
commands through existing cancellation boundaries, not another Agent or execution engine.

Baseline failure: `npm --prefix web run test:e2e -- conversation-stop.spec.ts` in
`/tmp/annotagent-guided-e2e-64293` failed its first assertion: an empty conversation receiving
the standalone message `停止` created one annotation Task and offered a goal authorization.
No Provider was configured and no inference was run. Its Playwright failure trace and screenshot
are in `web/test-results/conversation-stop-standalo-03c83-t-create-an-annotation-goal-chromium/`.
The isolated 8791/8796 servers exited afterward; the real 8787 service was untouched.

A second initial failing test verifies that the exact new stop endpoints use the existing
bounded control lane, not model admission or a broad suffix bypass. Four new web unit tests
cover exact standalone recognition, frozen task/no image scope, non-goal classification and
same-conversation message merging; their initial failure preceded the helper implementation.

Design constraints: a saved stop message freezes the explicit Task or the owned Conversation
scope, snapshots actionable targets, and cancels a unique target or asks for an explicit choice.
Retry uses that original command and target snapshot. It never stops a newly appeared task,
promotes candidate feedback to a goal, or executes on GET/refresh. Pending authorization and
pre-Batch processing must be included, not hidden by filtering truncated history. Journey
parents and their children are deduplicated only when the relationship is unambiguous.
Existing results, immutable versions, budgets, current image and unsaved edits remain intact.
Cancellation receipts must distinguish a persisted request from a settled remote result;
unknown remote completion/cost is not reported as zero. Implementation and verification follow.

Implementation uses the existing Conversation journal and task cancellation services. Migration
47 adds a saved command/target snapshot, not a new executor. Standalone `stop` (case-insensitive)
or `停止` has an explicit `stop_request` reference; prose and candidate feedback keep their
existing meaning. The server rejects stop references through the normal message/goal endpoints.
The same composer remains available on the canonical `work?...&processing=<id>` page: an audit
found that its old conditional hid the input precisely during dataset processing. Direct Batch
Cancel remains available; the fix reuses the input rather than adding a second chat mode.

The frozen targets include pending call authorization, Schema calls, Builder, Sample, Journey,
and confirmed processing before or after Batch creation. Discovery is not truncated to a recent
history page. Multiple independent targets require a choice; an explicit Task never falls back
to the latest Task. Shared Journey children are only grouped when ownership is unambiguous.
Selection is transactional and idempotent, including when a target finishes or gains a parent
after the snapshot. A late retry cannot discover or stop a new task. Reads only observe saved
state; they do not resend control commands, restart inference or authorize new work.

Three publication tests cover another audited race: a pre-publication check alone leaves a gap
before the actual INSERT. The processing-specific publication entry now verifies the exact
saved Project, Draft/revision, sample seal, models and confirmation inside the existing SQLite
publication transaction. Stop winning that transaction prevents a new Version or default;
publication winning first preserves the already-created immutable Version. Scope mismatch and
SQL failure roll back all publication writes. The old standalone publication API is unchanged.

The first expanded executor browser run in `/tmp/annotagent-guided-e2e-66964` passed 10/11 and
found a real Batch cancellation bug. The reused cancellation helper called stale-image recovery,
which cleared `child_run_id` before the post-transaction signaller could find active children.
The Batch said cancelled while its child actually completed, and lineage was lost. The fix
separates reservation release from stale recovery: cancellation keeps real child Run IDs and
consumed usage; recovery retains its existing requeue/clear-stale-child behavior. A failing
Storage lineage test preceded the fix; all six persistent Batch tests then passed. The browser
regression additionally requires each actual child Run to settle as cancelled, not merely a
cancelled parent status. Final aggregate verification is recorded below when complete.

The final browser command `npm --prefix web run test:e2e -- conversation-stop.spec.ts
conversation-stop-executors.spec.ts` passes **13/13** in one 1.4-minute run using
`/tmp/annotagent-guided-e2e-67739`. This includes actual delayed TEST Schema, Journey Builder,
standalone Sample and dataset Batch executors. The dataset test submits `停止` through the
normal processing-page composer and requires the original child Run to become cancelled while
preserving its association, immutable workflow, saved sample and admitted call count. Other
cases verify multiple-target selection, explicit Task A while Task B remains running, frozen
no-work commands, original command/selection recovery after lost acknowledgements, refresh and
Back without POST, new work not replacing a stopped target, rejected foreign objects, unknown
fields and missing CSRF. Existing server tests also prove the narrowly matched control lane
remains usable when ordinary mutation admission is saturated, without bypassing same-origin
protection. All browser services on 8791/8796 exited after the run.

Three new screenshots were captured by those browser tests and visually inspected:

- `conversational-workspace/chat-stop-schema-receipt.png`: an interrupted actual TEST Schema
  call, with truthful unknown remote completion/cost and preserved task scope.
- `conversational-workspace/chat-stop-selected-390.png`: a compact selected-target receipt in
  a 390px viewport. This is an element capture, not proof that the entire page fits one screen.
- `conversational-workspace/chat-stop-batch-workspace.png`: the real 1280×800 processing-page
  view scrolled to the saved stop message alongside its unchanged sample canvas. This is not a
  claim that every processing control is simultaneously visible; the image/prediction is TEST.

The stop parser uses only exact standalone words and no LLM. The browser composition check
dispatches DOM composition events; it is not a native Chinese IME acceptance test. A late GET
cannot erase a selected target or roll settled state back to pending (unit coverage); the UI
disables selection during initial restoration/manual reload, so no E2E artificially unlocks a
disabled control to manufacture a race. Selection and whole-command lost acknowledgements do
have real browser/network recovery coverage. No Live inference, model-quality evaluation,
native 200% zoom, full-repository E2E sweep or real-human usability test is claimed here.

Final formatting and strict Clippy pass (`cargo fmt --all --check` and
`cargo clippy --workspace --all-targets --all-features -- -D warnings`). The complete
`cargo test --workspace --all-features -q` run exits successfully: Application has 133 passed
and one explicit Live-dependent ignored unit test, Storage has 117 passing unit tests plus
integration suites, including six persistent Batch tests. Four additional explicit expert-weight
Live tests remain ignored, for five ignored Live-dependent tests overall.
`cargo build --workspace --all-features` also passes. Web typecheck,
171 unit tests in 37 files and production build pass. The existing >500 KB bundle warning
remains; no bundle-size or long-history performance improvement is claimed by these results.

Only this checkpoint's source/tests, migration, execution log and three new chat-stop images
are staged for its local commit. Pre-existing modified/untracked historical screenshots remain
untouched and excluded. Branch is `main`; origin and tsinghua remote URLs are unchanged and
nothing is pushed. The real workspace and 8787 server are not migrated/restarted for this
checkpoint, so it is not yet a claim that the running user instance serves the new code.

The overall goal stays active. Remaining slices include bounded LLM-authored Schema patches,
image-class scope application, other visual Human Requests, long-history/performance and the
broader accessibility/context acceptance matrix. This checkpoint completes the explicit chat
stop-command slice only. Real-model quality and real-human usability remain unverified.

### M3 continuation: model-assisted future Schema proposals

Previous checkpoint `b09e8df` is committed. This goal turn starts from that source; only
pre-existing historical screenshots are dirty. Current instructions and the full conversational
workspace attachment were read again; no repository/ancestor AGENTS.md or Sites hosting file
was found. The preceding turn made verified progress (persisted commands, regression evidence
and a local commit), not merely a status restatement.

The verified gap is that `ConversationFutureSchemaCard` only offers a human-edited future
fork, explicitly stating it cannot ask a model. The new slice adds a bounded model suggestion
inside that same card: freeze the original feedback, explicit future-rule scope and exactly
tested Schema; authorize one text call; persist the proposal and show semantic differences;
then let the human accept/edit into an independent Draft. It neither rewrites old results nor
inherits permission to publish, process images, install models or migrate annotations.

The first attempted browser baseline could not compile the concurrently introduced UI unit
test's missing module; that is not a user-experience failure assertion. The actual baseline
uses the existing verified `b09e8df` binary/assets in the isolated workspace
`/tmp/annotagent-future-proposal-baseline-DTPyST`. Its single browser test fails because the
existing future-rule card has no `Review model proposal authorization` action. Playwright
records a trace, failure screenshot and context under its temporary test-results directory.
Only the two test processes were stopped afterward; 8791/8796 are free and 8787 is untouched.

The implementation reuses the existing Conversation model-call receipt, task/Project budget,
Provider wrapper and cancellation service. Migration 48 adds a unique, source-linked consent
record, not another model executor or result store. The source is the saved candidate feedback,
the explicit future-rule scope answer and the exact tested Schema ID/revision. Both admission
and human confirmation revalidate this source inside their existing SQLite transactions.
The one-call preview shows the actual configured text model/destination, no image pixels,
a bounded output allowance and unknown cost; it does not imply permission to test images.

Only a single bounded `propose_future_annotation_schema` tool response is accepted. A Draft
may change supported labels, output type, multiple-label behavior, attributes, goal and boundary
rules; a Clarify response asks for missing semantics. Arbitrary actions, unknown fields,
additional tools, image geometry and free-form response text are not executable. Raw output is
retained in the existing receipt and revalidated on save, including its source/digest and
cancellation state. Provider error/unknown completion cannot silently retry or produce a Schema.

The existing future-rule card now previews semantic changes and supports explicit adoption or
human editing. Saving creates a distinct Schema at revision 1; the previous Schema, goal,
tested workflow, sample, Run and annotations are unchanged. Proposed attributes and multi-label
settings survive edits to goal, type, labels and boundary rules. This form displays/adopts
attribute definitions but does not edit them; it says so rather than rendering a fake editor.
The independent draft then uses the existing Builder and sample authorization boundaries.
Model assistance is recorded as provenance, not as a claim of HumanVerified annotation quality.

Initial browser verification passes **11/11** in `/tmp/annotagent-guided-e2e-70292` (2.4 minutes).
The classification and bbox cases each continue through a fresh actual TEST Builder and one-image
Sample Test using the exact new Schema binding. Other cases cover manual edits, model clarification,
invalid output, unknown remote response, changed model/tested Schema, authorization acknowledgement
loss and actual in-flight cancellation. Repeated POSTs, read/refresh and old-object snapshots
verify no implicit calls, adoption, publication or historical rewrite. Independent review then
found two additional UI recovery/control edges, now being regression-tested: manually saving a
draft while its separate suggestion call is running must not hide Stop; and cross-tab cancellation
before authorization acknowledgement must not discard the cancelled call's frozen identity.

Full formatting and strict all-workspace Clippy passed before the final edge regressions. Web
typecheck and **176 unit tests in 38 files** pass. Final aggregate test results and screenshots
will be recorded after those fixes. All model executions above use an explicitly marked local
TEST fixture, not Live inference or evidence of annotation quality.

The additional control failures were reproduced before changing production UI in
`/tmp/annotagent-guided-e2e-70925`: (1) an actual delayed TEST model receipt remained reserved
after a manually saved independent draft, but its Stop control disappeared; (2) another client
cancelled a frozen, not-yet-authorized call, and retrying its rejected authorization cleared the
browser envelope, making that cancellation no longer addressable. Fixes keep model execution
state independent of draft adoption and preserve a frozen envelope when a matching cancellation
receipt is known. They neither implicitly cancel a request nor restart one after a rejection.
The server's human-save and cancellation behavior were not weakened for these UI fixes.

Screenshot inspection also exposed a real default-width problem: the narrow desktop chat pane
kept a two-column semantic diff because the old CSS responded only to viewport width. The same
browser case first failed its measured vertical-stacking assertion in
`/tmp/annotagent-guided-e2e-71036`. Its grid now wraps based on the comparison's available content
width; wider split views still show a side-by-side diff. The test verifies the narrow default
before optionally expanding the draggable split to capture readable detailed screenshots.

Final verification for this checkpoint:

- `npm --prefix web run test:e2e -- conversation-future-schema-proposal.spec.ts`: **13/13**
  pass in 3.3 minutes, isolated `/tmp/annotagent-guided-e2e-71226`. This includes both new
  control/recovery fixes and the measured narrow-chat layout fix.
- `npm --prefix web run test:e2e -- conversation-future-schema.spec.ts --grep
  'forks explicitly|current-image intention'`: **3/3** existing human-only regression cases
  pass in `/tmp/annotagent-guided-e2e-71827`. Screenshot-producing historical cases were not
  run, preserving unrelated pre-existing images.
- `cargo test --workspace --all-features -q`: successful full run, including **136** passing
  Application unit tests and **129** passing Storage unit tests plus integration suites.
  Five explicitly Live-dependent tests remain ignored. After adding the model-admission
  regression, `cargo test -p annotagent-server --all-features` separately passes **44/44**.
- `cargo fmt --all --check`, strict all-workspace/all-target/all-feature Clippy,
  `cargo build --workspace --all-features`, Web typecheck, **176** unit tests in **38** files,
  and the E2E harness's production Web build pass. The existing >500 KB bundle warning
  remains; these checks do not demonstrate a bundle-size or long-history performance gain.
- No 8791/8796 listener remains after the browser suites. The user service on 8787 and its
  workspace were not restarted, migrated or altered.

Two new screenshots were captured from the actual browser and visually inspected:
`conversational-workspace/future-model-proposal-diff.png` (1280×1400) and
`conversational-workspace/future-model-proposal-human-reviewed.png` (1280×1800). They show the
explicit proposal comparison and the independently saved, human-edited Schema with its fresh
Builder authorization entry, alongside the unchanged sample canvas. Their split is deliberately
expanded using the real accessible divider to show the detailed diff. The smaller default pane
has a separate measured stacking assertion; these taller captures do not claim that all controls
fit a 720px screen. The synthetic scene and scripted labels are TEST evidence only, not a claim
that the shown object was visually identified correctly. Transient Playwright failure traces
from earlier runs may be replaced by later runs; the red assertions and workspaces above are
recorded history, not promises of archived trace files.

This slice permits one saved suggestion per feedback scope. An invalid or unknown completed
request is not silently replayed; human-only editing remains available. It is not a universal
free-form rule-migration engine, and attribute definitions are not editable in this compact form.
No Live Provider, real annotation-quality evaluation, native 200% zoom, native Chinese IME,
full-repository browser sweep or real-human novice usability test was performed.

Only the checkpoint's source/tests, migration, this log and two new screenshots are included
in its local commit. Branch remains `main`; origin and tsinghua URLs are unchanged, with no
push. Pre-existing modified/untracked historical PNGs are preserved and excluded. The full
Conversational Workspace goal remains active: current-image class-scope application, remaining
visual/global Human Request behavior, long-history performance and the wider accessibility and
context acceptance matrix still need implementation or verification. This record does not mark
M0–M4 complete or claim that the running user instance already serves these changes.

### M3 continuation: atomic current-image class feedback (in progress)

The preceding model-assisted future-rule slice is committed as `fd413eb`. This continuation
starts from its committed source; the only pre-existing dirty files are historical screenshots,
which remain outside the new work. The preceding goal turn made verified progress (source,
tests, screenshots and a local commit). The full goal remains unchanged; no completion or
blocking condition is inferred from the remaining implementation work.

Current code inspection confirms that `CurrentImageClass` saves intent but has no application
step. Existing regression tests intentionally assert that scope selection alone does not create
a single-candidate request or feedback. The new first browser assertion reproduces the missing
follow-up control after a real TEST scope answer in `/tmp/annotagent-guided-e2e-72842`.
Unlike earlier temporary Playwright output, its failure screenshot, trace and error context
were copied to `/tmp/annotagent-image-class-baseline-evidence-HDLxzo` for this continuation.
The test servers then stopped. No 8787 or real workspace operation was performed.

The chosen implementation reuses Sandbox feedback and the existing draft-copy service: preview
the exact image/class membership, explicitly create a frozen human review, edit or exclude
members in one canvas, then atomically save the complete answer. It must not create N independent
repair Drafts for N objects. A durable answered record resumes into one editable Draft with
exact feedback revision IDs, with fresh authorization required for any new model invocation.
This is a bounded human command, not a new annotation store or inference executor.

The original terminal candidates remain immutable. Membership uses effective labels/values at
the frozen feedback baseline; prior human edits are not reset by Keep. Detection labels match
exactly, and classification uses an explicitly selected real label token, never a joined display
string or substring. Other classes, images, predictions, formal annotations and versions remain
unchanged. Whole-candidate exclusion gets explicit `ExcludeTarget` feedback semantics; the old
`WrongTarget` quality note is not silently reinterpreted as deletion. Removing one classification
token preserves all the others. Collection membership, source/artifact identity, baseline sequence,
ownership and cancellation are checked together before the batch transaction commits.

The existing TEST classifier protocol emits one class per subject. The multi-label regression
will therefore start with an actual single-label sample and explicitly saved human multi-label
feedback against a compatible edited Schema, then exercise class-scope changes. It will not
pretend that a fixture returning an invalid multi-label model response is real classifier support.

The implementation now has a durable image-class review command, Migration 49, Application
recovery and same-origin HTTP endpoints. Creation remains explicit after a read-only preview;
GET/mount never creates a review. A pending group and a pending single-candidate request on the
same task/image cannot coexist. The shared Sandbox feedback transaction validates every member
before writing any of them. An answered outbox resumes into one deterministic repair Draft,
including the frozen members' prior corrections plus this answer, not unrelated later feedback.

The first Rust aggregate passes (139 Application, 141 Storage and 45 Server unit tests, plus
integration suites; five Live-dependent tests ignored). Strict all-workspace Clippy passes after
normal lint fixes. Web reached 192 passing unit tests. Browser execution is still in progress:
the first six-case run in `/tmp/annotagent-guided-e2e-74413` passed three cases, including the
classification path, but all three bbox cases stopped before group creation. Their immediate
feedback execute was falsely rejected as a changed authorization scope. The failure traces,
screenshots and contexts are preserved in
`/tmp/annotagent-image-class-scope-failure-evidence-TQaTsN`.

Investigation identifies an existing numeric persistence boundary: converting typed f32 box
coordinates into a JSON Value and decoding that Value from SQLite can shift a promoted f64's
last bit. A fresh Value equality check then rejects an unchanged typed context. The fix must
retain complete subject/envelope checks and reject real changes; changing fixture coordinates
to avoid the failure is not acceptable. A dedicated red regression and final browser rerun
will be recorded below.

Independent UI review also found and fixed two local-edit losses: editing a second field while
a box temporarily has an invalid width must not reset that width to the baseline; and a saved
answer from another window must not silently discard the local dirty guard or hide its unsaved
geometry. A conflict now offers explicitly labelled local/saved views. The existing Pipeline
inspector gains only an exact, project-validated class-review return context, preserving review,
task, sample and image; the general return-destination allowlist is unchanged. This inspection
does not pretend that a group is a single HumanRequest or automatically invoke another Builder.

The numeric regression now exercises the original failing `[0.12, 0.2, 0.16, 0.22]`
coordinates through persisted authorization, one actual TEST Provider request, receipt recovery,
scope selection, class review, answer and one resumed Draft. Complete typed context and exact
JSON structure are checked, accepting only its original or JSON-persisted representation;
contract, model, scope, unknown fields and real candidate changes remain rejected. Server
admission, both Application receipt checks, class preview and its Storage anchor use the same
strict intent. Focused feedback tests pass 37/37 and class Storage tests 12/12.

A further real browser red reproduced the analogous **client acknowledgement** defect: an
answer with width `0.123456789` was saved and applied by Rust, yet the UI claimed that another
batch had answered because Rust returned that coordinate at f32 precision. Evidence is preserved
in `/tmp/annotagent-image-class-ack-failure-evidence-CCfgWA`. Client comparison now normalizes
only typed bbox edit coordinates to the same f32 representation; command, Artifact, membership,
labels and all other fields remain exact. A one-ULP actual coordinate change still differs.
Validation also follows Core's f32 arithmetic and bounds before freezing a command, while
retaining incomplete local form input for correction. The initial expanded API comparison test
likewise required comparing typed f32 coordinates across typed and generic JSON responses, not
weakening evidence or identity assertions.

After these Rust changes, a second full `cargo test --workspace --all-features -q` succeeds:
141 Application unit tests, 141 Storage unit tests, 45 Server unit tests, and the other workspace
and integration suites pass; the same five Live-dependent tests remain ignored. `cargo fmt --all
--check`, strict all-workspace Clippy and `cargo build --workspace --all-features` pass. These
checks do not exercise the user's workspace or claim that a Live model improved its boxes.

Final checkpoint verification:

- Web typecheck and **194 unit tests in 40 files** pass; the E2E harness's production Web
  build also succeeds (the existing large-bundle warning remains).
- All **6 new browser cases** passed in `/tmp/annotagent-guided-e2e-76513`. After correcting
  the scoped CSS selector to match the existing `.canvas-shell` wrapper, the combined
  **21-case** run (12 existing feedback, 3 existing future-Schema and 6 new image-class cases)
  finished in `/tmp/annotagent-guided-e2e-76821`. Following a user interruption, its saved
  Playwright `.last-run.json` was recovered with `status: passed` and no failed tests; the
  matching selection was re-listed as 21 cases in three files. No test server remained.
- The final bbox browser case measures the actual SVG at 650px on a 1280×1800 viewport and
  300px at 390×844, retains selection and unsaved edits across that change, saves a high-
  precision coordinate, restores an unknown acknowledgement without another batch, and opens
  the revision Draft then returns to the exact class review without a POST.
- Two actual TEST-browser screenshots were inspected:
  `conversational-workspace/image-class-bbox-review.png` and
  `conversational-workspace/image-class-classification-review.png`, both **1280×1800**.
  They show local edits before the explicit atomic save. These tall captures do not claim
  that all controls fit a 720px screen; the bbox capture also shows that the preceding image
  heading can scroll behind the sticky header. The synthetic scene and scripted labels are
  execution evidence only, not an accuracy or real-world visual-recognition demonstration.

This slice covers terminal model candidates in one frozen sample image/class. Human-added
examples are not silently incorporated into that membership. Classification token removal
preserves other labels, and explicit exclusion remains separate from a WrongTarget note.
Answers generate one independent Draft; inspecting it has an exact return link. The group-
specific, same-chat fresh Builder/Sample authorization continuation is still a next slice,
not a completed capability disguised as a single-candidate HumanRequest. Existing single-
candidate continuation remains available and regression-tested.

The next bounded integration can reuse `PipelineBuildMode::RepairDraft`, the existing advisor
loop and Sample operation. It needs an explicit image-class-review source, exact saved group
evidence and sealed Schema validation, and durable Builder admission linkage; it must not infer
ownership from a recent-history window or create another repair Draft. Broader Human Request
coverage, long-history performance and the full accessibility acceptance matrix also remain.
No Live Provider, native 200% zoom, native Chinese IME or real-human usability study was run.

This checkpoint includes only its source/tests, migration, this log and two new screenshots.
Pre-existing historical PNG modifications remain excluded. Branch is `main`; both existing
remotes are unchanged, with no push. The user's service on 8787 and real workspace were not
restarted or altered. The overall goal remains active, not complete.

### Continued slice: exact image-class Builder source and durable recovery

The previous atomic class-review checkpoint was committed as `1b18ddb`. This continuation
implements the backend boundary for a **separately authorized** repair of its prepared Draft.
It does not yet claim that the same-chat group Builder/Sample controls are complete.

- A distinct typed image-class repair source binds the applied review, frozen membership
  digest, exact feedback digest, editable Draft ID/revision/hash and original sealed Schema
  ID/revision. It is not represented as a single-candidate HumanRequest.
- Pending or merely Answered reviews cannot become Builder sources. The resolver compares
  the original Sandbox sample and sealed Schema, and verifies the prepared evidence against
  exactly the frozen member baseline plus saved group decisions. It makes no Provider call.
- The existing Builder repair mode and bounded authorization are reused. Both Server scope
  and Application execution check the exact source; the load checks revision/hash again.
  New optional source fields are omitted when absent from request serialization, retaining
  existing authorization/request-hash shape. No automatic Sample Test or publication is added.
- Admission stores source evidence before a session/working Draft exists. Retries and
  settlement cannot replace it; restart and settlement preserve it. Server POST receipt
  recovery now uses an owned operation-ID lookup rather than scanning the newest 32 records.
- The existing history GET additionally accepts either an exact operation ID or an owned
  image-class review ID. Source filtering happens **before** LIMIT 32, not in the client after
  an unrelated recent-history window. Read paths do not admit execution or increase budget.
- Prepared repair lookup excludes foreign, deleted, archived and published Drafts, including
  unavailable parent Pipeline records. This leaves historical objects intact.

New isolated tests cover source persistence before seed, altered retry/settlement rejection,
restart recovery, exact lookup and source-filtered history after 33 newer operations, task
ownership, unavailable Draft states, Applied-only sources, modified revision detection,
sealed-Schema substitution rejection and unchanged original sample data. The resolver started
with an explicit failing implementation test, then passed after implementation. Both Application
source tests now pass; six Builder Storage tests and the unavailable-Draft test pass. Full
workspace regression and final lint/build results are recorded below when completed.

Remaining in this slice: wire the distinct group source into the existing chat Builder card;
persist its bounded consent across unknown acknowledgements; use exact operation/source GETs
for refresh recovery; then explicitly authorize samples and verify the complete browser loop.
No new browser screenshots or Live inference evidence is claimed for this backend checkpoint.
Historical screenshot changes remain untouched and excluded from the local commit. The real
workspace, service 8787, credentials and remotes remain unmodified; no push is authorized.

Backend checkpoint results: `cargo test --workspace --all-features -q` exited successfully
(142 Application tests passed/1 ignored, 144 Storage tests passed, 45 Server tests passed,
other workspace/integration/doc suites passed; five explicit environment-dependent tests
ignored overall). The final additional changed-Draft/Schema regression was then run with both
Application source tests, 2/2 passing. `cargo fmt --all --check`, full-workspace/all-target/
all-feature Clippy with warnings denied, and `cargo build --workspace --all-features` passed.
Web source was unchanged in this checkpoint; Web/browser tests were not rerun and are not
claimed as evidence for the new group continuation. Overall goal remains active.

### Same-chat class correction → Builder → sample continuation

Backend source/recovery support was committed as `b41a339`. The Web continuation now reuses
the existing repair-context loader, Builder card and Sample card rather than introducing a new
executor or disguising class reviews as single HumanRequests. Applied group reviews display
their own repair card; pending/Answered reviews poll only the owned saved record, allowing
the card to update after canvas save without model work. The original sealed sample supplies
the Schema identity, and the backend revalidates the full source at authorization/execution.

Builder history uses the exact operation when known and the admitted class-review source
otherwise. A class repair cannot match an unrelated working Draft or a record with absent
source provenance. Confirmed requests persist their original preview and consent in scoped
session storage before POST; storage failure prevents dispatch. Refresh reads receipts, while
an unknown outcome offers an explicit retry of the same envelope. It does not renew a grant
or auto-dispatch. A recovered authoritative receipt replaces the transient transport warning;
the saved operation's real failure evidence remains displayed.

Three new pure Web tests cover exact pending-envelope restoration, incorrect source/Schema/
Draft/authorization rejection, and matching admitted provenance before a session seed. The
full Web suite passes 197 tests in 41 files, and typecheck passes. The isolated production-build
browser harness retains its existing bundle-size warning.

The first real browser test in `/tmp/annotagent-guided-e2e-82888` passed through saved group
decisions, explicit Builder authorization, explicit sample authorization and canvas opening.
The subsequent two-case run in `/tmp/annotagent-guided-e2e-83061` also passed a deliberately
dropped successful Builder response plus failed receipt reads, then refresh recovery without
another POST/model call. Screenshot inspection found that explicit result opening selected
the first input rather than the corrected image. The group callback now supplies its original
image ID; both cases passed again in `/tmp/annotagent-guided-e2e-83173` with an exact image URL
assertion. This is an actual navigation fix, not a screenshot-only adjustment.

Final bbox/classification and narrow-screen evidence is recorded below after completion.
All new model work uses the explicit TEST transport on 8796 in disposable 8791 workspaces.
Fixture detections on synthetic images are not accuracy evidence or Live improvements. No
real workspace/service8787, credential, formal user annotation or remote was changed. Old
screenshots are preserved; only new continuation screenshots belong to this slice.

Final run `/tmp/annotagent-guided-e2e-83300`: **4/4 browser cases passed**. Bounding-box and
classification groups each exercise normal completion and lost successful Builder response
with temporarily unreadable receipt GETs. The latter recover after reload with one Builder
POST, then authorize samples separately. All cases assert the prepared repair Draft is reused,
the exact group provenance is returned, refresh does not add calls, and explicit sample opening
retains the original correction image. The two unknown-ack cases also verify 390×844 navigation
without document horizontal overflow; this is not a claim of complete mobile annotation or
native 200% zoom accessibility.

Two new 1280×800 actual TEST screenshots were inspected:
`conversational-workspace/image-class-builder-continuation.png` and
`conversational-workspace/image-class-builder-classification.png`. The chat is scrolled to the
saved sample action and the canvas shows the original correction image. Synthetic soccer
pixels with scripted cup/bottle labels test transport and UI only; they deliberately provide
no model-quality evidence. Long chat/canvas content still scrolls within the workspace; these
captures do not prove the full layout/accessibility acceptance matrix.

This slice completes the basic same-chat group repair/sample path, not the whole objective.
Next checks include advanced Pipeline detail return context from the new Builder card (the
existing generic detail link still lacks a return context), storage-corruption/blocked-storage
browser recovery, source changes during authorization, and broader single-request regression,
long-history performance and accessibility coverage. No real-user usability study was run.
Rust source is unchanged from `b41a339`; its preceding Rust regression is not reported as a new
run here. No push; main branch and the user's historical screenshot modifications are retained.

### Pipeline detail return and browser retry-storage failures

The chat continuation checkpoint was committed as `8e0b67a`. The Pipeline inspector now accepts
a validated same-project conversation return with conversation/task IDs, preserving the selected
sample, image and other canonical query context. Existing class-review returns still require their
complete class/sample/image context. External URLs, foreign Projects, fragments and incomplete
task contexts are rejected; unrelated management destination allowlists are unchanged. The
Builder detail link supplies the current canonical workspace location. The inspector retains
the appropriate visible return action across refresh without authorizing model work.

The first expanded browser run `/tmp/annotagent-guided-e2e-83714` passed three cases but exposed
a genuine classification UI race: the detail link was visible, then detached and hidden when
the pending-human-request read changed `editing`, causing the completed Builder section to
collapse. The test timed out rather than reaching the inspector. Failure screenshot/context/
trace were copied to `/tmp/annotagent-builder-return-race-dEB9s3` before rerunning. The fix removes
editing readiness from the historical Builder completion decision and keeps its detail link
outside the collapsible evidence section. Tests do not add sleeps or force-click hidden links.

Unreadable local retry envelopes now prevent new Builder authorization until the user explicitly
discards that exact local record. The warning states that server operations/costs may remain;
discard neither cancels them nor authorizes a call. If another value replaced the local record,
discard refuses it. Clearing a recovered valid pending request never silently removes a corrupt
record. Browser-storage write failure remains a pre-dispatch failure, with a visible explanation
and explicit retry after storage is restored.

The expanded browser matrix covers bounding boxes and classification under normal transport,
lost acknowledgements, rejected session-storage writes and corrupted pending records. Every
path then inspects the saved Pipeline, refreshes its detail URL, and returns to exactly the prior
conversation/sample/image without increasing call count. Final results are recorded below.

Final `/tmp/annotagent-guided-e2e-84080` run passed **8/8 cases** in 1.3 minutes. Each corrupt-
record case proves the authorization button is disabled before explicit local discard, while
each storage-write rejection proves no Builder receipt and no extra call were admitted. All
cases complete the separate authorized Builder/sample flow and Pipeline detail refresh/return.
There was no force-click or blind re-dispatch. The same two owned continuation screenshots
were refreshed; the classification capture was visually inspected after the passing run.
Web typecheck and all **198 unit tests in 41 files** pass; production build passes in the E2E
harness with the existing bundle-size warning. Both isolated listeners stopped after completion.
Rust source was unchanged, so no new Rust regression is claimed here. No Live or real-human
usability test, native Chinese IME or native 200% zoom test was performed.

Remaining objective work includes the broader original single-request continuation regression,
authorization/source changes in flight, long-history behavior/performance and the full
accessibility/requirement audit. This commit does not claim completion of the overall goal.
Only this slice's source, tests, log and two owned screenshot updates are included. Historical
unrelated PNG changes remain excluded; no push or remote/workspace/service8787 changes.

### Authorization changes and existing single-candidate repair lifecycle (2026-09-09)

Re-read the original Conversational Annotation Workspace attachment against the current
implementation, retaining its full scope. The preceding return/recovery slice is `fff2139`.
This checkpoint adds bbox and classification browser cases where another editor changes the
prepared Draft **after** the displayed authorization preview. The old confirmation is rejected
before any Builder receipt/model call or cumulative grant increase. Explicitly requesting a
fresh preview then completes the existing repair/sample/detail-return flow. Both cases passed
in `/tmp/annotagent-guided-e2e-85139` using the TEST transport only; no screenshots were rewritten.

The original single-candidate repair resolver still read Drafts through the historical getter,
which includes Trash. A new TempDir-only regression uses the existing management service to
move a prepared repair Draft to Trash and then asks for a repair preview. It first failed with
`A trashed repair must not be offered for fresh authorization`. The resolver and execution load
now share the same availability check as class repairs: owner, editable state, Draft lifecycle
and parent Pipeline lifecycle must remain usable. The helper name is generalized; no duplicate
executor or deletion implementation was added. The test passes after the fix and additionally
checks that the already completed operation is still readable without new Provider calls.

Single-candidate frontend matching now prefers its admitted `human_request` provenance, even
before a session/working Draft exists. Conflicting modern provenance is never overridden by a
working-Draft heuristic. Only source-less legacy records retain the existing repair-mode and
Draft-ID fallback. A unit regression covers the no-session receipt, wrong request, wrong Schema
and rejection as a class review. Broad single-candidate browser regression and source-filtered
retrieval beyond the ordinary history window remain to be completed; this matching change
alone does not claim those broader behaviors.

Final test results are recorded below. The real workspace, service8787, remote configuration
and credentials were not changed. All lifecycle mutations above occur only in the Rust test's
temporary workspace. No Live model, real-world accuracy or human-usability claim is made.

Checkpoint verification: full `cargo test --workspace --all-features -q` exited successfully
(143 Application passed/1 ignored; 144 Storage passed; 45 Server passed; remaining workspace,
integration and doc suites passed; five environment-dependent tests ignored overall). Full
workspace/all-target/all-feature strict Clippy, formatting check and build pass. Web typecheck,
199 unit tests in 41 files and production build pass; the existing large-bundle warning remains.
The two new browser authorization-change cases passed, not the whole browser suite. No new
screenshot/Live/native-IME/200%-zoom/human-test evidence is claimed. Only owned source/tests and
this log are committed; historical PNG modifications remain excluded and the goal stays active.

### Single-candidate source-filtered history and original golden-path regression

Previous lifecycle/authorization checkpoint: `819284a`. Single-candidate Builder cards now
query history by their owned human-request ID, not only the last 32 operations across the task.
The Server rejects mixed operation/class/human source filters. Application validates the exact
request's Project, conversation and task using a point lookup rather than enumerating all human
requests. Preview uses that same point ownership check.

Storage filters admitted human provenance before LIMIT 32. Source-less legacy operations may
match only the owned request's prepared Draft and a repair-mode Agent session. Conflicting
modern source metadata cannot be overridden by that fallback. Tests insert 33 later operations
and prove both modern and legacy repair records remain discoverable while an image-class
record with the same working Draft is excluded. Application tests also cover the real completed
repair record, unknown requests, foreign conversations and mutually exclusive selectors.

To rerun the original long browser scenarios without overwriting pre-existing screenshot work,
their screenshot path now accepts the optional `ANNOTAGENT_E2E_EVIDENCE_DIR`. Default paths
are unchanged; no assertion, transport or execution is stubbed. This run uses
`/tmp/annotagent-single-repair-evidence-QZJqlj` and the existing disposable harness workspace
`/tmp/annotagent-guided-e2e-86453`. Bbox and classification-review are selected from the existing
`conversation-samples.spec.ts`, including human answer, preserved repair, samples and formal
processing/Review/export rather than a new reduced demonstration. Final results follow below.

Both original browser scenarios passed (bbox 9.6s, classification-review 10.1s; total harness
42.3s). Their real files and 27 screenshots are retained in the isolated evidence directory;
`repair-bbox.png` and `formal-export-classification.png` were visually inspected. These are TEST
synthetic images/scripted predictions and actual Rust execution, not Live-quality evidence.

The export inspection identifies an outstanding **delivery** gap: the UI capture exposes the
server output path and report, and the existing test verifies output JSON by directly reading
the server file. That is evidence of actual export generation, not proof of browser download
or an export-asset completion card. No export download endpoint was found in the targeted
Server/API search. This must be investigated and completed next before claiming the required
goal → correction → export *delivery* path, despite the current golden tests being green.

Full workspace/all-feature Rust tests pass (143 Application/1 ignored, 145 Storage, 45 Server,
other workspace/integration/doc suites; five environment-dependent tests ignored overall).
Strict all-target/all-feature Clippy, fmt check and workspace build pass. Web typecheck and
199 unit tests pass; production Web build succeeds in the browser harness with the existing
bundle-size warning. No real workspace/service8787/credential/remote was modified and no push
was performed. Screenshot changes predating this work remain untouched. Goal remains active;
browser delivery, broader long-history/performance and accessibility acceptance remain open.

### 2026-09-09 — Real export archive delivery

Completed the missing browser delivery boundary using the existing Project exporter. Each
explicit export creates a unique generation and ZIP of exporter-produced files plus a delivery
summary. The latest-result pointer is atomically replaced; old generations remain intact.
The owner-scoped download endpoint takes an export UUID, never a filesystem path, and checks
the persisted size/SHA-256 before streaming the archive. It neither runs an exporter nor calls
a model. Directory/file symlinks, outside-generation source paths, oversized manifests and
archives, missing IDs and altered archives are rejected. Download is bounded to 512 MiB;
this is an annotation-file delivery, not a promise to bundle all original images.

The export success view now exposes a download link with the same persisted metadata after
refresh. Legacy reports still load but do not pretend to have a downloadable asset. Browser
tests now actually receive a download before and after refresh, compare bytes and SHA-256,
and check the suggested ZIP filename; the Rust integration reads the native annotations
inside ZIP, checks independent generations and refuses a tampered old archive.

Validation: Application/Server all-feature tests pass (144 Application passed, one explicitly
billable smoke ignored; 45 Server passed). Strict targeted all-target Clippy, fmt, Web
typecheck and 199 Web unit tests pass. The two full TEST bbox/classification-review browser
paths passed with actual downloads (44.4s total). Screenshot review found the initial anchor
was unstyled, so it now uses a visible, keyboard-focusable primary download treatment;
post-style browser verification is recorded below. Evidence is isolated under
`/tmp/annotagent-export-delivery-evidence`, not overwriting earlier repository screenshots.

Remaining: persistent conversation export job/card integration, complete long-history and
accessibility acceptance. This is TEST backend evidence, not Live model quality or human
usability validation. No real Project data, service 8787, credentials or remotes changed;
no push. The overall goal remains incomplete.

Post-style rerun initially exposed an existing bbox race-test harness hang: after switching
tasks the old lookup can be cancelled, but the test awaited only a response. It now subscribes
before navigation to either response or request failure and retains the same stale-result /
stale-error rejection assertions. Final rerun passed both complete scenarios (9.3s each,
23.9s harness) on `/tmp/annotagent-guided-e2e-88140`. The styled download success screenshot
was inspected; no fake file-manager control was added. Production build retains the known
bundle-size warning. Broader workspace tests were not rerun in this increment; the two changed
Rust packages were fully tested instead.

### 2026-09-09 — Task-linked export receipts and conversation delivery cards

Added a minimal SQLite receipt linking the existing Project exporter to the requesting
Project/Conversation/Task. It does not define a new exporter or Annotation store. Admission
checks stable Project ownership and reserves the operation UUID before export. Repeated
completed requests return the recorded result; changed format/owner and unavailable tasks
are rejected. A caller-selected UUID cannot overwrite an existing export generation.
Completion is persisted only after the real archive/report exists. Unconfirmed operations
are labelled as such, never as running or complete. Their retries do not blindly re-export.

The canonical export page obtains task context from its validated workspace return route.
Its browser retry ID survives refresh; confirmed failure permits an explicit new request,
whereas unknown outcomes retain the original identity. Returning to the conversation shows
the saved downloadable archive and compatibility warnings. The card explicitly says the
export is Project-wide and can contain other tasks' confirmed results: task linkage is a
request origin, not a false claim of task-only export filtering. Reads cannot create exports.

Evidence: Storage receipt tests cover ownership, duplicate identity, conflict, immutable
completion and database reopen. All-feature Storage/Application/Server tests pass (146/144/45
unit tests respectively; one billable Application test intentionally ignored, associated
integration tests pass). Web 199 unit tests and typecheck pass. Both complete browser paths
passed with task-linked export cards, duplicate POST returning the same archive, foreign task
rejection, and refresh recovery (9.5s/9.6s; 38.6s harness). Production build succeeds with the
known bundle-size warning. Isolated workspace `/tmp/annotagent-guided-e2e-89065`; screenshots
`/tmp/annotagent-task-export-evidence/export-card-bbox.png` and `export-card-classification.png`.
The classification card screenshot was visually inspected. A final targeted Rust check
also covers rejection of a pre-existing generation UUID.

Remaining: true background export dispatch/recovery across interruption, receipt history
pagination (currently explicitly displays latest 100), broader long-history/performance and
accessibility acceptance. Pending receipts deliberately do not claim a live worker. Tests
use the explicit isolated TEST backend, not Live quality or human usability evidence. No
real workspace or service 8787, remote, credentials or historical PNG edits were touched;
no push. Overall goal remains active.

### 2026-09-09 — Bounded, owner-checked export history pages

Replaced the inaccessible older-than-100 history boundary with keyset pagination using
`(created_at,id)` and an owned export UUID cursor. Unknown/cross-task cursors fail explicitly;
the cursor is never interpreted as a path. The conversation renders only 20 records per page
with newer/older controls (not shown for a single short page), keeps the task/image URL, and
does not create any export on paging. Latest-result query invalidation still targets only the
requesting task. Refresh currently returns to the first history page; this remaining local
view-restoration limitation is not considered completed acceptance.

Storage test reads all 125 historical receipts with tied timestamps in 17-record pages,
adds a newer receipt between reads, verifies no duplicate/omission, and rejects another
task's cursor. Two full TEST browser paths pass (10.3s/10.0s, 46.6s harness), including real
HTTP empty-page/invalid-cursor checks. The classification path separately injects an explicitly
labelled read-only 21-record browser fixture to exercise older/newer controls; these are not
21 real exports and never appear as successful assets. The real download card is restored
after removing that UI fixture. Web 199 tests/typecheck and targeted strict all-target Clippy
pass. Screenshot evidence: `/tmp/annotagent-export-pages-evidence`; isolated server workspace
`/tmp/annotagent-guided-e2e-89648`. No real data or old screenshot edits changed; no push.
Background export interruption recovery, history-page refresh retention and broader goal
acceptance remain unfinished.

Final browser rerun after hiding unnecessary one-page navigation passed both paths
(9.9s/9.8s; 25.0s harness), workspace `/tmp/annotagent-guided-e2e-89881`.

### 2026-09-09 — Recover the durable-file / missing-receipt window

Before adding asynchronous dispatch, added a regression that reserves a real task export,
writes its actual archive/report through the existing exporter, omits the completion receipt,
then reopens LocalApplication and retries the original operation. It failed with the previous
“pending, interrupted or failed” error. The explicit retry now reads the exact owned export
generation, verifies its archive identity/size/SHA-256 and matching format, then completes
only an unterminated receipt. It does not regenerate files, change the completion timestamp,
or execute inference. The same verified reader is shared by download and recovery to avoid
reading an unverified second manifest. Terminal failures cannot be rewritten as success.

The regression now passes and checks original timestamp/digest equality after reopen. Missing
files, tampered archives, mismatched formats and an existing terminal failure are separately
rejected; missing-file retries create no generation directory. This simulates a precise crash
window with persisted state; it is not a process-kill or background-worker-disconnect test.
Targeted all-feature integration, strict Application/Server Clippy and formatting pass.
Full changed-package and browser verification follow below.

This increment repairs receipt recovery, not the whole asynchronous export requirement.
The HTTP export action still waits for exporter completion; durable Job-ID-first dispatch,
bounded worker lifecycle and its interruption tests remain necessary, along with the other
open M4 items. No real workspace, model/credential, service 8787 or remote was changed.

Full Application/Server all-feature tests pass (144/45 unit tests, one billable Application
smoke ignored). Both complete TEST browser paths pass (10.1s/10.0s; 1.0m harness including
builds), isolated workspace `/tmp/annotagent-guided-e2e-91081`, screenshots under
`/tmp/annotagent-export-recovery-evidence`. Production Web build succeeds with the existing
bundle-size warning; no new frontend visual behavior was introduced in this increment.

### 2026-09-09 — Detached, bounded export Jobs

Conversation export now opts into Job-ID-first HTTP admission. The existing receipt is
reserved before a worker is dispatched; two blocking-worker permits bound concurrent archive
work. The worker owns its Application reference and request scope independently of HTTP
response/browser lifetime and invokes the same existing exporter. Existing synchronous
clients remain supported. Duplicate IDs validate the original owner/format and cannot
dispatch twice. The status endpoint is a read-only, exact owned-Job lookup. Active status
comes from the current server's actual JoinHandle, not a persisted pending flag. A finished
or absent worker with no result is unconfirmed; it is not automatically restarted. Explicit
POST retry can use the prior verified-file recovery path.

The export page observes the returned Job with GETs; unmount/project change aborts only that
observation and retains the retry identity. Conversation pending cards query only their Job
and refresh the receipt page when it becomes terminal. Errors have an explicit status retry.
The UI explains that leaving does not cancel export. Completion notifications currently use
bounded per-Job polling, not durable export SSE events; that remaining requirement is open.

New server regression verifies capacity exhaustion writes no receipt, response ownership can
end independently of the registered worker, real empty-Project failure is persisted, terminal
retry does not spawn another worker, and foreign-task status fails. Client regressions cover
one POST plus read-only observation, abort without cancellation/re-execution, and no blind
retry on unconfirmed inactive work. A hard process-kill / live long-export browser-disconnect
test has not been performed; the existing reopened-Application recovery test covers its
specific durable-file window only.

Validation: full changed-package all-feature tests pass (146 Storage, 144 Application plus
one explicitly billable ignored smoke, 46 Server; associated integration suites pass).
Targeted strict all-target Clippy, fmt, Web typecheck and 202 unit tests pass. Both complete
browser paths pass against real background Job receipts and actual ZIPs (12.5s/12.0s;
52.9s harness), isolated workspace `/tmp/annotagent-guided-e2e-92099`; evidence under
`/tmp/annotagent-background-export-evidence`. Production build retains the known size warning.
All model data in these tests is explicit TEST fixture data, not Live quality evidence.
No real workspace, service 8787, credential, historical PNG edit or remote changed; no push.
Overall M4/goal remains incomplete (persistent events, interruption end-to-end evidence,
history-view restoration and broader accessibility/performance acceptance remain).

### 2026-09-09 — Transactional export events and resumable SSE

The global Run event type requires a real Run ID, so it is not reused with a fabricated ID.
A small owned export-event journal now records requested/completed/failed in the same SQLite
transaction as its existing receipt transition. Failed event insertion rolls back admission
or completion; duplicate terminal calls do not emit another event. Old receipts are still
readable without inventing historical events. The stream reuses the existing SSE capacity
guard and keep-alive pattern and sends only operation ID/kind/sequence, not another result copy.

The owned task stream supports Last-Event-ID replay. Initial connection (or a cursor beyond
the current head) sends a snapshot refresh marker at a saved sequence boundary; subsequent
events come from the durable journal in bounded pages. Conversation UI coalesces notifications
and refreshes only its current export page. New events do not change the image or history page.
Per-Job read-only observation remains for actual worker liveness/unknown outcomes; it is not
an Agent loop or node-by-node inference polling.

Evidence: SQLite reopen test verifies requested/completed persist once and replay begins
strictly after the given cursor. Injected admission/completion event-write failures roll back
their receipt writes. Server stream-body test checks resumed failed event and initial snapshot.
Both full browser paths additionally use real streaming HTTP, close after requested, reconnect
with that event ID, and receive completed for the exact archive without executing export again.
They pass (11.8s/12.1s; 44.4s harness), isolated workspace
`/tmp/annotagent-guided-e2e-92791`, evidence `/tmp/annotagent-export-events-evidence`.
Full changed-package all-feature tests pass (146 Storage, 144 Application/one billable ignored,
46 Server plus associated integration suites); Web 202 unit tests/typecheck pass. Production
build retains the existing size warning. Strict Clippy was rerun after correcting test import
placement. No real workspace/service8787, credentials, remote or previous PNG edits changed.
No push; no Live or human usability validation is claimed. Hard process-kill evidence and
the remaining view restoration / accessibility / performance acceptance still remain open.

### 2026-09-09 — Actual child-process kill at two export checkpoints

Added an isolated process-level regression, not another simulated reopen. The parent starts
the current Rust test executable with only the existing generic classification/export test,
using a child TempDir nested under the parent's owned test directory. Test-only checkpoint
code publishes a synced handoff file and parks the child. The parent verifies the child is
still live, calls Child::kill, waits for termination, and on Unix asserts signal 9. It then
opens the saved workspace in the parent process and retries the same task-scoped operation.

Two checkpoints are covered: (1) admission and requested event saved, before generating
files — two retries remain unconfirmed, create no directory and no completion event;
(2) archive/report saved, before completion receipt — two retries preserve original path,
timestamp and digest, leave exactly one generation, and persist exactly one completed event.
Both killed children are waited on; all data is under the owning TempDir and cleaned by its
parent. No production fault-injection hook or model/HTTP call was added.

The process-kill test passes (0.30s runtime after build); Application strict all-target
all-feature Clippy passes. This proves abrupt process death at these Application/export
boundaries. It is not a machine-power-loss test, arbitrary mid-compression kill, or a kill of
the user-facing HTTP service. Previous HTTP Job/SSE/browser evidence remains separate. No
new screenshot is claimed because this increment changes only test code. Full Application
regression result follows below. The overall goal remains open for view restoration and
broader product/accessibility/performance acceptance; Live/human validation remains unexecuted.

Full Application all-feature regression passes: 145 unit tests, one explicitly billable
smoke ignored, plus the offline advisor integration test. Formatting and diff checks pass.
No push, remote modification, real workspace mutation or prior screenshot overwrite occurred.

### 2026-09-09 — Export history URL restoration and an actionable escape during refresh

Export history now uses an explicit `export_before` cursor in the existing conversation
canonical route, alongside the same task, image and processing context. Empty, malformed,
duplicate or unscoped cursors are rejected by route parsing; the existing server still checks
actual cursor ownership. Refresh and browser Back/Forward restore the selected history slice.
Visited predecessors remain a local navigation convenience only: a deep link with no known
predecessor says Latest exports, not a fabricated previous page number. Same-task image/result
selection and the formal result return path preserve the cursor. No new business-state copy,
model call or export POST is triggered by navigation.

The initial routing test failed before implementation. A subsequent browser run exposed an
additional race: the initial SSE snapshot could disable the Latest exports button between
pointer events by refreshing history. Read-only return navigation no longer depends on query
loading. The browser regression explicitly holds a refresh GET open and verifies that return
still works; it then releases that old response without replacing the current page.

Final isolated browser run passes both bbox and classification-review full paths (11.5s and
12.1s, 28.8s harness), workspace `/tmp/annotagent-guided-e2e-94709`. Pagination's 21 rows are
explicit browser TEST fixtures, not claimed real exports. Invalid cursor rejection uses the
real Rust service, and the surrounding paths use actual saved export archives/downloads.
Screenshot inspected: `/tmp/annotagent-export-view-evidence/export-earlier-page-restored-TEST.png`.
The same evidence directory contains the two workflow screenshots without overwriting existing
repository PNG changes. Web 203 unit tests and production build/typecheck pass; the existing
large-bundle warning remains (1.07 MB uncompressed main JavaScript). Diff check passes.

This increment does not change Rust code, the real workspace/service, credentials or remotes.
No push. Overall M4 remains open for the broader accessibility/performance and final acceptance
audit; Live model and human usability testing remain unexecuted. This navigation fix is not a
claim of model-quality improvement or overall goal completion.

### 2026-09-09 — Keyboard-operable split pane and narrow-panel checks

The existing splitter supported arrows but lacked range-end keys and a readable percentage.
A browser regression first failed (End left aria-valuenow at 34 rather than 50). The same
component now supports Home/End within its existing 25–50% bounds, reports the conversation
percentage, and ignores composition/229 and modified system shortcuts. No new layout or
business state was introduced. Arrow input in the message textarea does not resize the pane.

The isolated journal test passes after the fix, including actual keyboard activation of the
mobile Conversation/Images buttons, hidden desktop splitter on mobile, preserved object URL,
and no inference requests. It exercises 1440, 1280, 1024 and 390 widths with reduced-motion
media enabled. This checks supported basic switching under that preference, not a screen-reader
session, native Chinese IME session, or actual browser 200% zoom; those remain unverified here.
Its synthetic source image is explicitly TEST data and demonstrates layout, not model quality.

Final browser run: 1 passed, 6.0s harness, workspace `/tmp/annotagent-guided-e2e-95174`.
Screenshots now honor the existing optional isolated evidence directory, avoiding modification
of prior repository PNGs: `/tmp/annotagent-conversation-keyboard-evidence/journal-{width}.png`.
The 390px screenshot was visually inspected: header, panel switch, upload and entire image
are visible without horizontal overflow. Web 203 unit tests and typecheck pass. The browser
harness rebuilt production assets successfully, retaining the existing bundle-size warning.
No real workspace, paid model, credential, remote or push action occurred. Overall goal stays
open; this is bounded keyboard evidence, not a claim that all accessibility requirements pass.

### 2026-09-09 — Bounded dataset thumbnail rendering

Code inspection found that the conversation workspace rendered every dataset image as a
button/image/span. Native lazy image loading did not bound that DOM. A browser-only 1,001-row
TEST metadata fixture reproduced 1,001 buttons where the last page should contain 17.
ConversationImages now slices the existing dataset index into 24-image pages; it does not
introduce another dataset, query or ownership model. Paging the strip does not select an image,
change the canvas, write business state or request inference. Selecting an image still uses
the existing guarded URL navigation. A selected-object change resets the strip to that image's
page; this also fixes a separately reproduced browser Back failure caused by stale browsing state.

The two journal/large-index browser tests pass (7.8s harness, isolated workspace
`/tmp/annotagent-guided-e2e-96130`), checking bounded DOM, previous-page browsing without selection,
exact image selection, Back/Forward and reload. Web 203 unit tests and typecheck pass, and the
browser harness production build passes with the existing size warning. Screenshot evidence is
under `/tmp/annotagent-large-images-evidence`; the large-index image uses the app icon as an
explicit layout fixture, not real imported data, inference output or a model-quality example.

Limit: the existing image-index API still transfers the full metadata list, and thumbnails
still reference image assets rather than server-generated small previews. This increment proves
bounded thumbnail rendering, not server-side pagination, lower index-transfer bytes or a measured
end-to-end speedup. Those remain relevant to the broader performance audit. No real workspace,
Provider, remote or previous repository PNG was changed; no push or Live/human claim.

### 2026-09-09 — Current-tree full Rust regression checkpoint

Verified the current implementation after the export/navigation/keyboard/thumbnail increments.
The following exact chained commands completed with exit 0 (not just compilation started):

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

All executed Rust tests pass. Explicitly ignored billable Provider and legal-weight-dependent
real model tests remain ignored: this does not claim all live backends were exercised. The
application suite reports 145 passed/one billable ignored and the separate offline advisor
integration passes; process-death, storage and other workspace suites were included in the
full command. No ignored tests were force-enabled and no old conversation credential was used.

The default-entry screenshot now honors the isolated evidence directory. Current-tree browser
checkpoint passes the existing default-entry test and both complete bbox/classification-review
paths: 3 passed, 31.2s harness, workspace `/tmp/annotagent-guided-e2e-97103`, screenshots under
`/tmp/annotagent-regression-checkpoint`. The paths include real Rust persistence, TEST model
transport, human canvas/Review operations and verified archive downloads; they are not live
accuracy or human usability trials. Production Web assets were rebuilt by that harness.

This is a regression checkpoint, not completion: full-browser-suite evidence still needs a
non-overwriting screenshot strategy for the remaining old hardcoded paths. Existing modified
repository screenshots were preserved. Other open acceptance items include the full requirement
matrix audit, metadata/thumbnail transfer performance and unexecuted assistive-technology,
native IME, actual browser zoom and human/live-conditional checks. No push or remote change.

### 2026-09-09 — Non-overwriting full-browser evidence routing

All 104 explicit screenshot option paths in the existing E2E tests now pass through one
test-only isolatedEvidencePath helper. An AST-driven mechanical rewrite changed only those
expressions and imports (including locator screenshots); assertions, fixtures and product
code are unchanged. With ANNOTAGENT_E2E_EVIDENCE_DIR set, legacy docs/execution paths retain
their subdirectories under that destination. Explicit paths already outside that tree remain
unchanged. Without the environment variable, existing output behavior is preserved.

Path-boundary assertions pass in Node; two equivalent Playwright checks are part of the full
suite. Web 203 unit tests/typecheck pass. The full browser run was started with
`ANNOTAGENT_E2E_EVIDENCE_DIR=/tmp/annotagent-full-browser-evidence npm run test:e2e` against
`/tmp/annotagent-guided-e2e-97754`. It contains 166 tests and is still running at this checkpoint;
the first 14 pass, including a real rate-window wait, client disconnect and stop recovery.
Do not interpret this checkpoint as a full-suite pass. The live command handle is 79494;
continuation must poll that handle or inspect its authoritative process state before restarting.
Output files observed under the temporary root confirm that old repository screenshots were
not used as the new evidence destination. Full-suite results and failures will be recorded after
termination. No real workspace, Live provider, credential, remote or push action was used.

### 2026-09-09 — Visual assistance contract audit during full regression

Inspected the actual `ConversationHumanRequestInput`, Web `HumanRequest` type and
`ConversationSampleCanvas` submission path. This request contract requires an outcome ID and
the canvas rejects a request if that ID is absent from terminal results. The structured answer
uses the existing SampleFeedbackRevision. This proves an existing-candidate correction path,
not a generalized point-identification or candidate-comparison request protocol. Candidate-free
visual assistance therefore remains a concrete gap in the requested interaction coverage; do
not label all suggested request kinds as implemented based on the correction golden path.
Separate Schema clarification, class-scope and setup/authorization cards retain their existing
contracts; this audit does not remove them or alter persisted requests.

The same full E2E command (handle 79494) is still live, with the first 43 tests passing, including
future-rule lost ACK, stop, independent manual draft and cross-tab cancellation scenarios. No
restart or source/product change was made to the active test run. Its terminal result is pending.

### 2026-09-09 — Full-suite failure: unbounded default model selection

The ongoing full run reached test 56 with two failures (future classification/bbox Schema UI,
tests 46/47). Trace response evidence, not just the test's TypeError, identifies the cause:
Journey preview returns HTTP 400, `Choose 1–32 distinct registered image model bindings`.
`ConversationJourneyCard.prepare` defaults to every available image profile/native selection,
while Application correctly enforces 1–32 distinct bindings. The shared TEST registry has now
accumulated more than 32 available choices. This is also a real UI edge case for a populated
registry, not grounds for increasing the authorization bound or treating the error as consent.

Required follow-up: bound/explicitly resolve the default selection before preview, expose an
actionable model choice state and keep the server cap. Tests must assert preview status before
dereferencing consent and cover a >32-model registry deliberately. Do not merely remove models
from the fixture to hide the product edge case. Failure traces currently remain at
`web/test-results/conversation-future-schema-{c281f,966bc}-on-one-plan-for-the-new-one-chromium/trace.zip`.
The current 166-test run remains active on handle 79494; no product rebuild or second suite was
started against its server. Full terminal totals and subsequent regressions are still pending.

### 2026-09-09 — Oversized registry fix staged for browser verification

Journey model selection now deduplicates bindings and preserves explicit choices. When the
available registry exceeds 32 bindings and no choice exists, it selects none and opens the
model selection area with an actionable message; it does not send a rejected preview request
or arbitrarily truncate to 32. Users can select up to the unchanged server bound. Starting a
new preview clears any previous preview so an error cannot leave a stale confirmation card.
This adds a necessary choice only for an oversized registry, not another normal-path setup page.

Three new selection unit tests pass (Web 206 total), typecheck and diff check pass. The existing
two failing future-Schema UI tests now explicitly inject 34 browser-only registry choices,
assert no preview before selection, select their one real registered TEST binding, and assert
both HTTP success and exact allowed_models. They no longer dereference an unchecked response.
These updated browser tests are NOT yet run: the earlier baseline suite remains live on handle
79494 and has reached test 57 with the two previously diagnosed failures. No new Web build or
server restart was made while it is running. Follow-up must verify this source change against
a freshly built isolated server after baseline termination; no browser-pass claim yet.

### 2026-09-09 — Additional baseline failures separated by evidence

The baseline run now reached test 77, with five failures so far (not terminal totals).
Test 68 (initial UI clarification) is the already diagnosed >32-model preview rejection.
Test 67 (initial UI classification) failed while image-upload returned repeated HTTP 429
`mutation_rate_limited` responses: its 10-second assertion expired during the fixture's existing
bounded 65-second pre-execution wait. The initial-journey test now observes for up to 75 seconds,
matching the established other conversation tests; production limits/retry behavior are unchanged.
When the registry is oversized, that test also explicitly chooses its own real TEST binding,
instead of assuming automatic all-model selection. Updated tests typecheck; browser rerun pending.

Test 58 (classification storage_blocked repair) timed out waiting for the Builder authorization
checkbox. Its trace has no Builder preview request, only history reads, despite a completed
Playwright pointer click. The captured frame at that click shows the conversation scrolled
back near its top rather than the repair card. This suggests a layout/interaction race but does
not yet prove the trigger. Keep the trace and reproduce separately; neither increasing its
240-second timeout nor weakening the consent assertion fixes a lost click. Later corresponding
storage_corrupt and draft_changed cases passed. No cause or fix is claimed for test 58 yet.
Same baseline handle 79494 remains live; no restart, rebuild, paid call or real data change.

### 2026-09-09 — Pre-admission stop assertion corrected from exact trace evidence

Baseline test 86 failed because it assumed the first history item was its cancelled operation.
The actual response is HTTP 400: `operation was stopped; no new work was admitted`. The history
before/after is byte-identical; its completed item belongs to the preceding successful Builder.
Pre-admission cancellation correctly creates no new Builder session/receipt. The test now checks
that rejection text, unchanged complete history, absence of its exact operation ID and unchanged
call records. It does not relabel the old completed task or require a fake interrupted session.
Typecheck/diff check pass; this corrected browser assertion awaits the post-baseline rerun.

Baseline reached test 114 with eight failures so far. Two additional failures remain to diagnose:
test 88 waits for a reserved standalone-sample call that it never observes; test 104's legacy
image-first flow cannot find its expected goal-page image. Do not group those with the proven
model-count/limit-window causes without inspecting their distinct traces. Tests 81–85 cover
complete bbox/classification/human flows and background Schema disconnect recovery successfully;
tests 89–103 include Batch stop, stop selection, independent task safety, keyboard/pagination and
the two new screenshot-path checks. Same full-suite handle 79494 remains active.

### 2026-09-09 — Baseline complete; exact Project resolution repair

The full baseline terminated with exit 1: **120 passed, 12 failed, 34 did not run (29.8m)**.
The 34 depend on the failed serial guided-workspace creation and are not passes. Full retained
failure traces were copied to `/tmp/annotagent-baseline-mXrJND/test-results` before another run.
Handle 79494 is terminal and must no longer be treated as a live wait.

The retained goal route's 404 exposed a real index/ownership bug: App used only the bounded
dashboard Project list (server default 100) to resolve an explicit Project URL. App now fetches
the exact missing Project summary through the existing query cache, checks its returned ID,
keeps a loading state until resolution, and shows non-404 read failures with a read-only retry.
Late results from another route cannot supply its owner. It does not expand the dashboard limit
or infer another Project from a display name. Web 206 unit tests/typecheck pass.

Fresh isolated verification is now running on handle 7057, workspace
`/tmp/annotagent-guided-e2e-837`, evidence `/tmp/annotagent-fix-verification`, nine selected tests.
The new deep-link test passes: a real Project omitted from a browser-mocked dashboard list loads
through a held/released real summary endpoint, refreshes, and opens its conversation workspace.
The remaining selected model-scope, initial-goal, pre-admission stop and prior failure tests are
still running. No full repair-pass claim yet. Real workspace, original screenshots and remotes
remain untouched; no push or Live model call.

### 2026-09-09 — Targeted verification and independent fixture repairs

Handle 7057 terminated: **5 passed, 4 failed (5.4m)**. Exact Project deep links,
image-first upload, both initial UI journeys and classification storage-blocked repair passed.
The prior lost-click failure did not reproduce; this is not evidence of a product race fix.
Traces are retained at `/tmp/annotagent-repair-traces-rIIyc2/test-results`.

The two future-Schema tests did not inject their oversized registry: the application requests
`/api/model-profiles?`, whereas their glob matched only the query-less URL. They now match the
exact URL pathname, preserving the oversized-list and exact selected-binding assertions.
The Schema test reached a later pre-admission cancellation assertion: server history correctly
contains no new execution. It now checks the cancellation acknowledgement, rejection, unchanged
history and read-only refresh restoring the preceding saved build, not a fabricated interruption.

The standalone sample test's actual preview bound another accumulated TEST model, not its slow
model. Retained receipt proves successful sample inference, not a stuck cancellation worker.
Its cancellation fixture now explicitly edits the existing Draft through the normal revisioned
API to bind its own delayed model, and verifies that exact model in the authorization preview.
This is a test setup change, not a claim to have repaired planner preference or improved quality.

Typecheck and diff check pass. Fresh isolated round two runs on handle 54047 with evidence under
`/tmp/annotagent-repair-round2`, including future Schema, Schema, stop executors and the previously
blocked guided-workspace/local-model/model/ready suites. Results are pending; no full-pass claim.

### 2026-09-09 — Previously skipped management regressions execute

Round two (handle 54047, isolated workspace `/tmp/annotagent-guided-e2e-1466`) terminated:
**47 passed, 1 failed (3.7m)**. All future-Schema, Schema and three executor-stop tests passed.
The full guided-workspace serial group now executes successfully, including versioned Draft
editing, two-tab conflicts, Run/Review ownership, keyboard/compact layouts, export, refresh/SSE,
and Run lifecycle management. Local model setup and ready-to-sample journeys also passed.
The tests' 200-percent reflow boundary is not a claim of actual browser zoom or human testing.
Inspected the 390px future-bbox form screenshot; field labels and actions are visible.

The sole model-connection test expired at its 10-second assertion while the shared fixture's
bounded pre-execution rate-limit handler was still running (`fetchWithinMutationLimit`). Its
trace failed to finalize; retained error/screenshot are in
`/tmp/annotagent-round2-traces-yExPXu/test-results`, not a complete network trace. The test now
uses a 75-second observation window and 180-second total bound, retaining exact probe counts,
model setup cancellation/refresh and no implicit inference assertions. Production retry, rate
limits and consent have not changed. Single-test verification runs on handle 45386; pending.
Web unit tests freshly pass **206/206 (42 files)**; typecheck passes.

Candidate-free IdentifyTarget remains an implementation gap: both server terminal validation
and the canvas require an existing outcome ID. Existing candidate corrections and separate
Sandbox additions do not establish that request/continuation capability. No completion claim.

Single model-return verification terminated successfully: **1 passed (9.7s)**, isolated
`/tmp/annotagent-guided-e2e-2127`, evidence `/tmp/annotagent-model-return-verification`.
All baseline failures now have passing targeted verification, but that is not a combined-suite
pass. A fresh full `npm run test:e2e` has been started with evidence isolated under
`/tmp/annotagent-full-browser-round2`; await its terminal result before claiming full regression.
Local changes in this repair sequence: `9ef54e0`, `be0fe85`, `ba496a3`. No push, no remote change.

### 2026-09-09 — Final-only comparison correction (browser verification pending)

Full round two is confirmed live on handle 21905, isolated workspace
`/tmp/annotagent-guided-e2e-2275`, 167 tests. It uses the build from before the change below;
do not attribute its results to later source edits, or rebuild/restart its server mid-run.

Source audit found that SampleFeedbackEditor gated its original comparison on projection
existence but then drew `originalSample.outcomes`, which can include intermediate detections.
Both original and current editor predictions now use the same terminal-only helper, selecting
final/review projections, deduplicating candidate IDs and retaining the saved test provenance.
Absent projection and a no-target empty projection produce no invented annotation. Existing
human feedback is still applied separately; no accepted annotation or model/geometry rule changed.
The regression was first observed failing, then passes with coarse+final+review mixed input,
duplicate candidate, legacy absence and empty terminal cases. Typecheck and **207 unit tests**
pass. Browser comparison verification requires a fresh isolated build after handle 21905 ends.

Prepared `sample-terminal-comparison.spec.ts` as a browser regression for that change. It creates
an actual isolated TEST sample, then explicitly perturbs browser read responses with an extra
coarse aggregate and a test-only comparison link. It checks current/before canvas counts, empty
terminal results, and zero writes during navigation/reload. This transport perturbation is not
a real revised-plan lineage or accuracy demonstration. Typecheck passes; browser execution is
pending so it is not counted as passed. Full handle 21905 remains live (last observed test 31),
and its pre-change server has not been rebuilt or interrupted.

### 2026-09-09 — Reference-target request backend foundation

Human requests now distinguish an existing optional `outcome_id` from a new optional
`addition_id`. Exactly one is permitted. Existing serialized candidate requests remain readable;
new reference requests require their own UUID and `identify_target` reason, never an invented
model outcome. Application validation still checks the exact saved Sandbox/image hash/Project.
Storage binds the answer to the same target and feedback sequence, reuses existing missing-target
feedback validation, and commits feedback plus the resume outbox atomically. Another request
cannot reuse the reference identity; an exact retry restores its original record.

New Storage regression covers foreign/ambiguous subjects, wrong-answer rejection, one feedback
and one resume on duplicates, unchanged prediction, conflicting target and database reopen.
New Application regression covers a candidate-free image, changed pixels, foreign ownership and
invalid identity. Existing correction/feedback tests adapt to the optional ID without substituting
another candidate. **This is not yet the complete IdentifyTarget UI/Agent feature**: the canvas
truthfully rejects a new-reference request for now; no new default request is generated and no
fake candidate/working button was introduced. Next step is the actual reference drawing and
request creation/continuation integration, with browser evidence.

`cargo clippy --workspace --all-targets --all-features -- -D warnings` and
`cargo test --workspace --all-features` completed exit 0 (handle 38102). Application: 146 passed,
one explicitly gated billable smoke ignored; other Live-gated tests retain their exclusions.
Web typecheck and **207 unit tests** pass. Cargo fmt/diff check pass. No updated server/build
has been launched into the running full-browser baseline, so it does not verify this contract.
Full browser handle 21905 is still live, observed through test 74 with failures at 49
(independent goal selection) and 67 (multi-label class canvas). Causes still need inspection;
do not call the suite green or silently restart it. No push or real workspace modification.

### 2026-09-09 — Reference-target canvas integration (verification pending)

The existing SampleFeedbackEditor now accepts either an exact candidate or reference target.
Reference editing reuses the same annotation canvas, label field, feedback payload, idempotent
answer API and continuation callback. The user explicitly adds a reference and must change its
box geometry or enter its category before submission; an untouched seed cannot be submitted.
Only box/category reference editing is exposed. Existing terminal predictions remain separate.
Undo can remove the seed and allow a new attempt; restored feedback selects the saved addition
identity instead of another candidate. Saved/deferred requests retain their owned sample context.

Two pure readiness tests plus the existing suite pass: **209 tests / 43 files**, typecheck and
diff check pass. Added actual-service E2E cases for box/category request creation, seed blocking,
submission, duplicate answer, reload/checkpoint and unchanged inference history. These are not
yet run: the old full-browser server remains active on 21905 (observed through test 94), and
must finish before the new build is tested. Automatic request generation for no-target evidence
is still pending; these explicit API-created TEST requests do not prove that Agent behavior.

### 2026-09-09 — Full round two ended; new contract browser verification starts

Handle 21905 is terminal: **129 passed, 4 failed, 34 did not run (23.7m)**. The full retained
failure evidence is `/tmp/annotagent-full-round2-traces-o1KGja/test-results` (copied before the
next run). Failures: goals, multi-label class canvas, guided creation, and journey-ready.
Guided creation still found its ID by name in the bounded dashboard; the test now uses the
creation URL's ID and verifies its exact summary. Goals' retained trace proves HTTP 429
`mutation_rate_limited`; its lost-ACK route used `continue` on later requests, bypassing the
fixture pacing layer. It now falls back to that layer and uses its bounded observation window.
Typecheck passes; neither fix is counted as browser-passed yet.

The class-canvas failure shows no preview request and remains to reproduce; the independent
journey-ready test exhausted its total 120 seconds at the later missing-annotation action.
Do not assume a shared cause without evidence. The serial 34 skips are not passes.

Fresh new-code verification runs on handle 75462 with isolated evidence
`/tmp/annotagent-reference-verification`: both reference target UI paths, the terminal comparison
transport regression, goals, and the prior multi-label canvas failure. This run includes the
new Rust contract and Web build; unlike the full baseline it can validate the new implementation.

Handle 75462 terminated successfully: **5 passed (23.8s)**, isolated workspace
`/tmp/annotagent-guided-e2e-4983`. Both real request/answer/reload/checkpoint reference paths and
the explicitly perturbed terminal-comparison transport passed. Goals passed after its fixture
repair. The multi-label class canvas failure did not reproduce; no product race fix is claimed.
Saved screenshots are under `/tmp/annotagent-reference-verification/conversational-workspace`.
Still required: automatic evidence-driven reference requests, guided full-group verification,
journey-ready investigation, and a final combined regression. No Live/accuracy/human-test claim.

### 2026-09-09 — Evidence-driven reference assistance

Existing post-sample assistance now prepares a stable reference request when no terminal
candidate survives and semantic/geometry failure evidence exists, provided the saved scope
contains a supported box/category Schema. Normal no-target results, missing scores alone, and
infrastructure/Provider/budget/invalid-artifact failures do not trigger reference work. The
question describes the evidence limit, invites a reference only if the user can identify it,
and offers deferral otherwise. It does not infer a hidden target or accept a dataset annotation.

Application regressions (6) pass. The end-to-end application scenario saves a candidate-free
quality-failure report in a temporary workspace, restarts through assistance recovery, obtains
one request, saves the answer, and verifies copy/outbox crash recovery without inference. This
uses explicit TEST report evidence, not a Live model-quality demonstration. Browser automatic
trigger validation remains pending; the explicit reference canvas paths passed previously.

Management verification handle 2585 terminated: **36 passed, 1 failed (1.3m)**, workspace
`/tmp/annotagent-guided-e2e-5194`, screenshots `/tmp/annotagent-management-round3`. Guided serial
management tests all ran/passed. Journey-ready reached shape editing but its four handles
disappeared: the retained detail response contains the TEST polygon, while the rendered UI
returns to the original classification. Review's queue refresh can overwrite a later detail
response. Preserve the trace before another run and fix this read-order race, not the assertion.

### 2026-09-09 — Review detail hydration and queue ordering

The first queue-order fix alone did not solve the browser regression (handle 18377, one
failure). Further inspection found the editor initialized only on selected ID changes: a
queue entry could initialize it before the exact same-ID detail arrived. This is a second,
independent hydration issue, not evidence that the response merger was sufficient.

Review now preserves detail responses newer than an in-flight queue request and hydrates a
pristine editor when its same-ID annotation changes. Geometry, attribute and decision edits
prevent that hydration; successful saves establish the new baseline. Existing project/request
generation guards, decision services and ownership checks remain in place. No new write API.
Seven focused unit cases cover queue ordering, pagination, fresh refresh, detail hydration and
unsaved input protection. The existing shape browser regression failed before hydration and
passed afterwards: handle 85864, **1 passed (34.4s)**, isolated workspace
`/tmp/annotagent-guided-e2e-6102`, evidence `/tmp/annotagent-review-detail-verification`.
The shape responses are explicitly UI-only TEST substitutions, not saved model predictions.

Current-source full verification is running: Rust fmt/clippy/test/build handle 7814 and
combined browser handle 87265 (evidence `/tmp/annotagent-full-browser-round4`). Their results
are pending, not a green-suite claim. Live model quality and real human usability remain
unverified. Real workspace, existing screenshot edits, remotes and published user data were
not changed.

Verification checkpoint after `586a7b5`: Web typecheck and **216 unit tests / 44 files** pass.
Rust handle 7814 terminated with exit 0: fmt, strict all-target/all-feature clippy, complete
all-feature workspace tests and build all pass. Existing explicitly ignored external/billable
tests remain ignored (Application: 148 passed, 1 ignored); this does not validate Live quality.
The 170-test browser round remains running on handle 87265 in isolated workspace
`/tmp/annotagent-guided-e2e-6319`. Its final outcome must be collected before another browser
run clears traces. Do not poll completed handles 18377, 56861, 59327, 85864, 40919 or 7814.

### 2026-09-09 — Bounded browse preview transfer

The image-index response now advertises an optional `thumbnail_url`. ConversationImages uses
that URL for its 24-item strip, while the image's original `url` and annotation canvas remain
unchanged. The Rust endpoint reuses image-tools loading/thumbnail/PNG encoding; there is no
model call, derivative workspace file or second image entity. Project ownership and path safety
are resolved through the existing Application method before returning image bytes. Decoding
runs on the blocking pool with two permits held until actual worker completion (including after
client disconnect). Input limits are 32 MiB and 16 million pixels; output is at most 256px on
either axis without upscaling. Oversize/unreadable images fail explicitly; no fake preview.

The server integration test failed first for the missing URL, then passed after implementation.
It checks PNG dimensions, rejection of a real different Project owner, original bytes unchanged,
and a 404 after isolated image removal. Web rendering tests check preview selection and legacy
index fallback without altering the original URL. Typecheck plus **218 tests / 45 files** pass;
targeted server test, fmt and strict server all-target/all-feature clippy pass (handle 5436).

Limitations: previews use private no-store responses, not a generated-cache claim; metadata still
loads as a complete index. No end-to-end latency improvement has been measured. The running
170-test round on handle 87265 predates this preview change and cannot verify its browser
integration. Wait for that round before rebuilding/restarting its server or overwriting traces;
then verify preview network bytes/dimensions and unchanged full-resolution canvas in a fresh run.
No real workspace data, original PNG edits, Provider or remote was modified. No push.

Preview evidence tightened: the earlier server fixture was only 160px and proved the size cap,
not downscaling. It now imports a 640px TEST image and asserts exactly 256px output while original
bytes remain unchanged; the server regression passes (handle 35419). A new browser test
`conversation-previews.spec.ts` checks thumbnail intrinsic dimensions, full-resolution canvas
source, and read-only reload. Playwright discovery passes, but execution is pending until the
existing combined run finishes; discovery is not browser evidence. The prior goal turn changed
implementation and tests; this increment strengthens the acceptance test rather than claiming
a new model-quality result. Combined run 87265 is still live (most recently tests 44–47 passed).

### 2026-09-09 — Combined-run human Schema retry failure

Round 4 test 50 failed after the saved-label lost-ACK simulation. The captured transport shows
the initial save returned 200, followed by a retry with `mutation_rate_limited` 429; the page
retained the frozen inputs and offered retry. The custom test route used `continue()` on the
retry, bypassing the standard fixture's bounded pre-admission pacing. Archive:
`/tmp/annotagent-human-schema-failure-nyMoqu/trace.zip`; relevant body resource
`ca363da5321ce89e7b9ce0860b9ea8084a9db042.json` explicitly names that guard.

The test now uses the existing bounded helper for its intercepted save, requires successful
persistence before simulating lost ACK, and falls back to the fixture on retries. Its two
save observations allow the existing 65-second pacing window; the test has a finite 180-second
ceiling. No production retry policy or security limit changed. Test discovery passes; fresh
execution is pending after round 4 finishes, alongside the preview browser test. The round is
still live on handle 87265 (test 58 completed); do not restart it or label the suite green.

### 2026-09-09 — Reuse dataset queries on conversation context changes

Inspection found the workspace bootstrap re-fetches image metadata for every task/Draft/request
context change. It now uses the existing project-keyed RouteQueryCache with a 30-second freshness
window, not a second cache or dataset. The existing outer AbortController still guards application
of results from old contexts. Upload invalidates the key before any file (including partial failure)
and forces a fresh load on completion. Browser reload creates fresh memory state; server ownership,
content hashes and authorization checks remain authoritative.

The pending preview browser test also counts index requests across a saved message/conversation
URL change. Typecheck and 218 Web unit tests pass; this is not yet proof of the new browser request
count assertion. Full Rust verification after preview implementation (handle 76344) exited 0:
fmt, strict all-target/all-feature clippy, workspace all-feature tests and build. Existing ignored
real-model/paid tests are still not Live evidence. The current browser round predates this cache
change and is live through test 78, with the recorded test-50 failure. Do not rebuild web/dist
mid-run: the server uses ServeDir over that actual directory, so rebuilding would change its
test subject. After completion archive traces, then run new preview/cache + repaired human Schema
tests on the fresh source. Metadata remains an all-image index, not server-side pagination.

### 2026-09-09 — Bound historical task context fan-out

Conversation assistance previously started a Promise.all over every task, each loading human
requests and processing records. The existing loaders now run through four read workers. Result
order and all task records are preserved; cancellation or a read failure stops queued admission.
The same AbortSignal still reaches the real APIs and old-context updates remain guarded. There
are no model actions, retries or new persistence entities in this queue.

Three tests run against the previous Promise.all behavior failed (7 concurrent reads instead of
3, admission after cancellation, and admission after failure); they pass with the bounded queue.
Web typecheck and **221 tests / 46 files** pass (handle 3792). This is a bounded-concurrency claim,
not a completed long-history pagination or total-transfer reduction claim. Full message loading
and historical context pagination still need attention. Browser round 87265 predates this change,
remains live through test 121, and has the already recorded test-50 failure; fresh preview/cache
and human Schema verification remains next after its completion.

Follow-up failure-state fix: a failed context read left `requestsReady=false` and the Refresh
requests button disabled indefinitely. The component now distinguishes loading from failed reads,
shows the actual read error, and enables explicit read-only retry. It never presents a failed read
as an empty queue. An SSR regression failed on the old loading/disabled output and now passes;
typecheck and **222 tests / 46 files** pass (handle 31860). No model retry was added.

Round 4 is still live, but test 122 (Automation Recipe) timed out and skipped its later serial
management cases. Inspect its saved trace before attributing the failure or changing timeouts.

Round 4 handle 87265 terminated: **134 passed, 4 failed, 32 not run (24.2m)**. Complete traces
were copied to `/tmp/annotagent-round4-traces-bffAzU/test-results` before any new run.
The four failures are human Schema lost-ACK pacing (patched, awaiting fresh run), Pipeline
Builder disabled with empty target selects, dataset results missing manual-add controls, and
Expert Worker setup unable to select the new Project. The latter three are not simply slow
timeouts: their snapshots expose missing Project data outside the first dashboard page.

The parent already resolves an exact routed Project, but WorkflowsPage and Batch/Run consumers
received only the paginated dashboard array. Those scoped consumers now also receive that
verified Project (deduplicated by stable ID, never inferred from a name). This preserves normal
management inventory pagination. Expert setup is global and still needs actual project-page
navigation; adding a routed owner alone cannot fix that selector. No timeout increase is used
for these missing-data cases. Fresh targeted browser checks and final combined checks remain.

### 2026-09-09 — Global setup Project paging and exact scoped regressions

Expert setup now reads the existing paginated Project endpoint, exposes Load more Projects,
deduplicates stable IDs and retains the user's selection while appending. Failed page reads have
an inline retry. Project-image requests are cancelled/guarded on selection change so a late old
list cannot replace the new Project's images. No new registry or automatic model invocation.
The Expert browser case explicitly loads pages and selects its own Project before sample testing;
it no longer merely asserts the name exists while testing whichever Project happened to be first.

Fresh verification handle 68814 finished **5 passed, 1 failed (48.3s)**. Both repaired human
Schema cases, journal/large-index cases and journey-ready passed. Preview failed because the test
incorrectly expected Save message to navigate to a task URL. Saving a note is not selecting a goal;
the test now explicitly selects the saved message before checking context-query reuse. Trace:
`/tmp/annotagent-preview-trace-T6nyTi/trace.zip`. PNG dimensions had passed before that assertion.

The Pipeline and Batch tests now intentionally omit their real Project from the dashboard GET
while keeping exact summary/entity APIs intact, exercising the routed-owner bug even in a small
isolated workspace. Current-source targeted run is handle 21777, evidence
`/tmp/annotagent-paging-repairs`: preview, complete guided serial group, journey-ready and Expert
integration. Results are pending. Web typecheck and 222 unit tests passed before these additional
browser assertions. No full-suite success or Live/human-quality claim.

The 41-test run 21777 finished **38 passed, 1 failed, 2 skipped (2.7m)**. Preview/cache, the
entire guided management group, and journey-ready passed, including forced dashboard-owner
omission. Expert setup had loaded the correct Project and pixels but its implicit selector label
did not satisfy the exact accessible-name lookup. The select now has an explicit translated
Project aria-label. Fresh Expert-only run 58308 passed **3/3 (6.2s)**, workspace
`/tmp/annotagent-guided-e2e-10015`; previous trace is
`/tmp/annotagent-expert-picker-trace-tUv5DR/trace.zip`. Typecheck passes. These are TEST Worker
transport/discovery/Artifact tests, not Live SAM weight or accuracy evidence.

Visual inspection of `/tmp/annotagent-paging-repairs/conversational-workspace/bounded-browse-preview.png`
also shows the desktop composer below the initial viewport as historical cards grow. A viewport
assertion was added to the preview test; baseline verification is running on handle 79098
before restructuring the scroll areas.

### 2026-09-09 — Keep the desktop composer visible

Baseline handle 79098 failed with textarea viewport ratio 0. The desktop conversation panel
now has a separately scrolling history area and a natural-height composer row. At small/short
viewports the original document-flow layout remains, avoiding fixed-height clipping. Existing
Human Request spacing follows the new wrapper; no task state or event handling was copied.

Typecheck and 222 unit tests pass. Fresh real-browser run 12566 passed **5/5 (8.6s)** in
`/tmp/annotagent-guided-e2e-10349`: human bbox/category labels, preview/cache/visible composer,
journal keyboard/narrow layouts and large image strip. The resulting screenshot was inspected:
`/tmp/annotagent-composer-repairs/conversational-workspace/bounded-browse-preview.png` visibly
retains both textarea and send action at the bottom of the left pane. Previous failed trace:
`/tmp/annotagent-composer-trace-M6OrBQ/trace.zip`.

The stronger full-control visibility checks at 1440×900, 1280×720 and 1024×768 are now running
on handle 47082 (`/tmp/annotagent-composer-viewports`). These are real viewport checks, not
native browser zoom, assistive-technology or a human usability study. Handle 47082 completed:
**1 passed (6.0s)**, workspace `/tmp/annotagent-guided-e2e-10444`; all three viewport checks passed
and their screenshots were saved under `/tmp/annotagent-composer-viewports/conversational-workspace`.
No tool process remains running from these targeted checks. Broader history pagination and the
final combined regression remain outstanding.

### 2026-09-09 — Bounded reverse journal reads (backend increment)

The server now supports `messages?latest=true&limit=N`, an exclusive `before=sequence`
cursor, and exact `messages/:messageId` reads using the existing project-owned journal.
Pages are capped at 100 and returned in chronological order. The original forward
`after` API is preserved. Conflicting directions and non-positive reverse cursors are
rejected. These GETs do not create tasks, execute models or write annotations.

The new server regression first failed against the old API (400 instead of 200).
After implementation, the storage test verifies the 100-item cap, stable backward
pages after an append and database reopen, and ownership rejection. The server test
verifies tail/backward/forward reads, missing exact IDs, invalid cursor combinations,
and rejection of an existing different Project's attempt to read the message.
Both targeted tests pass. Expanded verification completed successfully: all four storage
conversation tests, all 47 server unit tests, formatting, and server all-target/all-feature
Clippy with warnings denied. No external model inference was used.

This is deliberately not claimed as a visible pagination improvement yet: the current
React bootstrap still drains forward pages. Frontend integration must preserve the
selected task's source message, frozen candidate references and stop-task context even
when those messages are outside the visible history page. No pagination button or
performance claim has been added ahead of that integration. Real workspace data,
historical screenshot modifications and remotes remain untouched.

### 2026-09-09 — Frontend history window and exact task context

React now opens the latest 100 journal messages, with an explicit earlier-message
read. Selected task sources and frozen message references are fetched by exact ID
when outside that page and kept separately from the visible journal. Loading older
messages does not choose a goal or change the URL. Source read failure is surfaced,
not replaced with a newer message. Cancellation prevents old reads from committing.

The historical no-selected-task fallback remains the oldest unscoped message, not
the newest visible goal. It reads from the beginning until that first goal is found;
this normally adds one bounded read, but a journal containing only reference/stop
notes can still require scanning. This remaining exceptional scan is not described
as fully bounded. Historical task auxiliary reads also remain concurrency-bounded,
not fully paginated. Explicitly loading all earlier pages can grow the DOM; the
initial page no longer does so automatically.

Typecheck and all 226 Web unit tests pass. Browser run 51536 passed all four tests
in isolated `/tmp/annotagent-guided-e2e-11428`: a 250-message metadata fixture opens
100 notes, reads older pages only on click, restores the tail on reload and emits no
API writes; the existing journal/image recovery, bounded previews and large image
index regressions also pass. Run 31614 passed both real HTTP/Fixture human-reference
flows (bbox and classification). A stronger rerun additionally hides the selected
source behind 100 later metadata notes and checks exact source reads across reload.
That stronger run 80538 passed **2/2 (6.5s)** in
`/tmp/annotagent-guided-e2e-11710`; screenshots are isolated under
`/tmp/annotagent-history-reference-exact`. Production build succeeded during these
runs, with the existing large-chunk warning. These tests do not claim Live model
quality or real human usability. Full combined regression remains outstanding.

### 2026-09-09 — Current-source completion audit and combined round 5

Previous increment was progress (committed runtime-backed history paging), not a wait
or a completion claim. At `32eb846` the Rust command chain completed with exit 0:
`cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && cargo build --workspace --all-features`.
Handle 16894 is terminal. Existing explicitly ignored external-weight/paid tests remain
ignored; successful workspace tests are not evidence of Live SAM or Provider accuracy.

The complete browser suite is running on **36529**, using the actual process-confirmed
workspace `/tmp/annotagent-guided-e2e-11895`, service 8791, screenshots under
`/tmp/annotagent-full-browser-round5`. First 18 tests passed at this checkpoint; this
is not the suite total or a green final result. Continue polling that same handle.
Do not start another browser suite or rebuild its Web assets until it is terminal.
No product source was changed during this combined run.

Re-read attachment sections 3–18 and checked current source, rather than treating old
status prose as proof. Current evidence boundaries:

| Requirement area | Inspected current evidence | Remaining conclusion |
| --- | --- | --- |
| Goal → Schema → Builder → Sample with one initial consent | `conversation-initial-journey.spec.ts` exercises UI and actual child service receipts, including clarification | Current combined result pending; Fixture only |
| Canvas answer → Revision → checkpoint | `conversation-samples.spec.ts` checks lost ACK, exact revision, duplicate answer and resume Draft; `conversation-reference-target.spec.ts` checks bbox/category targets | Proven targeted tests; all-suite result pending |
| Correction → autonomous bounded improvement | `conversation_human_requests.rs::continue_conversation_correction`, `ConversationRepairCard.tsx`, and sample browser test | Current answer prepares a Draft locally; Builder repair and subsequent sample each use a separate authorization. Not equivalent to automatically continuing an already-authorized improvement allowance |
| Initial authorization scope | `ConversationJourneyConsent` binds concrete Builder and sample operation IDs, model/image scope, call bounds, expiry; only clarification continuation is explicitly permitted | Do not reinterpret existing saved grants as permission for new repair operations. A bounded opt-in continuation needs an explicit scope design using existing services |
| Formal processing and export | Sample test source verifies immutable batch scope, lost start ACK, duplicate operation ID, Review save failure, real JSON file/archive and durable export events | Broad source assertions exist; await their execution in round 5 |
| Reference assistance without a candidate | Application `needs_reference_target` requires semantic/geometry evidence and excludes infrastructure/provider/budget failures | Actual bbox/category reference path exists. Do not claim generic point/comparison request types from it |
| Large journal and images | Tail paging, exact context reads, bounded thumbnails and cache regression | No-task reference-only scan and task auxiliary pagination remain limitations; no measured end-to-end latency claim |
| Accessibility | Real viewport/keyboard/reduced-motion tests; simulated composition events | Native 200% zoom, actual OS IME and assistive-technology testing remain unexecuted |
| Quality / usability | Explicit TEST transport and real local persistence | Live inference/accuracy and real novice study remain unexecuted, not replaced by fixtures |

The automatic improvement continuation row is a fidelity issue to resolve explicitly,
not permission to increase budgets or silently extend old consent. The overall goal
remains active. No remote changes, push, credentials or real workspace mutations.

### 2026-09-09 — Repair continuation reuse, verified while round 5 runs

Round 5 handle 36529 is still live; tests 1–31 have passed. The previous Rust handle
16894 is terminal/successful and must not be restarted merely to observe it again.

Current-source tracing identifies the exact integration seam: joint Journey preview,
save validation and dispatch hard-code `BuilderSelection.repair_request_id: None`.
The standalone Builder already resolves an owned `ConversationBuilderRepair` containing
request ID, Draft ID, revision and content hash. Its scope hash includes that evidence;
launch rejects changed evidence, and persisted-operation retries compare the full frozen
execution hash. Journey's existing sample seal already prevents changing the generated
Draft/model/image allowance on retry. Reuse these services and gates.

Important implementation constraint: storing only a request ID and re-resolving its latest
Draft at retry is insufficient. The Builder itself may have advanced the editable Draft;
the original repair snapshot must remain part of the acknowledged consent/child request.
Old consent JSON must deserialize without any new repair permission. Initial Schema
proposal and exact repair are distinct sources and must not be accepted together. One
post-correction joint confirmation can replace the current two confirmations, but does
not by itself prove pre-authorized automatic improvement after a future answer; that
broader allowance still requires explicit bounded scope, expiry and shared budget.

Required follow-up tests are exact repair snapshot preservation, changed-source rejection,
lost acknowledgement and restart, revoked/expired permission, sample model/image expansion
rejection, and no repair inference from GET/reload or legacy consent. No product rebuild or
source mutation was made against the currently running round 5 server.

### 2026-09-09 — Round 5 first failure: long-card screenshot positioning

Test 39 failed at its last viewport assertion, after its Schema save, revision/diff,
unchanged original state and no-repeat-call assertions had passed. Error context reports
`Review build and sample authorization` viewport ratio 0 after centering the entire
long Future rule card. Inspected the real failure PNG: saved state and comparison occupy
the independently scrolling history; the composer remains visible and the next-action
button is below that scroll viewport. Centering a card taller than the history cannot
prove all descendants fit simultaneously.

Trace, error context and PNG are preserved at
`/tmp/annotagent-round5-future-trace-pDvudU/`. The test now captures the saved-state view,
then scrolls to the actual action, requires full visibility and performs a Playwright
trial click (no authorization or inference). No product layout, permission or persistence
assertion was weakened. This edited test is **not yet rerun**: full handle 36529 remains
live, through test 46 with that single failure. Wait for termination before a fresh run;
do not claim the screenshot fix passed based on inspection alone.

### 2026-09-09 — Round 5 terminal evidence and recovery repairs

Full handle 36529 is terminal, exit 1: **139 passed, 7 failed, 26 did not run (26.1m)**.
All failure artifacts were archived before rerun at
`/tmp/annotagent-round5-traces-MXvleH/test-results`. The seven failures comprise two
long-card viewport assumptions, two missing-reference expectations, a manual-label
request-list wait, a journal upload wait, and a historical dashboard-page assumption.

Missing-reference failure exposed a product bug: exact optional reference GET failed
bootstrap before messages/images were committed, leaving the valid task and canvas in
Loading indefinitely. History loading now distinguishes mandatory task source failure
from optional reference failure. The latter preserves valid context and reports the
error; the canvas's existing exact-scope guard refuses substitution. A new unit test
verifies that distinction; typecheck and all **227 Web tests pass**.

The journal trace shows repeated image-upload 429s during the test fixture's bounded
pre-execution pacing, while its UI assertion expired after 10 seconds. Its observation
now permits that existing 65-second window (75-second expectation), not extra Provider
retries. The manual-label test had the same explicit 10-second override over this suite's
documented 75-second observer; removed that override, but its failure had no saved trace,
so rate limiting is not asserted as independently proven there. Both require rerun.
The historical publication assertion now reads the exact Project summary rather than
assuming its Project exists on the first dashboard page. Long-card tests scroll to the
action and require a full-visibility trial click; all data and permission assertions remain.

Fresh affected-file browser run **80890** is live, evidence under
`/tmp/annotagent-round5-repairs`. It covers future Schema/model proposals, complete sample
flows, journal and Guided Workspace regression. Do not rebuild/restart while it runs.
No browser-pass claim for these repairs yet; changes await that verification/local commit.

During rerun 80890, the previously failing future-model long-card test passed (test 9,
1.8s). Inspected `/tmp/annotagent-round5-repairs/conversational-workspace/future-model-proposal-next-action.png`:
the full next-action button is visible in the history pane and the composer remains
separate and accessible. The optional-reference recovery implementation and its unit
regression are committed as `d483800` (227 unit tests/typecheck passed). Browser/test
adjustments and this record remain pending until the affected-file run completes;
through test 12 no failures have been reported. Handle 80890 remains live.

### 2026-09-09 — Affected-file rerun terminal; all conversation repairs pass

Handle 80890 completed exit 1: **38 passed, 1 failed, 26 not run (7.7m)**. Both long-card
tests, all nine full conversation sample scenarios (including classification-review,
human classification and human bbox), journal upload/recovery and image index passed.
This verifies the optional-reference fix through actual missing/wrong-image deep links,
not merely the loader unit test. The conversation test adjustments can now be committed.

The remaining Guided publication test failed on its Project summary shape assertion.
Source currently destructures `{ project }` from the owned summary response; this line
was corrected after the prior run had begun. A fresh run is required before attributing
failure to a current server bug. Archived its artifacts and other remaining outputs at
`/tmp/annotagent-round5-guided-trace-bMGz5E/test-results`.

An accidental root-directory `npx playwright` invocation (99158) terminated with a test
runner version/config mismatch and ran no tests; package manifests/lockfiles are unchanged.
The corrected command uses `web/node_modules/.bin/playwright` from `web`. Current handle
**46539** runs the complete Guided Workspace file against a fresh isolated workspace,
evidence `/tmp/annotagent-guided-summary-repair`. Do not count either failed invocation
as a passing check. No real workspace/remote/credential changes or push.

Handle 46539 is now terminal/exit 0: **36/36 Guided Workspace tests passed (39.3s)**,
including publication/default-version selection, all formerly skipped Review/export,
keyboard, compact reflow and Run deletion/restore/provenance tests. The fresh runner
confirms the exact Project summary destructuring correction. This is not a new full-suite
green run, and the “200 percent reflow boundary” remains a viewport test, not native zoom.
Conversation fixture/scroll repairs are committed as `ae9a7d0`; source recovery as
`d483800`. The complete objective and automatic repair-authorization work remain open.

### 2026-09-09 — Exact correction snapshot in joint Builder/sample authorization

Implemented the backend seam, reusing the existing Journey executor and Builder.
The shared `ConversationBuilderRepair` DTO now lives in Storage and is re-exported
through Application without changing its wire shape. Journey consent can explicitly
carry that exact request/Draft/revision/content-hash snapshot. Preview resolves it
through the existing owner-checked repair service; acceptance verifies it against the
Builder scope; execution forwards the frozen snapshot to the original Builder launch.
The existing sealed sample scope and task/Project call ledgers remain authoritative.
There is no new model engine or execution route.

Absent repair data remains absent for old serialized grants. Invalid repair snapshots
and initial-Schema/clarification-continuation combinations are rejected. Storage tests
cover restart and exact retry plus changed request, Draft, revision, hash or removed
repair scope rejection. Eight Journey storage tests, all 47 server tests, formatting
and server all-target/all-feature Clippy pass. An initial compile typo in a Result
predicate was corrected before these passing checks.

New isolated browser/HTTP test performs a real canvas answer, persists its checkpoint,
previews and acknowledges one bounded joint repair, then executes Builder and sample.
It checks changed-revision rejection, read-only GET and duplicate execute without added
call records. Initial test setup incorrectly used an unqualified model ID; Registry
correctly rejected it (trace `/tmp/annotagent-joint-repair-trace-sP2MMy/test-results`).
After using the existing `model-profile:` selection format, run 6519 passed 1/1 (6.3s),
workspace `/tmp/annotagent-guided-e2e-19392`. Stronger rerun 78594 additionally asserts
both child operations use the exact correction Draft, not a newly generated substitute.
That stronger run passed 1/1 (6.5s), workspace `/tmp/annotagent-guided-e2e-19490`.

This backend capability is not yet connected to a unified UI repair card, and does not
grant automatic improvement after a future answer under old initial consent. UI scope
matching, restored-history isolation, and any bounded future-answer opt-in remain open.
Fixture execution is not Live inference quality evidence. No push or real data changes.

### 2026-09-09 — Unified repair card and source-isolated restoration (verification pending)

Connected the human-correction repair card to the existing joint Journey component.
Its default action offers one bounded repair/sample consent; the standalone Builder
and separately authorized sample remain available through the existing advanced entry.
Legacy standalone receipts restore that advanced view; a Builder owned by a joint
Journey restores the joint view instead. Current-image class-group repairs still use
their existing standalone flow; no unsupported scope is silently converted.

Journey history and pending browser envelopes now match both exact Schema identity
and the correction request/Draft identity. Normal initial cards reject repair history;
different corrections cannot restore each other's consent. Pending keys include the
repair source and do not fall back to old normal-goal envelopes. Preview rejects a
mismatched repair source. The card explicitly says this consent does not cover future
corrections. One matching helper test was added; typecheck and all 228 Web tests pass.

Browser run **12890** is live (11 tests, workspace `/tmp/annotagent-guided-e2e-19830`,
evidence `/tmp/annotagent-joint-repair-ui`). Backend joint-repair test passed. The new UI
test failed because it used `getByRole(region)` for a named authorization div. The
corrected `getByLabel` locator is saved but the runner had already loaded the old test;
it needs a fresh rerun after this process ends. Existing sample-flow regression is
continuing. No UI-completion claim or commit for this increment yet. No rebuild of
served assets while this run is active, and no real workspace/model/remote changes.

### 2026-09-09 — Unified repair UI verified

Run 12890 completed with **10 passed / 1 locator failure (2.9m)**: all nine existing
sample scenarios retained the standalone advanced flow. That trace is preserved under
`/tmp/annotagent-joint-ui-trace-hX3ChR/test-results`. Fresh run 30326 reached successful
repair execution and reload, but its new test incorrectly expected the original sample
result card to disappear. Existing initial results must remain, so the assertion now
checks distinct exact Draft links for the initial result and the correction result.
That failed test evidence is at `/tmp/annotagent-joint-ui-history-trace-SKmIrF/test-results`.

Fresh run 74368 passed **2/2 (8.7s)**, including actual page authorization, one consent
POST plus one execution POST, no browser-issued child operation POSTs, restored state
with no new POSTs, and separate original/correction Draft identities. Screenshot review
then found the wrapper's advanced button and explanatory text flowing inline. A minimal
grid/gap class fixes that; an explicit bounding-box separation assertion was added.
Final run 75590 passed **2/2 (8.8s)** in `/tmp/annotagent-guided-e2e-20339`. Inspected
`/tmp/annotagent-joint-repair-ui-layout/conversational-workspace/joint-repair-result.png`:
the next action, advanced entry, explanation and composer are visibly separated.
Typecheck, all 228 Web unit tests and production build pass (existing chunk-size warning).

This completes the post-correction single-authorization UI slice, not the whole goal.
Current-image class-group repair and pre-authorized improvement after a future answer
are not enabled by this UI change. Old grants do not acquire those permissions. Final
combined full-suite validation and conditional Live/accessibility reporting remain open.

### 2026-09-09 — Repair sample storage scope fence

Before extending answer-triggered continuation, audited the existing repair envelope.
Application already verifies a completed Builder receipt and exact resulting revision;
Storage's sample seal nevertheless accepted a different Draft under a repair consent.
Added a regression first: `repair_sample_cannot_switch_drafts_or_regress_the_authorized_revision`
failed with `repair accepted draft` (test process 2448, exit 101). This is a storage
invariant gap, not evidence that the current UI had executed an unauthorized Draft.

The sample scope now requires the authorized repair Draft identity and a revision at
least as recent as the approved starting revision. Builder may advance that same copy;
Application still checks its exact final receipt/hash. Once sealed, even a later revision
cannot replace it. Regression checks rejected attempts leave no sample seal, then checks
a valid advancement, database reopen, idempotent retry and rejection of later replacement.

Verification: all 9 Storage journey tests and Storage all-target/all-feature clippy pass
(29199); Application journey regression passes (81827, 1 test). Isolated temporary
SQLite only; no model calls, user workspace changes, remote changes or push. No browser
changes in this increment. Pre-authorizing a pending human answer remains unimplemented;
it must freeze the request/image/feedback revision and resolve only that acknowledged
answer, not authorize arbitrary future corrections. The overall goal remains incomplete.

### 2026-09-09 — Pending-answer authorization storage groundwork

Previous increment is verified progress (2d01fed), not a wait/blocker. Added an optional
`repair_after_answer` envelope holding the exact existing Human Request input: task,
conversation, source sample/image/hash, candidate or addition, feedback sequence and
resume checkpoint. Older JSON omits this field and gains no permission. It cannot be
combined with a completed repair, initial Schema proposal or clarification continuation.
Saving requires the exact owned, active Pending request; Answered/Applied/Cancelled/Stale
requests are rejected for this *new* pre-answer permission. Identical persisted retries
remain readable without granting execution. Sample sealing and dispatch claiming reject
unresolved envelopes; the HTTP consent endpoint explicitly does not accept this new
mode until resolution and event dispatch are connected. No new UI or executable path
is advertised in this storage-only increment.

`pending_answer_permission_is_exact_persistent_and_not_executable` checks all five
request states, changed sequence/checkpoint/candidate/image/conversation, restart,
idempotent save, permission removal and rejection of unresolved dispatch/sample. Its
request row is deliberately seeded in a temporary TEST database to isolate authorization
validation; this is not an end-to-end answer/outbox test. All 10 Storage journey tests
pass (73509), server all-feature check and server/storage all-target/all-feature clippy
pass (84695). No model calls, real workspace mutations, push or remote changes.

Remaining immediate work: resolve an Applied answer into one immutable repair snapshot
using its acknowledged Sandbox feedback and resume result, reuse the existing resolved
consent/dispatch state, then connect server answer events and explicit UI preauthorization.
Do not treat the new persisted envelope as completed automatic recovery. Full integration
must test deferred/cancelled/expired permissions, changed snapshots and duplicate events
without expanding models, data, budget or future corrections.

### 2026-09-09 — Acknowledged answer resolves one immutable repair

Implemented Storage `resolve_conversation_journey_repair` and its Application wrapper.
The wrapper rechecks current data/Registry scope; Storage keeps the original grant and
uses the existing immutable resolution slot. Only the repair snapshot and derived Builder
scope hash may resolve; model/image/Schema/call/expiry/operation scopes cannot change.
The saved request must exactly match the authorized input, be Applied and carry the
expected next Sandbox feedback sequence. Resume result, acknowledged outbox, actual
feedback JSON and copied-plan feedback must agree. The current Draft revision/hash must
match the proposed snapshot. A later feedback sequence prevents initial resolution.
Repeating the same resolution restores it; a different resolved envelope is rejected.
Resolution itself creates no model call, dispatch or new budget; the existing explicit
dispatch claim can proceed only once the pending envelope has resolved.

Extended the temporary-database regression before implementing the resolver. An initial
test compile error (`u64` SQLite binding) was fixed; the semantic red test then reached
the missing-resolution error (52303). Tests now cover changed scopes/revisions/hashes,
revocation, missing outbox acknowledgment, unapplied answer and missing copied evidence,
plus reopen, immutable retries and the dispatch gate. They deliberately seed receipt
rows and are storage protocol tests, not browser or live inference evidence. All 10
Storage journey tests and the existing Application journey regression pass (19406).

HTTP/UI preauthorization and answer-event dispatch remain disconnected intentionally
until their preview, authorization and retry path are tested together. No automatic
resume UI claim, no live calls, no real workspace mutation and no push. Next work is
that server integration, followed by a browser answer-to-repair-to-sample regression.

### 2026-09-09 — HTTP preauthorization and answer-triggered joint execution

Journey preview now accepts one `pending_request_id`, mutually exclusive with a completed
repair or initial Schema call. It returns the owned active request's full frozen input.
Consent saving uses the existing Builder/data preview and Storage exact-request checks.
Execution while the request remains Pending returns the saved status without a worker
claim or model call. After an applied answer, it derives the real repair Builder scope,
checks unchanged call/previous-grant bounds, resolves the immutable consent and uses the
existing journey worker. Cancelled/deferred/stale/unapplied inputs cannot resolve.

Human answer accepts an optional exact `journey_consent_id`; it checks the request link
before saving. After the existing Sandbox/continuation transaction, it invokes the
existing executor. Errors are returned separately as `journey_resume.error`, not as a
false lost-answer error. Answer endpoints now use expensive-action admission rather
than a control-lane bypass (regression failed at 81936 before the security fix).
Normal/other repair cards do not adopt pending-answer history; the matching applied
request's card can recover it via the frozen checkpoint identity.

Browser 4359 passed the new path; final 83395 passes all **3 joint-repair tests (27.9s)**
in `/tmp/annotagent-guided-e2e-22302`. The new test obtains consent over HTTP, changes a
real canvas bbox, and injects the optional consent ID into the ordinary browser answer
request. It verifies waiting makes no calls, answer application automatically reaches
sample success on the same repair Draft, duplicate answer has no added calls and refresh
has no added calls. This tests the real server contract, not a shipped preauthorization
button. Existing post-correction UI scenarios remain green. Server 48 tests + clippy pass
(73355); typecheck and 228 Web unit tests pass (58474); production build passes with the
existing chunk warning. No live Provider, real workspace, remote or push changes.

Remaining: expose this bounded preauthorization in the pending request UI, visibly
restore its status/errors, and close the server-restart gap between answer acknowledgment
and execution admission using durable continuation intent. The current optional body
link plus saved consent permits explicit safe retry but is not yet a demonstrated
restart-driven automatic delivery protocol. The whole objective remains active.

### 2026-09-09 — Pending request UI authorizes and restores answer continuation

The selected active Pending human request now reuses its exact source Sample Schema
and the existing joint Journey card. Users may preview/approve one post-answer repair,
or save their correction without inference permission. The card lists actual planning
and image destinations, image/call bounds, unknown cost and expiry, names the frozen
feedback revision, and excludes future corrections. Saved permission has a visible
revoke action. History restoration associates it only with the exact request/checkpoint;
the canvas receives the acknowledged consent ID, never an inferred latest global grant.
Its button changes to `Submit correction and continue` only after that restoration.

On answer success, the applied request's existing repair card restores the same joint
execution/result. Continuation errors stay on the human request card and do not replace
the editable canvas or turn a saved correction into a failed-save report. The optional
consent ID held in current React request state is not business truth; server history and
server exact-scope validation remain authoritative. No mount/refresh model POST added.

Browser 43879 passed 4/4; screenshot inspection found an unhelpful scrolled-down view.
Adjusted screenshot navigation to show the saved authorization and the actual new sample
canvas. Inspection also removed duplicate waiting text and the unrelated “different
plan” action while a pending permission is active. Final 57258 passes **4/4 (12.4s)**,
typecheck and all 228 Web unit tests, in `/tmp/annotagent-guided-e2e-22869`. Production
build passes (existing chunk warning). The new UI test uses real controls without
request interception: authorize, reload, edit bbox, submit with exact consent, await
saved sample, open its result. Earlier contract test still uses explicit interception
and is not counted as evidence of the new UI control.

Inspected screenshot evidence:
`/tmp/annotagent-pending-answer-ui-verified/conversational-workspace/pending-answer-authorization.png`
and `pending-answer-result.png` in the same directory. Synthetic TEST pixels and fixture
models prove interaction/protocol semantics only, not object-detection accuracy or Live
model behavior. No real workspace or remote changes and no push. Still pending: durable
answer-to-dispatch delivery/restart tests, pre-admission error persistence, complete
combined regressions and the outstanding final acceptance audit.

### 2026-09-09 — Transactional answer intent and startup admission recovery

Migration 0052 adds one delivery intent per request/consent, referencing the real
Sandbox feedback revision. Answer feedback, request state, local resume outbox and
explicit continuation link now commit together. Unknown/mismatched links roll back
the answer transaction. Duplicate answers cannot add or replace an automatic intent.
Expired/revoked permission still allows the human correction to save, but records a
failed continuation instead of silently renewing it.

Claiming the existing journey worker atomically marks delivery dispatched. Startup
(`serve`, after listener binding and Application local recovery) reads pending Applied
answers in pages of 64 and attempts existing scope-validated execution. Any prior
dispatch, including interrupted/in-doubt work, is excluded from automatic recovery.
Pre-admission failure is persisted and requires explicit retry, not a startup loop.
Late admission errors cannot overwrite dispatched intent. Status API and Journey card
expose persisted delivery failure separately from the saved correction. No new executor
or independent budget. A recovery result without worker admission is settled for
attention to prevent unchanged-intent loops.

Verification: temporary SQLite answer transaction test verifies bad-link rollback,
one feedback revision, reopen/idempotence, eligibility only after local acknowledgment,
and durable failure without automatic requeue. Extended journey test verifies worker
claim consumes delivery and restart/late errors cannot make it pending again.
11 Storage human-request tests + 10 journey tests pass (63109/16800), 48 server tests
and 6 Application human-request tests pass (2080), server/storage clippy passes.
Typecheck + 228 Web unit tests pass (49904). Existing 4 browser joint-repair cases pass
(96998, 25.4s, isolated `/tmp/annotagent-guided-e2e-23276`). No real data/Provider or remote
changes, no push. Browser execution preceded the final delivery-error display addition.

Still not proven: an actual server process killed precisely between answer commit and
worker admission, then restarted through `serve` and observed completing the same job.
Database reopen tests are not a substitute for that test. Also inspect explicit local
Draft-preparation retry: an Answered request whose first local resume failed has saved
intent, but its manual preparation endpoint still needs to dispatch that intent after
successful preparation. These remain required follow-up work, not completion claims.

### 2026-09-09 — Explicit preparation retry uses the original delivery link

The human-request resume endpoint now reads its owned persisted answer-delivery consent,
finishes local revision preparation and invokes the same saved-answer delivery helper
as initial submission. It never selects a latest grant or creates one. No linked intent
means local-only recovery as before. Answers remain saved when continuation admission
fails. Request UI explains that retry can also continue already-authorized inference.
The resume route uses expensive-action admission, not the control lane; the new guard
assertion failed before the fix (76614). Storage tests reject foreign-owner link reads.

Normal 4-case browser regression passed (9896). Added an admission-failure variant:
disable the fixture Model Profile after consent, save the real canvas correction, check
persisted failure/no worker/no added calls and reload the visible error. The first test
incorrectly expected re-enabling the profile to restore old permission; 32155 rejected
it because enabling changes profile revision and leaves it unverified. That rejection
is correct: current availability cannot override a frozen binding. The final assertion
requires the same consent's explicit retry to preserve the correction and remain blocked
without calls, not silently expand permission. Unchanged completed-task retries reuse
the original operation without more calls. This is not a forced local-copy failure test.

Final browser **80423: 5/5 (13.8s)** in `/tmp/annotagent-guided-e2e-24278`; 48 server tests
and server clippy pass (90487), answer delivery storage test passes (5010), typecheck
and 228 Web unit tests pass (17077). Existing production chunk warning only. No real
Provider/data/remote changes and no push. Actual process-kill/restart delivery testing
and a forced local preparation-failure integration case remain to be demonstrated;
the full acceptance matrix and combined regression sweep remain open.

### 2026-09-09 — Real SIGKILL checkpoint and production startup recovery verified

Added a dedicated browser process test with its own temporary workspace and ephemeral
port, not the shared E2E server or the user's 8787 service. Setup uses real HTTP and
the existing TEST transport; canvas submission supplies a real structured correction.
The browser request is intentionally intercepted before persistence. A subprocess-only
ignored Rust test invokes the actual Application answer and local repair services,
verifies pending delivery/no dispatch, flushes a checkpoint marker, then is SIGKILLed
by its owning test. This fixture is compiled only into the test executable; no production
fault injection or pause hook was added. The normal production `annotagent serve`
binary then reopens that same workspace and admits the pending continuation at startup.

The recovered sample succeeds under the original sample operation ID and exact repair
checkpoint Draft. Exactly one Sandbox feedback revision exists. A second normal server
restart preserves the completed result and leaves both call receipts and the complete
task/Project budget unchanged. This demonstrates a real process death at the Application
commit/pre-dispatch boundary, not a production HTTP handler killed at an arbitrary line.

12103 passed the initial process test. 92168 exercised all recovery assertions but failed
only when writing its evidence JSON because its new output directory was absent; fixed
the test artifact directory creation. Final **7009 passes 6/6 (15.2s)**: restart plus the
five normal/failure joint-repair cases. Server tests: 48 passed, one subprocess helper
intentionally ignored in the ordinary suite and explicitly exercised by the browser;
server clippy/fmt pass (75992). No real credentials, datasets or remotes touched; no push.

Inspected evidence:
`/tmp/annotagent-answer-process-final/conversational-workspace/answer-restart-evidence.json`.
Workspace `TEST-answer-restart-n7rAvm`, task `2e8c27cb-33f3-4f4c-9e1b-834662134451`, consent
`9b798e57-cd1a-46f8-b8ae-7078eedb46d7`, recovered sample
`74ce3cdb-973d-4806-8e13-8c2c7757f682`. Reserved calls were 7 before the authorized repair,
13 after it, and remained 13 after completed-work restart. This is fixture protocol and
recovery evidence, not Live inference or annotation quality evidence. Next: combined
full regression and current requirement-by-requirement acceptance audit; no completion
claim is made from these targeted tests alone.

### 2026-09-09 — Combined regression in progress (after 027b8ea)

Started full workspace fmt/clippy/tests/build in process **77115** and Web
typecheck/unit/build/full Playwright in **34639**. Both were confirmed live at the last
poll. Web typecheck, 228 unit tests and production build have passed; Playwright startup
was waiting on the shared Cargo build lock. Rust workspace fmt/clippy passed and the
full test build was still running. Evidence destination is
`/tmp/annotagent-conversation-full-final`. Do not rebuild served Web assets or restart
either test process based only on an observation timeout. Final results are not known yet.

### 2026-09-09 — Current regression recovery and evidence audit

The former 77115 handle could no longer be read; process disappearance was not treated as
success. Re-ran the exact fmt/clippy/test/build chain with all workspace features, logging
to `/tmp/annotagent-rust-final-20260909.log`. **53088 exited 0**, including the final build.
Ignored real-weight/inference fixtures remain conditional; the ignored answer checkpoint
helper is exercised explicitly in its isolated browser subprocess, not by ordinary Cargo.
Web 34639 remains alive: its typecheck, 228 unit tests and build passed earlier; browser
cases 1–44 have now been observed passing. No final 178-test result is claimed. No production
source/build assets were changed while this browser run was live.

Current requirement-to-evidence index (test sources were re-read; the full sweep is pending):

| Requirement group | Concrete test or service evidence | Remaining qualification |
| --- | --- | --- |
| Project entry, persistent split canvas, frozen image selection | `conversation-entry`, `conversation-workspace`, `conversation-goals` browser specs | Narrow-screen/reduced-motion automation is not a novice study. |
| Goal to Schema and sample, bbox and classification | `conversation-initial-journey`, `conversation-human-schema`, `conversation-clarification` | Explicit TEST transport, not Live semantic accuracy. |
| Missing model and return from setup | `conversation-goals` creates a TEST Provider/credential/model, explicitly probes it and returns to the exact saved task URL; also checks cancelling setup through settings sections | This test has now passed as case 50 in the current sweep. It does not exercise every possible image/request deep link. |
| Frozen candidate, ambiguous scope, future rules | `conversation-feedback`, `conversation-image-class`, `conversation-future-schema(-proposal)` | One local repair does not establish dataset-wide calibration. |
| Canvas answer and same-grant continuation | `conversation-joint-repair` five targeted cases passed; `conversation-answer-restart` passed | Actual process death is at the Application commit/pre-dispatch boundary. |
| Duplicate answer, failed admission, restart budgets | Same targeted specs and storage receipt/outbox tests | A forced local Draft-preparation failure is still a distinct integration gap. |
| Formal confirmation, publication, actual review/export | `conversation-samples` checks exact revisions, receipts, downloaded archives, failed-save preservation | Some control display tests use explicit browser fixtures; actual cancellation is separately tested. |
| Real executor cancellation | `conversation-stop-executors` checks Builder, standalone sample and dataset Batch | No provider physical exactly-once claim. |
| Pagination and faithful preview pixels | `conversation-history`, `conversation-previews`, large-index workspace case | Default goal discovery can still scan reference-only history; full dataset metadata is still loaded. |
| Keyboard and layout | Workspace separator keyboard tests, 390/1024/1280/1440 reflow, synthetic composition in `conversation-stop` | Native OS IME, assistive technology and actual 200% browser zoom not verified. |

The optional suggested point-only / generic CompareCandidates Human Request protocols are
not established by the implemented reference-box/category and correction/comparison UI.
They must not be reported as implemented generic request kinds. The priority acceptance is
generic bbox and whole-image classification. No Live Provider test or real-human novice
usability test was executed in this round. Historical screenshot changes and `test-results/`
remain untouched and unstaged. Branch remains main; no push or remote modification.

### 2026-09-09 — Bounded default-goal discovery and verified setup return

Read the complete `conversation-goals` test, not only its title: it already exercises
real TEST Provider/credential/model creation, explicit active-probe consent, model default
binding, settings refresh and return to the exact original conversation/task URL without
reserving inference calls. Case 50 of 34639 has now passed. Inspected its actual 390px
screenshot in `/tmp/annotagent-conversation-full-final/conversational-workspace/task-model-setup-390.png`;
the return control and setup scope are visible and the layout reflows. The earlier audit's
claim of only legacy setup coverage was incomplete and the matrix above is corrected.

Fixed the confirmed reference-only-history transfer gap: default goal discovery now uses
one owned read on the existing messages endpoint (`first_goal=true`), returning zero or
one oldest unscoped message. SQLite filters the existing journal; it does not select the
newest visible note, create a task, change saved task selection or authorize execution.
The ordinary 100-message tail and explicit older-page navigation remain unchanged.
Exact task sources still bypass default-goal discovery. This bounds network transfer and
frontend parsing, not the database's worst-case scan; no false constant-time claim is made.

Added Storage reference-only/empty/oldest/foreign-owner/reopen coverage and HTTP owner/query
conflict coverage. Both targeted tests pass in 72162. Typecheck and seven history unit
tests pass, including late-result cancellation. Server all-target/all-feature clippy also
passed (72162 exited 0). The browser history fixture's expected query was updated only after its
baseline case 51 finished. **The new frontend has not been built or served yet**: the live
178-case run must finish against its original assets. Its last observed passing case is
74. A fresh targeted browser history/setup test and build are required after that run;
the ongoing baseline sweep cannot prove this new read path. No real data or remote changes.

### 2026-09-09 — Full-suite registry-size failures diagnosed, exact test selection fixed

34639 remains live, through case 86. Cases 80, 81 and 83 failed; the full sweep is not
green. Inspected all three retained error contexts: the saved repair card had 68 available
Registry models, correctly selected none rather than exceeding the 32-model permission
ceiling, and asked for an explicit choice. The tests expected the authorization panel
without choosing a model. This is suite-size dependence in the test, not evidence that
saved corrections or execution recovery failed. Other tests in this file passed.

Updated the three UI scenarios to open the existing model choices, uncheck defaults and
explicitly select their own TEST model before reviewing consent. No production ceiling,
automatic selection or scope validation was relaxed. The helper works with both small
and large registries and retains the real UI authorization path, without interception.
Typecheck and diff checks pass. A fresh browser rerun is still required after the current
sweep terminates; no passing rerun is claimed yet. Failure screenshots, contexts and traces
were preserved under `/tmp/annotagent-full-run-failures-20260909` before any future runner
cleanup. New-query frontend integration remains pending as recorded above. The complete
230-unit Web suite passed in 44477 after 6eee1e0's changes.

### 2026-09-09 — Actual export inspected; missing review task binding reproduced and fixed

The running sweep passed bbox case 92 and classification/review cases 91/93/94/95.
Inspected `formal-export-bbox.png`, then read only the isolated workspace's export records
and files. Project `conversation-samples-1788914701229`, task
`eb94fb16-b54c-48d3-b98d-3f680e8bd3db` has applied correction request
`573c6655-c204-4fc2-9246-7348c620dbe6`, feedback `c8722cd4-57be-4646-b2fc-7aee600de3cf`,
repair Draft `3df764b4-28db-4906-acc7-a1d6e45fc98f`, retest
`3a1ca8e7-0cea-4852-8ad5-84deefc5e704`, workflow version 1 and processing/batch
`1bbde069-cd50-4c42-90bc-73c541ad6543`. Export `c9a5a23b-ecbd-4dde-b0da-d9c20b842724`
contains one human-accepted annotation and a report. ZIP size 1608 and SHA-256
`73a7082578c44b1bcab8df8c994ff4527285d2ee8db0b33577b91259b6f6ce33` match the delivery receipt.
This is actual Fixture service/file evidence, not Live image quality.

Inspection found `task_id: unbound` in the exported detection. The published Commit
contains the correct task, but its generated geometry-review node had only a reason.
`pipeline_annotations` obtains the task from the terminal review node, so needs-review
detections lost that binding before export. Added a composition regression: **19728 failed**
with missing task `objects`. The generated conversation detection review step now carries
its explicit target task and label, matching its Commit. **70110 passed** the regression
and Application all-target/all-feature clippy. Added an export assertion rejecting unbound
tasks; it has not been browser-run yet. No historical annotation or Published Version was
rewritten. Other generic review-insertion paths and old snapshots still need an explicit
audit; this targeted fix is not a claim that all legacy provenance is repaired.

Full browser 34639 remains live, last observed through case 135, with the three known
joint-repair test failures retained. The new build, targeted reruns and final complete
regression remain pending. No user data, credentials or remotes were touched.

### 2026-09-09 — Full sweep terminated; canonical Run owner pagination fixed

**34639 exited 1: 150 passed, 4 failed, 24 did not run (23.5m).** Three failures are the
joint-repair exact-model-selection cases already corrected in 14818ab. The fourth is
`guided-workspace` case 136: a real Run opened at `/runs/:id` but did not redirect to its
Project. Read the failure snapshot and current routing: both global and locally fetched
Run details depended on finding their Project in the bounded dashboard list. The service
already supplies a resolved canonical Project route ID in `HistoryRun.project_id`.

Routing now uses that server-owned ID when `ownership_status` is `resolved`, independently
of the dashboard page. Orphan records do not acquire an owner; the existing exact Project
summary loader handles the canonical destination. Image, annotation, view, node and
artifact query context is retained. The browser regression now explicitly excludes the
real owning Project from the inventory response, so a small fresh workspace also tests
this failure condition. No owner response is fabricated. The original failure was saved
alongside the three joint-repair traces in `/tmp/annotagent-full-run-failures-20260909`.

Typecheck, all **231 Web unit tests**, production Web build and all-feature annotagent
binary build pass (19211). Existing large JS chunk warning remains. Because the previous
browser process is terminal, fresh targeted browser **14035** now checks history, Project
entry, joint repair and the complete serial Guided suite using new assets and an isolated
workspace. Results are pending; these include the new goal query and the previously
unrun management/review cases. New bbox export/task-binding verification and final Rust
full regression still need to run. Generic geometry-review insertion audit remains open;
old Published Versions and exports are unchanged. No push or remote changes.

14035 has passed entry/history/new goal-query cases, but joint UI cases 5/6 exposed a
locator error in the new test helper: its nested `has` selector repeated the outer card
scope when evaluated relative to each details element. The actual model options were
present. Corrected the helper to filter its details by the visible unique model-group
text. The targeted sweep remains live and this selector correction still needs rerun;
neither the initial test-helper patch nor this correction is reported as browser-green.

### 2026-09-09 — Guided regression green and inserted review identity preserved

14035 terminated with **41 passed, 3 failed**. All 36 Guided cases passed, including the
explicit paginated-owner Run test, Review/source return, management deletion/recovery,
export, reflow and keyboard checks. Its three failures used the pre-correction relative
selector. Fresh 43586 now runs 14 joint-repair/sample cases with the corrected selector;
all five joint-repair cases have passed, including both real pending-answer UI variants.
Last observed case 10 passed; full result and the new export task assertion remain pending.

Audited `add_mandatory_geometry_review_boundaries`, the separate safe-Draft insertion path.
An initial assertion on an existing legacy fixture passed but did not prove propagation
of a nonempty task binding. Replaced that weak assertion with a three-label composition
fixture requiring explicit task/label metadata and at least one inserted boundary per
Commit. **24723 failed**: inserted review had no `task_id` while its Commit had `objects`.
The insertion now copies only the Commit's explicit `task_id` and `target_label` when
present, in addition to its mandatory geometry-review reason. It does not infer ownership
from names or copy unrelated execution parameters. Existing absent metadata is not invented.

**93743 exited 0**: exact boundary regression, legacy immutable/blocked/clonable workflow
regression, Application all-target/all-feature clippy. This modifies new Draft construction,
not stored Published Versions or historical annotations. Current browser run uses the
previous binary, so it does not validate this latest insertion change. Final full Rust
and browser regression still remain, along with accurate limitations on legacy output.

### 2026-09-09 — Classification review export binding gap caught by the new assertion

43586 terminated **13 passed, 1 failed (3.7m)**. All five corrected joint-repair cases
passed, and bbox export passed the new non-unbound task assertion. Classification review
failed that same assertion: its separately constructed confidence-review step also carried
only a reason. The export/Review services otherwise completed; removing the assertion
would have hidden a real annotation-scope defect.

The classification confidence-review step now persists its explicit target task and label,
as does the detection branch. Added composition checks for the classification route as
well. **31038 exited 0** for the targeted composition regression and all-target/all-feature
Application clippy. Historical results and immutable versions are not rewritten.

Fresh browser **99466** runs the exact classification-review export case against the new
binary; its result is pending. The attempted failure-trace backup used a root-relative path
from the Web directory and failed before the new runner cleaned its output; that particular
trace was not retained. The observed failure and exact assertion are recorded here rather
than claiming a preserved archive. Rust full fmt/clippy/test/build **6393** is also running,
with durable output at `/tmp/annotagent-rust-post-binding-20260909.log`. No full-green claim,
no real Provider/data mutation, and no push or remote changes.

99466 subsequently **passed 1/1 (31.0s)**, including actual classification-review export
with a non-unbound task. Started a new full Web typecheck/unit/build/178-case browser sweep
against 2502ab3; durable log `/tmp/annotagent-web-combined-20260909.log`, evidence destination
`/tmp/annotagent-conversation-combined-verified`. This sweep and Rust 6393 remain pending.

### 2026-09-09 — Post-fix full Rust pass and classification delivery evidence

**6393 exited 0** for workspace fmt check, all-target/all-feature clippy, all-feature tests
and build. The durable log totals **713 passed, 6 explicitly ignored**. Conditional live
and real-weight tests remain excluded; this is not a claim of real inference coverage.
Full Web process **37872** is confirmed live, through case 12 without observed failure.
Its isolated workspace is `/tmp/annotagent-guided-e2e-30211`; final results remain pending.

Inspected the fresh classification export JSON and actual export screenshot from 99466.
Conversation `f379c955-bf4b-46c6-9e19-a034f1ff9001`, task
`5a91d8b7-5a96-4583-995a-5a99ed6283c4`, Draft/workflow
`544686d9-cc74-4172-a59c-c36db472e3f0` version 1, sample
`abeeea6c-00cd-4c87-bcb0-ac460f29aba4`, processing/batch
`1e9c1b39-0861-422b-b962-12708c7c35b7`, delivery
`7f314511-879c-41b3-977a-c03ad8d44fb0`. The one human-accepted classification now records
`task_id: annotation_5a91d8b75a964583995a5a99ed6283c4`, not `unbound`. ZIP SHA-256
`5b20f2fa7619a2655ad48497d1469fba8c536489f2e1563b9ae86375db831261` matches its delivery.
Screenshot `/tmp/annotagent-classification-binding-final/formal-export-classification.png`
shows the real download/report and correctly distinguishes a server folder from a local
desktop folder. Model provenance still uses a binding alias in this exporter; this evidence
does not claim the archive contains a complete standalone model registry or all Artifacts.
No Live quality or real-human novice test was performed. No push or remote change.

### 2026-09-09 — Actual browser zoom attempt remains unverified

Opened an agent-owned Chrome tab only on the isolated 8791 Project inventory. The supported
page key API sent Meta-plus, but read-only viewport measurements stayed `innerWidth=1470`,
`devicePixelRatio=2` before and after. This did not establish actual browser zoom, and no
200% success is claimed. Closed the temporary QA tab without changing Project data, starting
inference or modifying user tabs. Existing reflow/keyboard automation remains distinct from
native browser zoom, native IME and assistive-technology validation. Full browser 37872
continues against unchanged assets; its final result is still pending.
