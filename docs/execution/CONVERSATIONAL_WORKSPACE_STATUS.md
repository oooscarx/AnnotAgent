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
