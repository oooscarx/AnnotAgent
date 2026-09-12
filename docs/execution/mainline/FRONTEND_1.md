# Mainline Frontend 1

## Baseline

- Common baseline: `d3220cb54bd50589efed86b6c0753bf4ea8db0db`.
- Integration worktree: `/Users/oscar/Documents/my_workspace/AnnotAgent-mainline-integration`.
- Branch: `codex/mainline-integration`.
- The source worktree contained only user-owned untracked design/product directories; none were copied, removed or committed here.

## F1-0 complete

Frontend 1 owns App/http/adapter/public route and type composition. Frontend 2 owns review/image/package modules. Frontend 3 owns model/plugin/settings modules. Backend owns Rust, migrations and `docs/contracts/mainline-v1`.

The first seam defines a server-backed `MainlineTaskView`, typed real message projections, complete frozen `VisualSelection`, capability setup return object and domain composition slots. It introduces no guessed HTTP route and no browser state machine. A task is complete only when the server-backed training package is Ready and downloadable.

Communication:

- `ML-001` queued to Frontend 2 UUID `01a094ad-5715-75d0-88cf-33b5eb7fa3e8`.
- `ML-002` queued to Backend UUID `01a0855e-9c39-7c33-9f18-93e084d14816`.
- `ML-003` resolved exact Frontend 3 UUID `01a094ad-337c-7bc0-99ea-b3d18acb7d2f` and was queued successfully.

No Provider call, true workspace write, push or main merge.

### F1-0 delivery

- `7fd1439`: stable composition seam and ownership/status records; 336 unit tests passed.
- Backend B0 contract `99d16f9` integrated as `5a732c5`; all planned routes remain disabled/unreferenced.

### F1-1 current slice

The existing server-backed delivery-intent editor now names the missing slots and initially renders only those fields. Full editing stays explicitly available. Saved dataset scope, label rules and YOLO training target remain one Task-owned revision; no model call occurs when saving them.

Persisted structured model decisions are projected as Agent replies or clarification messages with the actual call identity/status. Failed, empty or invalid receipts remain operation records and are not converted into assistant prose. The generic completed phase now says “current operation completed”; overall completion remains reserved for Package Ready.

Backend reports the exact processing subset, automatic package consent HTTP and the unified task read model as planned, not implemented at B0. Frontend does not call those routes yet.

The HTTP send boundary now accepts only a frozen `VisualSelection`, converts it to the existing `sample_candidate` message reference, verifies the Project Schema is still current before POST, and keeps the same command for uncertain retries. Preview-only/bare candidate identities are rejected before any request. Frontend 2 still needs to supply this selection from the real canvas before the Composer can expose the path.

### Isolated HTTP evidence

- Service: `http://127.0.0.1:8841`, fixture header `external-model-only`, generated TEST workspace only.
- Source build: `2c4b032bf614a63e26fc56904368d3b050ee315f` before the candidate-send boundary commit.
- The real Project route restored saved user messages, structured model decision receipts, execution history, a pending HumanRequest, completed Batch metadata and a real download link.
- Expanding delivery intake displayed only the three server-reported missing items. No Provider request or real workspace write was made.
- Candidate-reference unit coverage: exact image hash, Schema revision, Draft revision, Sample Test, candidate and source Artifact; stale Schema and Preview identities make zero POSTs.
- Browser regression `persisted Agent replies stay distinct and intake asks only server-reported missing items` passed against that isolated HTTP service and observed zero non-GET requests across open and refresh.

### Backend B1 integration

- Backend source `22ddee9` was inspected and integrated as `0f62007`.
- `workspace.mainline` is now retained on the owned Task and validated against Project owner, Conversation and Task IDs.
- The old client heuristic that exposed Plan/Sample/Process/Export together is suppressed once the Mainline projection exists. The visible sample action is derived only from the server's `build_and_test_pipeline · requires_confirmation` action; package/review actions remain in their domain components.
- Deterministic delivery-Schema preparation now posts `advance` only when the exact read model exposes `prepare_delivery_schema · authorized`. The command and read-model revision are persisted for lost-response replay; other actions continue through their explicit approval flows.
- Rust B1 targeted test passed; Web typecheck and 25 focused unit tests passed.
- A freshly seeded TEST service on `http://127.0.0.1:8843` proved the visible journey: create a real Task, save the three delivery slots, execute exactly one server-authorized local `advance`, restore the matching Schema and expose the separately approved Sample action. Refresh emitted no second advance.
- A reload/lost-response unit regression creates a second Adapter and proves it resends the original persisted command/read-model tuple rather than a replacement command.

### Domain deliveries received

- Frontend 2 commits through `238a838` are integrated: task-bound Sample/Formal review surfaces, strict formal Run lineage, review work-item seam and package readiness/consent UI. The new optional formal/package services remain absent in `HttpAdapter` until Backend B2/B3 exists; this leaves those actions visibly unavailable instead of falling back to the legacy latest-Run/package path.
- Frontend 3 patch `2c52266` (source `943a6ab`, patch-id verified by its owner) is integrated: task-scoped model preparation, exact return guard and Settings return banner. Task-side mounting remains blocked on Backend capability readiness; returning from settings never auto-resumes or reuses an old authorization.
- Frontend 2's initial Sample selection had two lineage defects caught during rolling integration: feedback revision was task-wide (fixed in `309b7ed`), and source Artifact is still image-wide even though candidates can have different Artifacts (`ML-009`, blocking Composer wiring).

## F1-2 complete on the integration branch

- Backend B2 canonical Sample selection (`0e776cd`) and Frontend 2's aligned selection projection (`3279929`) are integrated. Each selected Sample candidate now carries its own Project/Conversation/Task, Schema, Draft, Sample Test, image hash, candidate, source Artifact and result revision into the main Composer.
- Backend B3a/B3b (`c5d741d`, `dcf4abc`) and Frontend 1 wiring (`2401e27`) are integrated. Formal review selections use the server-issued `formal_annotation` reference; the client does not rebuild lineage or infer geometry from prose.
- The full-dataset action follows the exact server-issued `start_delivery_processing` preview URL. It prepares one confirmation card only; no GET/mount starts processing and approval keeps the existing idempotent mutation boundary.
- Backend B4 (`5a827d5`) and Frontend 3 readiness UI (`76507bc`) are integrated. Commit `361c402` mounts it in the task: only missing Agent/visual capabilities are requested, setup returns to the same Project and Task, and a return performs a passive recheck without resuming or expanding an earlier authorization.
- The old heuristic Sample action is suppressed while required capabilities are missing. Ready selected Agent plus a ready Project-bound visual profile leaves the server Sample action available.

### Verification after F1-2

- Full Web unit suite: 108 files, 363 tests passed.
- Production Web build and TypeScript/tokens checks passed.
- Focused model setup, capability recheck and HTTP adapter suite: 34 tests passed.
- Earlier isolated HTTP browser evidence on the generated TEST workspace verified canonical Sample selection and passive formal/package reads. No paid or real Provider was called.

## F1-3 delivery and current Sample continuation gate

The formal review, package readiness/consent, exact processing action and Package Ready completion semantics are composed in the production UI. A separate G3 TEST-provider run saved all whole-image decisions and downloaded a real YOLO ZIP; its SHA-256 is `a2a0242282c3e2d112ba1b11009dcfe161e0a175631ac858c1606227adc6b89d`, with 11 image/label pairs, 30 valid YOLO rows and 27 verified manifest hashes. This is structural TEST evidence, not Live model accuracy.

Backend ML-020 (`e705e63`, integrated `28e62b2`) now exposes an exact `test_pipeline_samples` action after Builder completion. Frontend reads the original consent, compares ordered image hashes and model binding digests, displays a second Sample-only confirmation and POSTs only the supplied execution URL. Focused unit tests pass and the first Browser approval produces no duplicate Journey/Builder.

Actual HTTP exposed ML-021: the second POST returns `dispatch:running` but never reserves the Sample. The UI remains incomplete and does not retry a chargeable request. The existing Backend task has the exact trace and reproduction. Package Ready remains the only overall Task completion condition.

### Task history and portable context

Fixed frontend commits `d3b35ad` through `b77f8fe` and backend UIAPI-018 `eaeae4c` are integrated through `474cad6`. The project menu now opens one Agent task-history page with persisted multi-turn messages, model-call receipts, Builder/tool steps, Sample/processing/HumanRequest/queue groups and the complete read-only task snapshot. It explicitly describes observable evidence rather than hidden chain-of-thought.

The same page downloads a versioned JSON context archive and supports previewed, explicit `archive_only` import. Imported state is untrusted, inert history: it does not create a live Task, restore grants or dispatch inference. Targeted Rust tests pass (7 storage + 2 server); focused Web tests pass (28). Real HTTP task-history refresh passed with zero writes, and invalid/oversized files were rejected with zero writes. Save → import is currently blocked by ML-022's cross-runtime numeric hash defect; the UI shows the real 409 and does not weaken integrity validation.

No push, main merge, real workspace cleanup or unapproved external model call has occurred.
