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

M4: exact publication/Batch cards, Review/export and management returns, reconnect,
performance, split-pane keyboard/narrow layout, full regressions and two golden paths.

No live Provider validation, new UI usability claim, accuracy claim or human-user
testing has been performed. Native zoom and OS assistive-technology checks remain
unexecuted. No new model, Python Worker, autonomous install or permissive tool API.
