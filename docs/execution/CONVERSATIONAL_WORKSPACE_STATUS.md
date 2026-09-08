# Conversational Annotation Workspace — execution record

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
