# Conversational Annotation Workspace — execution record

## Current checkpoint (after `c13f92d`; not a completion declaration)

The default Project entry now uses the persisted conversation/image workspace. Explicit goal
selection survives re-entry, sample candidate references are frozen, and clarification/correction
requests can be saved, cancelled or (for corrections) deferred/reopened without resetting budgets.
The isolated bbox/classification tests exercise real Rust services through TEST HTTP model
transports, including correction, revision, formal confirmation, review and export.

Still incomplete: the bounded coordinator that advances through Schema/Builder/sample stages
under one matching authorization; automatic interpretation of scoped conversational feedback;
the other structured visual/setup request kinds; complete scope-change/Schema patch and stop-text
semantics; large-history performance and the remaining accessibility/context restoration audit.
Current phase cards still require explicit intermediate actions. Live model quality, native
200-percent browser zoom and real-human novice usability are not proven. Full regression results
and their failures/fixes are recorded chronologically at the end of this file.

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
