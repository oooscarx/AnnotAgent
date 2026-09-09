# AnnotAgent Agent-first UI — execution record

## Current checkpoint — M0 in progress (2026-09-09)

Goal: implement the complete Paper & Graphite Agent-first contract, M0–M5.
This is not a completion declaration. The preceding Conversational Workspace delivery
is the baseline, not evidence that Plan, queue, model selection or continuation meet
this new contract. Previous goal turn produced evidence; this first turn changes the
authoritative acceptance tests and records the new baseline.

### Inputs and protection

- HEAD `b694f06`, branch `main`, initially synchronized with `origin/main`.
- Read the kit README, DESIGN_SPEC, CODEX_PROMPT, tokens JSON/CSS and legacy token map.
  Viewed all six desktop and both mobile PNGs. Opened the self-contained OPEN_ME.html
  through a loopback-only, read-only server on 8798 and exercised Plan → approval
  dialog → execution → interrupted. The prototype explicitly says it jumps straight
  to the stopped state; production must wait for server acknowledgement.
- No applicable AGENTS.md found in this repo or checked ancestors. Unrelated repos'
  instruction files were not applied.
- The design kit is user-provided and currently untracked. Preserve it and the 110
  existing screenshot/output changes. Stage only files authored for this goal.
- User workspace on 8787 is not changed. Tests use generated TEST workspace on 8791
  and the existing fixture transport on 8796. No paid calls, keys, remote changes,
  push, reset/rebase/amend, historical version changes or cleanup.

## Baseline and reusable contracts

| Area | Current evidence | Required change / retained boundary |
| --- | --- | --- |
| Entry/layout | ConversationWorkspace defaults to width 32, separator range 25–50; empty image surface always visible | Conversation navigation + centered thread; on-demand pane defaults 54/46 and can be enlarged. |
| Composer | Two buttons: Save goal and prepare labels / Save message; JSX decides prepare-goal based on current local selection | Unified send intent resolved by server; attachment/reference/mode/model and separate active control. |
| Messages | Ordinary journal items share card styling; specialized Journey forms are inline | Plain thread typography plus compact actual object blocks; reuse refs and persistence. |
| Stop | Server/Application/Storage conversation_stop persists exact targets, signals cancellation, exposes observations | Reuse this machinery; audit safe-boundary acknowledgement and continuation separately, never infer stopped from fetch abort. |
| Planning / consent | conversation_journey and builder services freeze Schema/model/scope/budgets; explicit execute POST dispatches | Add persisted per-turn Plan execution policy, deny vision/formal operations on direct API paths, bind exact approval. Existing authorization alone is not proof of Plan mode. |
| Model | Registry profiles and planner_model_id already exist in scope construction | Persist next-request Agent selection by conversation revision; no rebind of existing visual workflows. |
| Queue | Current message journal persists refs, but not yet proven to represent actionable queued turns | Add minimum queued command semantics with frozen selection and idempotency, no parallel engine. |
| Human answer | Existing Sandbox Revision/outbox/answer delivery and startup recovery | Preserve exact save-before-resume, scope and budget constraints. |
| Tokens | Runtime styles.css imports web/src/annotagent-tokens.css; old visual-system JSON documents itself canonical, CSS is currently duplicated | Migrate existing source and consumption/generation relation rather than append another theme; retain annotation slots independently. |

### Design observations

The eight images put navigation on the left and the thread in the main area; only
human/results states use a split surface. Ordinary assistant text has no enclosing
card. Plan/authorization/human blocks have light hierarchy. The mobile design switches
between conversation and result, not a compressed three-column desktop. Paper/Graphite
is monochrome chrome with quiet blue-gray focus; original pixels remain untouched.
All pictured objects, budgets and model names in the kit are illustrative, not production
bindings or actual model performance. They will not be imported into the Registry.

## Tests and screenshots

- `npm run typecheck && npm test` (web), process 53233 exited 0: 47 files, **232 tests**.
- `ANNOTAGENT_E2E_EVIDENCE_DIR=/tmp/annotagent-agent-first-m0 npm run test:e2e -- conversation-entry.spec.ts agent-first-workspace.spec.ts`,
  process 91330 exited 1: **2 existing entry cases passed, 1 new acceptance case failed**.
  Four expected baseline mismatches: missing Conversations navigation, missing Send,
  two old Save controls, visible empty image panel. This is a real isolated Project
  created through current APIs, not a mocked page or prototype render.
- [Actual empty-project baseline](agent-first-ui/m0-empty-baseline.png), inspected at
  1280×800. It visibly confirms the fixed empty pane and form-like sidebar.
- Failure trace: `web/test-results/agent-first-workspace-Agen-364f1-nd-exposes-one-send-control-chromium/trace.zip`.
  This path is ephemeral; screenshot above is retained as baseline evidence.
- Full Rust and full E2E for this new goal have not yet run. Earlier 714/178 green runs
  belong to the previous goal, not this goal's new requirements.

## Milestones and next work

1. M0: finish direct-API Plan/control failure contracts and current control-state audit.
2. M1: migrate token source, extract thread/composer/navigation/pane using actual data;
   make the new baseline acceptance test pass without weakening its intent.
3. M2: versioned Plan policy and exact approval, including direct API denial tests.
4. M3: stopped-at-safe-boundary receipts, resumable vs terminal controls, queue and
   persisted next-request Registry model selection.
5. M4: preserve human correction/outbox, URLs, reconnect, keyboard and mobile flows.
6. M5: six actual service screenshots, full regression, theme/viewport/200% evidence
   and honest fixture/live/native limitations; update README only with actual app shots.

No milestone is marked complete merely by this baseline or design inspection.

## M0 control-contract audit — unknown completion race

The previous turn was progress (new failing entry acceptance test and baseline commit
7a5ce44). This turn found and fixed an actual Application-layer stop observation bug.

When the stop menu offered multiple active requests, a chosen Provider could become
`in_doubt` before selection. Storage correctly saved `Finished` (nothing left to cancel),
but Application returned `finished` before inspecting the actual remote outcome. This
could hide remote completion/cost uncertainty in the new stopped-state UI.

Added `stop_selection_after_unknown_completion_does_not_hide_remote_uncertainty` with
two real reserved calls in an isolated temporary database. It failed before the fix:
expected `unknown`, got `finished` (process 58348, exit 101). The fix keeps the receipt's
historical selection status but always inspects the selected operation: pending and
unknown observations take priority. It does not cancel the second request, reset its
budget, retry a Provider, or invent a resumable checkpoint.

Verification:

- `cargo fmt --all`: only the two intended Application files changed (excluding existing PNGs).
- `cargo test -p annotagent-application conversation_stop::tests -- --nocapture`:
  3 passed, process 58925 exit 0; includes pending → unknown and exact-owner tests.
- `cargo test -p annotagent-storage conversation_stop -- --nocapture`:
  20 passed, process 17448 exit 0; includes transactional selection, shared siblings,
  cumulative grants, publication fences and late admission.
- `cargo clippy -p annotagent-application --all-targets --all-features -- -D warnings`:
  process 31615 exited 0.

Still open: Plan does not yet have the per-turn policy required here; do not present
existing grant checks as Plan-mode verification. Continuation and queued input remain
work for M2/M3. The M0 entry test remains intentionally red until the actual new layout
and unified server-resolved send contract are connected. No real workspace restarted.

## M1 first layout slice — on-demand artifact surface

Previous turn was progress: c3ae1fd fixed the stop-observation race with failing-then-passing
tests. This turn begins the actual layout migration; it does not complete M1 or Plan.

- The existing ConversationWorkspace now centers the thread when no images/results are
  present. A visible Open data and results control opens the existing importer/canvas.
- Expanded default is 54% conversation instead of 32%; separator range is 25–75, so an
  explicit user adjustment can prioritize either side. This is not the final navigation
  shell or token migration, and resized-width preference persistence is still outstanding.
- Added typed `pane=thread|artifacts` to canonical route parsing/building, rejecting invalid
  or duplicate values. It retains Project/task/image IDs and unchanged route focus key.
  Setup-return context carries this pane value. Explicit new object links may open results.
- The canvas stays mounted while hidden, preserving editors. Only the exact pane-toggle
  navigation bypasses discard confirmation; other navigation keeps its original guard.
- The first browser attempt correctly failed because the old dirty guard intercepted a
  pane toggle with unsent text (27598 exit 1). Fixed the precise navigation boundary, not
  the general guard. Pending sends, feedback or formal results are not re-executed.

Tests: new route test failed before implementation (missing pane); Web typecheck and
233 unit tests passed after implementation. Actual isolated browser test 71151 passed
1/1 (6.7s), including same-object toggles retaining unsent text, default 54% separator,
and open/closed URL restoration on reload. Production build ran in that test startup.
The browser test intentionally clears unsent text before reload: cross-refresh unsent
composer persistence is still required in M3 and is NOT claimed by this test.

Outstanding M1: session navigation, extracted thread/composer, unified send, theme source
migration, mobile/resize preferences, and dedicated object message presentation. The first
M0 whole-entry acceptance case remains red for those unimplemented requirements; it was
not weakened or marked as passed. Full Rust/browser regression remains for later stages.

## M1 second layout slice — real task navigation

The preceding turn inspected requested Live/usability preflight but did not advance this
UI goal. Resumed from the actual uncommitted navigation changes; no paid call or real
workspace mutation was made. Current backend has one primary conversation per Project,
so navigation lists its real independent tasks rather than fabricating a conversation tree.

- Added ConversationNavigation using persisted tasks and source message titles, with search,
  selected-task highlighting, collapse and existing Projects/settings routes. Missing paged
  titles use a task ID fallback, not invented text. Opening a task is a read-only URL change,
  not a new task, model call or overwrite of the persisted preferred-task selection.
- Existing ownership, task deep links, dirty guards, query layer and mounted editors remain
  shared. Empty Projects now show an honest empty state after loading, not an endless task
  loading message. Current default task is highlighted even without an explicit task query.
- Narrow screens start with navigation collapsed; desktop uses a 216px navigation alongside
  the existing thread and optional artifact pane. This is a partial shell, not final Paper &
  Graphite or unified Composer. Collapse/width preference persistence remains outstanding.

Evidence: Web typecheck and 233 unit tests passed (13030). Production build plus isolated
browser pane restoration and independent-task selection/search/reload passed 2/2 (43279).
Additional 390×844 browser test passed 1/1 (13030), checking disclosure, editable composer and
no document horizontal overflow. These are viewport tests, NOT native 200% zoom or human
usability evidence. Existing chunk-size warning remains. The whole-entry Send contract test
is still intentionally red; no claim of M1/M2 completion or full regression. No push and no
remote changes; unrelated screenshot/design-package changes remain unstaged.

## M1 third slice — canonical Paper & Graphite token generation

Previous turn was progress (22dafd9, task navigation plus three focused browser cases).
Migrated the existing production `design/annotagent-visual-system/tokens/tokens.json` to
schema version 2, using the supplied Paper & Graphite values, system sans/mono fonts and
layout dimensions. The reference kit is not imported at runtime. New
`scripts/generate-tokens.mjs` generates runtime CSS and both existing bundled CSS copies;
Web typecheck/build now reject drift. Historical package examples/checksums remain explicitly
documented as original-delivery references, not current verification. TUI migration remains.

Kept all eight established annotation slots and halo independently of UI colors. Added
paired foreground tokens for primary/danger/success/warning surfaces and updated existing
filled controls, including theme-aware checkbox marks. Legacy teal/violet compatibility
aliases preserve older management consumers but no longer supply decorative brand colors.
Removed the blanket rounded background on every conversation message and used the canonical
720px thread maximum/216px sidebar values. Brand asset migration and old Journey/composer
replacement remain outstanding; this is not a claim of finished Agent UI.

Web typecheck, generation check and 236 unit tests passed (7678), including both themes'
text/filled-control contrast >=4.5 and unchanged annotation palette. Production build and
three isolated browser cases passed (70151). Repeated theme/pane browser case 41061 passed
after adding computed button-background assertions and disabling screenshot transitions.
Actual isolated application screenshots are `/tmp/annotagent-agent-first-light-navigation.png`
and `/tmp/annotagent-agent-first-dark-navigation.png`; inspected the dark page. They contain
TEST Project data and real UI, not prototype assets or Live model results, and are partial
M1 evidence, NOT the required final six-state deliverable. Full Rust/management/theme suite,
native 200%, paid model accuracy and real-human usability are not claimed. Build's existing
large-chunk warning remains. No push, remote change, secret use or real workspace cleanup.

## M1 fourth slice — extracted Composer and keyboard submission

Previous turn was progress (0a7f018 canonical theme migration). Inspected actual journal,
task creation and control contracts: ordinary append grants no inference, first-goal
preparation currently spans append/task/selection/authorization calls, and candidate/stop
commands have separate validated scopes. A new server-resolved send command must persist
its selected scope and receipt across retries; renaming both old controls would not implement
that contract. No fake Plan picker or frontend-only execution permission was added.

Extracted AgentComposer from ConversationWorkspace while retaining the existing frozen
command coordinator. References precede input, the visual field label is screen-reader-only,
actions wrap, Enter submits, Shift+Enter inserts a newline, composition/native composing and
keyCode 229 suppress submission. Frozen input locking is separate from form admission, so
the original retry button remains usable after an uncertain acknowledgement. Existing
legacy save controls remain intentionally visible until the server send contract lands;
unified Send, queued editing while a request is active, Plan and model choice are not done.

Web typecheck/generation check, 236 unit tests (71457), and 12 isolated browser cases (34382)
passed, with fresh production build. Cases include pane restoration, 390px keyboard submission
with exactly one journal POST, empty stop, actual reserved-call cancellation, multiple-target
selection, frozen-target restoration, lost acknowledgements, Chinese composition simulation,
cross-task rejection and stopping one actual task without touching another. This verifies
the extraction, not final Plan/continue/queue requirements or native IME/200% usability.
No Rust code changed in this slice; no full Rust or entire E2E pass claimed. No push/remote
change and no Live inference. Overall M1 and the full goal remain incomplete.

## Server send foundation — atomic admission, not execution

Previous turn was progress (e1c2b70 Composer extraction with 12 browser regressions).
Added POST `/api/projects/:project/conversations/:conversation/send` with a strict input:
message ID/text/frozen image or candidate reference, explicit task context (nullable for a
new task), and schema revision. The application resolves stable Project ownership, validates
first-admission provenance and the new-task current schema under the existing schema lock.
Storage atomically saves the journal message, new task if needed, and frozen receipt in new
migration 0053. Existing tasks receive an explicit persisted message association, not a
replacement goal. Candidate feedback must agree with its task/revision; stop still requires
the dedicated stop endpoint. Legacy journal IDs cannot silently become new dispatch commands.

Exact retry returns the original receipt and task even after restart; changes to frozen
context conflict. Extracted the original journal validation into a shared transaction helper,
retaining foreign image and candidate schema checks. This is a dispatch admission record,
NOT an execution grant, Plan policy or implemented queue processor. Composer still uses the
old endpoint pending client frozen-command/receipt recovery integration. No auto-LLM call,
Publish, Commit or Batch is attached to send, and no prototype response is returned.

Verification: two new storage tests (restart/idempotency/retarget rejection and a deliberate
receipt-insert failure proving full message/task rollback), five existing journal tests and
one new application stale-schema/owner/stop test passed (77559, 55597). Initial clippy caught
test-module placement; corrected it. Storage/Application/Server all-target/all-feature clippy
passed 9772. Fresh production build and isolated real HTTP test 80866 passed 1/1, proving
stable retry, one task/two linked messages, forbidden extra execute field and zero authorized
or reserved model calls. This is not Plan-permission coverage or a full Rust/browser sweep.
No real workspace restart/migration, paid call, push, remote change or historical rewrite.

## Composer connected to atomic send

Previous turn was progress (cbc75b6 server admission/rollback tests). Connected the default
Composer to `/send`: one Send button replaces Save goal/Save message, with the dedicated Stop
control unchanged. The coordinator freezes message, task/schema and conversation for the
request; retries reuse the entire command. New-task admission comes from the server receipt,
then the existing selection and authorization UI is restored. Existing task messages remain
linked to that task; candidate feedback retains exact scope. Sending does not grant or
automatically perform inference. Missing selected-task context blocks send rather than
silently creating a new task. Ordinary task-message interpretation/queue execution is still
outstanding; this endpoint does not pretend to have generated an Agent answer.

Web typecheck/token check and 236 unit tests passed. Five isolated browser cases (20545)
passed, including the original unweakened M0 default-entry contract, actual keyboard Send,
server receipt checks and lost-response retry after opening the artifact pane. Ten existing
stop regressions (44826) passed. Production builds ran in both suites. The client explicitly
retains its frozen conversation ID for retry as well as its task/revision. Broader older E2E
scenarios using Save message/Save goal selectors still need migration and behavior auditing;
these focused passes are not an entire-suite claim.

Important remaining gap: ordinary pending-send input is held in mounted coordinator memory;
cross-refresh recovery of an unacknowledged send is not yet implemented (Stop retains its
separate existing recovery). Successful-message refresh is tested and never re-POSTs, but
it does not prove pending-send refresh. Plan policy, approval binding, queued execution,
model picker, advanced form reduction and final visual deliverables remain incomplete.
No Live call, push, remote modification or user-data cleanup.

## Pending-send refresh recovery

Previous turn was progress (2b6a57c unified Send and frozen in-view retry). Added owned,
read-only GET `/conversations/:conversation/send/:message` returning the saved command and
receipt or null. The client stores a pending command in this tab's sessionStorage before
POST; storage failure stops admission rather than sending an unrecoverable request. Reload
validates the pending envelope and reads that exact server receipt. A matching persisted
receipt restores the message and clears pending input; a missing/failed GET retains the
original command for explicit Retry same send. GET/mount never posts or grants model calls.
The existing AbortController guards prevent an old receipt response from overriding a newer
view. No published plan, grant or model binding is reconstructed from local storage.

Web typecheck/token check and 238 unit tests passed (45247), including malformed pending
envelopes and semantic command comparison. Four real isolated HTTP/browser cases passed
twice (45247, final 88936), covering lost response after server admission, transport failure
before admission, refresh with zero new POSTs, exact manual retry, same-page retry and
foreign-owner/missing receipt reads. Fresh production builds ran. Application/Server
all-target/all-feature clippy passed 40655 after correcting a map/unwrap lint. Initial npm
typecheck was accidentally invoked at repo root and failed; rerun from web passed. No claim
of full Rust/E2E, native zoom, Live quality or human usability validation.

This closes pending-command recovery within the same browser tab; it is not a cross-device
outbox, queued execution or unsent-text persistence. SessionStorage loss cannot restore text
the server never received. Old broad E2E selectors and new-task navigation remain work, along
with the central missing Plan/approval/queue/model contracts and final visual delivery.
No push, remote change, real workspace restart/cleanup or paid invocation.

## Builder Plan execution policy foundation

Previous turn was progress (afda92c pending-send refresh without reposting). Added persisted
`PipelineBuilderConstraints.planning_only`, independent of `maximum_dry_runs`. The existing
conversation Builder always sets it on the server; the client cannot supply replacement
constraints to that conversation operation. Legacy/general bounded Builder defaults remain
unchanged and are not silently relabeled Plan. This is a request/session policy foundation,
not the final selectable per-turn mode or a project-wide prohibition on independent actions.

Plan hides Dry Run from offered tools AND rejects a model-supplied DryRunPipeline at the
actual dispatch boundary before its implementation, with `plan_permission_denied`. Numeric
image budget cannot override it. Fixed a real fallback prompt that previously told a no-tool
response to perform Dry Run even with a zero-image allowance. Static-valid Plan submission
can reach human review without pretending an image test ran. Existing registered-tool
allowlist still excludes publish, full runs, credentials, shell and downloads; passive image
inspection returns dimensions/byte length, not image pixels. No new model/Runtime was added.

New Application test 74783 passed: a scripted planner explicitly calls the forbidden Dry Run
with positive numeric sample budget; the dispatch reports denial, no Dry Run or formal Run
or published version appears, a static proposal reaches human review, and the persisted
session retains planning_only. Core registry-escape and planning-budget tests passed in 40957.
The existing authorized bounded-sample multi-turn Builder regression also passed in 40957;
its scripted provider is not a Live quality test despite the historical test name.
Core/Application all-target/all-feature clippy passed 96955. These use synthetic TEST inputs
and scripted planning responses, not paid models or actual annotation quality evidence.

Remaining M2: selectable mode tied to frozen send/turn configuration, exact approval mode
transition, cross-API permission audit and direct Plan-mode HTTP rejection tests. This
Builder-dispatch test must not be cited as proof of all project API permissions, schema
privacy, model-switch authorization, queue or continuation semantics. M1/other stages also
remain incomplete; no push, remote change, real workspace mutation or old key usage.

## User-requested Live / native zoom / novice verification — 2026-09-09

Development paused for this explicit verification request. Existing unrelated Rust changes
and screenshot changes were not staged. User authorizes paid tests and agrees to arrange
real novice participants; no participant has performed a recorded test yet. Protocol and
anonymous per-person observation sheet: `NOVICE_STUDY_PROTOCOL.md`.

Native Chrome accessibility control succeeded (unlike the earlier attempt): Cmd+0 followed
by five native zoom-in commands produced the browser toolbar's `缩放比例：200%` and zoom
popup's `缩放比例：200%`. This was browser page zoom, not CSS, canvas zoom or viewport emulation.
On the existing yellow-cylinder Project, opened saved Sample Test
`b3bbdc85-5509-417c-8ca4-54893788cd8b`, opened image
`29fb71b9-0c1f-5728-9253-022646260406`, expanded the accessible annotation list, selected
the 59% candidate (selected state 1), reached Correct label / Confirm selected result,
toggled Original image and Current candidates, and closed the preview while preserving
Draft/Test URL. No edits or acceptance were submitted. Finally Cmd+0 restored toolbar
`缩放比例：100%`. Native screenshot acquisition reported unavailable, so this is interaction
and accessibility-tree evidence ONLY, not pixel-level clipping/contrast verification or
all-page/native-200% acceptance. Historical result restoration took a noticeable delay;
no timing benchmark was collected. Tested the running service, not proof that all unbuilt
HEAD changes were deployed.

Live preflight: registry lists enabled glm-5.2 and qwen3.7-flash-2026-07-15. The selected
yellow-cylinder Draft `e595c7b1-b811-429c-9e6e-86f8a27e30cb` revision 4 sample-preview reports
Qwen at DashScope and local efficientsam-ti-onnx, unknown estimated cost, request_limit 12,
but `supported:false`. Server code rejects this legacy non-LabelPipeline plan under bounded
guided sampling. Did not omit expected_revision/fingerprint to bypass that protection.
No new paid call, no full Run, publication, credential mutation or formal annotation write.

Dataset tooling documents class 2 as yellow_cylinder. reviewed_images.txt contains comments,
glob patterns and stems; its line count is NOT the reviewed-image count. Accuracy evaluation
must resolve the supplied read_manifest semantics, audit reference labels and include
reviewed negatives; unreviewed/autogenerated labels are not ground truth. No precision,
recall, IoU or accuracy result is claimed. Fresh bounded legacy-plan evaluation requires
an isolated compatible plan or explicit implementation of bounded legacy execution, not
unrestricted calls masquerading as this acceptance test.

Remaining: bounded Live evaluation, native visual QA/remaining flows, actual human sessions.

## M2 continuation — Plan policy at both Advisor and authorization boundaries

The previous verification turn yielded real native-zoom evidence and a recorded novice
protocol, but did not complete the goal. Continued from authoritative dirty application/server
changes; retained unrelated screenshots and kit files. No active test process was restarted.

The deterministic application Advisor now stops after static validation under planning_only,
persists the editable proposal and human-review state, and skips image execution. Its HTTP
adapter is TEST-build-only; this is not a newly discovered production HTTP escape. The real
LLM suggest endpoint now forces planning_only when an explicit planning authorization is
present, rather than trusting a client's false/default constraint. Existing exact goal,
destination, Provider and zero-image-budget validation remains before credential resolution.

Added the missing Model Profile ID to that authorization, independently of revision and
Provider ID. Two profiles on the same Provider can share revision numbers; accepting only
those fields did not bind the selected model. Both retained Journey planning entry points
now send the exact ID. Old clients omitting it fail closed and must reload/reconfirm; no
historical persisted version or authorization record is rewritten.

Evidence:

- Application deterministic Plan test passed (36004): valid editable Draft, persisted
  planning_only, no sample record, no Run or published version, human approval required.
- Server Plan HTTP test passed (49066): explicit positive numeric dry-run allowance still
  yields no sample execution under Plan, persisted policy true. Initial test expectation
  incorrectly used 200; corrected to the existing 201 Created contract, not changed API.
- Existing general Advisor dry-run/inspector/replay HTTP regression passed (35384), proving
  the separate execution-capable path remains functional.
- Browser `journey-ready.spec.ts --grep 'ready fixture journey'` passed (4022, 37.4s test),
  against fresh isolated `/tmp/annotagent-guided-e2e-56920` and loopback TEST model. Rejects
  altered model ID/revision, Provider, destination, goal and image-call allowance before
  sessions are created. Explicit client planning_only:false is overridden to true in the
  persisted real HTTP session. Planning makes no sample calls; separate sample approval,
  idempotency, stale scope and refresh assertions then pass.
- Web typecheck passed (97881); all 238 unit tests / 49 files passed (67222). Production web
  build succeeded in the browser harness with the existing >500kB chunk warning.
- Application/Server all-target/all-feature Clippy with -D warnings passed (51376).

These are bounded TEST model protocol checks, not Live vision quality or novice evidence.
Still pending: Composer mode/Registry picker and frozen turn configuration, queued messages,
full stop/checkpoint continuation trace, new real object UI and final six states. This does
not claim full Plan protection across all Project APIs or completion of M2/M3. No push,
remote change, real workspace write or deployment restart.

## M3 foundation — durable conversation Agent-model preference

Previous goal turn was progress: committed/tested Plan policy and exact model authorization
in 5812fbf. This turn inspected the real Registry and conversation coordinator. There was
no independent persisted conversation Agent-model selection; Project/global defaults were
the only selection source. Added the minimal preference contract, not another Registry.

Migration 0054 stores the selected Model Profile ID and a monotonic revision per owned
conversation, with immutable command inputs for retries. Null explicitly means use the
existing default resolver. GET is passive and does not create a selection or trigger probes.
POST requires expected_revision and request_id; stale multi-tab writes and changed replay
payloads fail. Exact old retries return current selection without rolling it back. This
preference is NOT an execution grant, plan approval, Model Profile revision snapshot or
authorization to send history to another Provider.

Application validates that the Registry profile/provider exists and the profile declares
text generation, tool calls and structured output. Unknown health is preserved, not probed
or relabeled Available. Actual invocation must still apply the existing availability and
credential checks. Selection does not rewrite global/project defaults or Workflow bindings.
An admitted selection command remains safely readable after capabilities change; a new
incompatible choice is rejected. Endpoints reuse existing project owner resolution and
same-origin-protected workspace router at conversations/:id/agent-model.

Verification in isolated temporary workspaces:

- Storage ownership/CAS/restart/old retry test passed initially (48335); no task, model-call
  receipt or grant created. A first compilation caught unsupported rusqlite u64 conversions;
  changed to checked i64 database conversion, keeping the public u64 revision.
- Application Registry/unknown-health/unchanged-default/no-session/no-Run test passed
  (29230). Initial test incorrectly changed model capabilities at the same revision;
  fixed the fixture to create a new revision, preserving immutable Registry semantics.
- HTTP GET/POST/duplicate/stale-version/foreign-conversation/no-session test passed (59569).
- Strict Clippy passed (62527) before the final HTTP test addition; final recheck recorded
  below after completion. No Web changes or new browser evidence in this backend slice.

Not yet connected: Composer picker, frozen send configuration, invocation consumption of
this preference, Provider-switch consent and active-request invariance trace. The endpoint
currently saves a preference only; it MUST NOT be represented as a working model switch in
the UI until those consumers and tests are connected. Plan/Execute selector and queue still
remain. Goal/M3 incomplete; no push, remote changes, real workspace writes or paid calls.

Final current-source Storage regression passed (70787); Storage/Application/Server strict
all-target/all-feature Clippy and cargo fmt --all --check passed (77699). git diff --check
also passed. This is targeted backend verification, not a full workspace or Web regression.

## M3 connection — Registry picker and next-authorization model resolution

Previous turn was verified progress (33b5a99). Connected its durable preference to preparation
of new Schema, Builder, candidate-feedback and future-rule scopes through one application
resolver. Explicit model IDs from an existing consent always win; the existing runtime call
still uses its frozen model/provider configuration. New scopes consult the conversation
preference, then the original Project/global resolver when the preference is null. The
combined Journey already calls these same Schema/Builder scope functions, so it reuses this
boundary without replacing visual-model selection. Actual health/credential validation and
exact scope confirmation remain mandatory. Selecting a model is never consent to transmit
history/images or a silent edit of an existing approved request.

Added Composer AgentModelPicker: real Registry reads, searchable Provider/account optgroups,
model capabilities/status, durable selection, same-command retry, stale-selection reload,
existing model-management return path, Escape/focus return. Search Enter cannot implicitly
submit the surrounding Composer. First explicit selection can create the Project's existing
canonical conversation; mount only reads. Unknown health is displayed truthfully. No key
form, model probe, background inference or duplicated Model Registry. Existing consent can
still run with its original model; picker copy therefore says **next authorization**, not
that changing a preference replaces an already approved operation.

Evidence:

- `agent-model-picker.spec.ts` verifies changing from TEST Alpha account to TEST Beta:
  fresh scope uses Beta, explicitly frozen Alpha scope keeps identical scope_hash. Search
  Enter preserves unsent message without `/send`; selection survives reload; Escape returns
  focus; 390px has no document horizontal overflow. Before explicit inference only selection
  POSTs occur and reserved calls remain zero.
- The same test explicitly starts a slow, authorized loopback TEST Schema request, observes
  backend `reserved`, changes preference to Beta, then observes one `completed` call. The
  TEST provider echoes the actual received Alpha model in its saved response rationale;
  preference remains Beta. This is real HTTP/ledger timing with scripted output, not Live
  model quality. First attempt waited for completion before observing reserved and failed;
  corrected to concurrent observation. Second attempted a nonexistent evidence.remote_model
  field; added truthful TEST request echo rather than claiming that missing field exists.
- Current-source picker + all four atomic Send/recovery browser tests passed (41057,
  5/5, 30.4s overall), isolated `/tmp/annotagent-guided-e2e-61110`. No 8787 restart.
- Typecheck passed (91892); Web 238 unit tests / 49 files passed (44665). Web production
  build passed in the browser harness with the existing large-chunk warning. Strict
  Application/Server/TEST-fixture Clippy passed (73337); earlier resolver Clippy passed13346.
- Real application screenshots (TEST data, not kit images):
  `agent-first-model-picker-desktop.png`, `agent-first-model-picker-mobile.png`.
  Inspected both: popup controls fit, but legacy inline Journey forms and the expanded task
  sidebar in the mobile resized state remain; these are not final six-state acceptance.

Still pending: freeze mode/model configuration at Send/queued-message admission (current
boundary is new authorization preparation), per-turn Plan/Execute controls, queuing,
cross-provider historical replay/summary tests, full stop→checkpoint→continue trace and
the remaining visual/feedback integration. Existing narrow consent proves no automatic
new transmission, not a general multi-provider conversation replay implementation.
No push, remote changes, real-workspace mutation, paid call or prototype output introduced.

## M3 Send boundary — concrete model preference snapshot

Previous goal turn made verified progress (91409e3). Found the remaining interval between
Send and later scope preparation: reading only current preferences could reinterpret a
message after another selection. Send now records its concrete preference in the existing
atomic receipt transaction, alongside task/message admission. No separate message or task
store, execution grant, Runtime or migration was required.

The input may carry observed agent_model {revision, model_profile_id}; the storage transaction
compares it to the current preference before creating either message or task. A mismatched
selection fails without admission. Exact retries return the original receipt/snapshot even
after another selection or restart. New receipts always contain a snapshot; deserializing a
historical receipt without one leaves it absent, never inventing a historical model.

Initial Schema and candidate-feedback resolution now consult the source Send's concrete
Model Profile ID before the current preference. Explicit saved consent IDs still win and
retain their original exact scope hash. Independent Builder/future-rule preparations remain
new authorized requests, while a combined Journey carries its already frozen consent model.
No Model Profile revision, Provider config or budget is granted by the message snapshot;
those still freeze and validate at explicit authorization.

Limits deliberately retained: receipts without a selected Model Profile ID (including
Project-default/unconfigured messages) still resolve during setup/authorization; the current
Composer does not yet attach its locally observed preference for client-side CAS recovery.
Server-side admission snapshots therefore work, but displayed-choice multi-tab rejection,
safe rejected-send re-editing, resolved default-model snapshot and full queued turn config
remain to connect. Do not claim this is the entire mode/model/budget Send envelope.

Evidence:

- Storage Send tests 3/3 passed (49899), including snapshot restart/retry, stale-input
  no-message/no-receipt, original transaction rollback and legacy receipt compatibility.
- Web recovery parser now validates optional snapshot fields and keeps them in exact-command
  comparisons; typecheck and all 239 tests / 49 files passed (80803).
- Picker/Send browser regressions passed 5/5 (73611), fresh isolated TEST workspace
  `/tmp/annotagent-guided-e2e-62546`. New assertions send a goal with Beta, select Alpha,
  verify that goal's unqualified Schema preview still uses Beta, retry Send with the same
  immutable receipt, and reject a new stale-choice Send without increasing message count.
  Existing slow in-flight Alpha/Beta boundary and refresh/lost-ack Send assertions pass.
- Strict Storage/Application/Server Clippy passed (61778); cargo fmt --all --check and
  git diff --check passed (5965). Production Web build in browser harness passed with
  existing large-chunk warning. These are TEST transport checks, not Live accuracy evidence.

Goal remains incomplete: Plan/Execute Composer control, queued input, full continuation
trace and substantial remaining view convergence are still pending. No push, real-data
cleanup, Published Version edits, remote changes or paid model calls.

## M3 Composer observed-choice CAS and rejected-send recovery

Previous goal turn was progress (7b1ef2f). Connected the optional observed model selection
to the actual Composer: Picker reports the passive server selection to its parent, Send
freezes that displayed revision/ID before asynchronous admission, and the frozen pending
command persists it. While a new selection is unresolved, ordinary new Send waits; direct
Stop remains independent. The API client now uses the shared SendCommand type rather than
a stale duplicate inline type (caught by typecheck before runtime tests).

Added a typed StorageError for stale model choices, mapped only at Send to HTTP409 with
code send_model_selection_changed and admitted:false. This is an atomic pre-write rejection,
not an unknown network outcome. The UI exposes recovery ONLY after that exact response.
It fetches current choice, preserves original text/image/candidate/task/schema, creates a
replacement command ID and persists it before dropping the rejected pending ID. No POST
occurs until the user separately presses Send updated request. Enter cannot bypass the
required review. Generic failures/lost acknowledgements still retry the original command;
they never get silently replaced. Refresh retains the pending command; if definitive
rejection state was not local, retry gets the same server rejection before replacement.

Tests on current sources:

- Browser picker + Send 5/5 passed (26270), isolated
  `/tmp/annotagent-guided-e2e-63660`. Test changes selection externally after UI load,
  confirms UI POST includes the stale observed snapshot and receives 409, then checks
  review causes no Send, replacement changes only message ID/model snapshot, and exactly
  one message is finally added. Existing in-flight model test and all four original
  Send admission/lost-ack/refresh tests remain green. This is TEST transport, not Live.
- Stop browser suite 10/10 passed (29607), separate isolated
  `/tmp/annotagent-guided-e2e-63954`: empty stop, reserved-call cancellation, frozen target,
  late completion, lost acknowledgement, revoked authorization, composition and task scope.
  This preserves existing Stop contracts, not yet the full new UI continuation trace.
- Storage Send 3/3 + fmt passed (7023); Web typecheck passed before browser execution,
  239 unit tests / 49 files passed (33728), strict Storage/Application/Server all-target/
  all-feature Clippy passed (7179); git diff --check passed. Browser harness production
  builds passed with the existing large chunk warning.

Still incomplete: resolving/fixing default models at Send, mode/turn policy, queue draining,
continuation/remaining-budget trace, compact actual plan/human objects and six final states.
No real workspace, Published Version, credentials, remote or push operation changed.

## M2 continuation — planning-first entry, 2026-09-09

The preceding user-reply turn only confirmed novice-study arrangements; it was not
implementation progress. Resumed against HEAD fd2b8c8, with unrelated screenshot
changes left untouched and no applicable repository/ancestor AGENTS.md found.

Code inspection found that the standalone conversation Builder already enforces
`planning_only: true` and `maximum_dry_runs: 0` in Application, but a fresh goal
defaulted to the combined Schema/Builder/sample authorization UI. New goals now
start at the text-only Schema preview, and their saved labels lead to the standalone
planning Builder. The UI explicitly states possible text-model fees and separate
image authorization. This changes the real service path, not just a Plan badge.

Existing combined Journeys remain available through an explicit review action.
Opening that action does not approve it. Saved Journeys restore by exact Schema
identity and repair provenance, including a consent saved before Builder admission;
standalone pending retry envelopes keep precedence. No saved consent is rewritten.

Verification:

- Web typecheck and production builds passed; 239 unit tests / 49 files passed.
- New `agent-plan-entry.spec.ts` plus Send recovery: 5/5 passed (41479), isolated
  `/tmp/annotagent-guided-e2e-65599`. It drives Registry selection, actual Send,
  text authorization, persisted label proposal, refresh, Builder preview and explicit
  combined-panel opening. Only Send, task-selection and the approved Schema proposal
  POSTs occur; the task ledger records exactly one reserved call. Refresh and panel
  changes add no mutation. The first test attempt rejected the legitimate saved
  task-selection POST; the assertion now explicitly allows that metadata operation,
  not arbitrary mutations.
- Existing initial Journey classification service regression 1/1 passed (7170),
  isolated `/tmp/annotagent-guided-e2e-65430`; this is service-chain evidence, not a
  claim that all older UI tests using the removed Save message label pass.
- Application `planning_policy_rejects_model_requested_dry_run_even_with_positive_sample_budget`
  and `deterministic_advisor_plan_stops_before_sample_execution` both passed.
- All model traffic was the explicitly marked local TEST protocol fixture, not Live
  accuracy evidence. Existing production chunk-size warning remains.

Visual inspection of the actual application still shows oversized legacy goal/status
forms. The partial authorization screenshot is not one of the six final deliverables.
Remaining M2 work includes persisted Composer Plan/Execute mode and per-turn policy,
the scoped transition into execution, and compact real Plan/approval objects. The
default-path change does not establish that all Plan permissions or the complete
Agent-first goal are finished. No push, remote change, user workspace cleanup or
Published Version mutation occurred.

## M2/M3 continuation — frozen message mode, 2026-09-09

Previous goal turn made verified progress in c4c4c5e. This continuation retains that
planning-first entry and adds `ConversationSendMode` (`plan` / `execute`) to the
atomic Send input and receipt. The existing SQLite JSON receipt transaction stores
it with message/task/model scope; no new executor or preference database was added.
Changing the mode under the same message ID is rejected, and server restart preserves
the original receipt. Missing legacy mode stays missing, not fabricated execution
authority. Unknown enum values fail deserialization.

Composer defaults to Plan and exposes an accessible next-message selector. It freezes
the selected mode synchronously with the message before asynchronous work, persists
it in the retry envelope, disables edits to that uncertain command and checks the
returned mode. Definitive model-conflict replacement retains the original mode.
Changing the selector itself has no HTTP effect and does not interrupt ongoing work.

Initial goal restoration reads the server Send receipt: Plan uses standalone text
planning; an explicit Execute message offers the existing combined authorization.
It does not execute on mount/Send. Separate exact operation grants remain mandatory.
Existing saved authorizations/Journeys take precedence over this entry preference.
The selector is **requested behavior**, not a wildcard grant or proof of a complete
per-turn capability middleware. Follow-up queue dispatch/policy and the complete
Plan mutation-permission audit remain unfinished; no in-flight turn is rewritten.

Verification:

- Storage Send 4/4 (17519), including mode conflict rollback, independent next-message
  mode, restart, legacy omission and invalid enum; fmt check passed (36157).
- Web 240 unit tests / 49 files (19934). Typecheck initially found a stale inline
  POST receipt type; switched it to the shared SendReceipt type, then typecheck passed.
- Picker + Plan entry + Send 6/6 (30897), isolated
  `/tmp/annotagent-guided-e2e-66716`, including actual in-flight model switching.
- Expanded Plan/Execute + frozen retry + Stop browser suite 16/16 (49959), isolated
  `/tmp/annotagent-guided-e2e-66929`: Execute Send and reload yield zero authorized
  and reserved calls; later Plan selection does not rewrite its receipt or entry;
  an altered-mode retry is rejected; uncertain Execute send restores as Execute.
  Plan goal stays text-only after selecting Execute for the next message. All ten
  existing Stop/selection/lost-ack/composition/independent-task tests passed.
- Strict Storage/Application/Server all-target/all-feature Clippy passed (17475).
  Browser production build passed with the existing large-chunk warning. Only the
  local TEST protocol fixture was invoked, not a paid model or accuracy evaluation.

No user workspace cleanup, Published Version modification, remote change or push.
Full Agent-first completion, six final screenshots, queue/continuation, compact thread
objects, native zoom across the complete flow and live/human validation remain open.

## M1 continuation — remove redundant thread context, 2026-09-09

Resumed after verified mode work in 62e201a. The initial welcome/input explanation
now appears only in an empty conversation, not above every existing task. An explicitly
selected goal remains identified in its saved message and navigation, without a button
to select itself again. Other messages retain their goal-selection action; legacy
messages without an explicit task URL can still be admitted through that action.
No goal, history or selection API was removed.

An actually loaded, entirely empty human-request list uses a compact inline status and
refresh action. Errors, loading state, pending help, selected completed requests,
deferred work and other-goal history retain their existing treatment and controls.
Existing five rendering regressions are preserved; two new tests cover the compact
empty boundary and retained actionable/foreign history. No failure is reported empty.

Actual screenshots revealed that shrinking a desktop window kept the default expanded
task navigation. It now follows the media query until the user explicitly chooses a
collapsed/expanded state. Manual choices are retained while mounted; cross-reload
layout preference persistence is still outstanding.

Verification: typecheck and production build passed; 242 unit tests / 49 files passed
(16071). Thread context + Plan/Execute + Send browser tests 7/7 passed (18091), isolated
`/tmp/annotagent-guided-e2e-68455`. They check independent task switching, URL refresh,
no redundant selected-goal action, read-only request refresh, zero inference ledger
entries for selection, 1440/1024/390 widths, automatic nav collapse and manual reopening.
This is viewport evidence, not native 200% or human usability validation.

Actual TEST application screenshots inspected: `agent-thread-context-1440.png` and
`agent-thread-context-390.png`. They show reduced repeated context and the mobile
navigation correction, but still reveal legacy planning forms and oversized actions.
They are partial regression evidence, not the six final designed states. Older tests
asserting the selected goal's redundant aria-pressed button need migration to saved
context/navigation assertions; no full legacy browser-suite pass is claimed.

No backend/credential/Published Version/user-workspace mutation, push or remote change.
Full compact thread/Plan blocks, attachment integration, queue/continuation and final
visual/accessibility evidence remain unfinished.

## M1/M4 continuation — real Composer image attachment, 2026-09-09

Previous turn made verified progress in 6f3cc9b. Composer now exposes one PNG/JPEG
attachment through the existing Project uploader; bulk dataset import remains in the
image pane. No endpoint, executor or model connector was duplicated. Encoded image
bytes are hashed with browser SHA-256 and matched to the refreshed owned dataset index,
consistent with the existing importer. Filename collision, duplicates and list ordering
do not choose the attachment. Empty/oversized files fail before upload; missing secure
WebCrypto fails with a specific message rather than sending unverified content.

The unsent attachment is pinned independently of the displayed canvas. A candidate
reference must be explicitly removed before uploading a replacement image. Sending
freezes the existing Image ID/hash contract with mode/model/task scope. Removing an
attachment removes only the reference, not the saved image. Failed upload leaves the
prior attachment unchanged. The original uploader still reports partial import errors.

Unsent attachment metadata is currently protected by the navigation/unload dirty guard,
including when text is empty; it is not advertised as a durable sent message. After
Send admission, existing session retry/server receipt recovery persists the reference.
Discarding an unsent attachment does not delete the uploaded dataset file. This is the
single-image contract already supported by Send, not a claim of multi-image chat turns.

Verification:
- Typecheck/build and diff check passed. Web unit suite 242/49 passed (79708).
- Attachment + Send 5/5 (29098), isolated `/tmp/annotagent-guided-e2e-69300`.
- Expanded attachment + model picker + Plan/Execute 4/4 (82960), isolated
  `/tmp/annotagent-guided-e2e-69541`: same-name distinct encoded TEST content resolves
  the right image hash, duplicate upload adds no source, corrupt input preserves pin,
  canvas changes do not retarget it, lost Send acknowledgement retries the identical
  command, reload restores the saved reference, and the task has zero model calls.
- Final attachment test including empty-text navigation guard passed (30636), isolated
  `/tmp/annotagent-guided-e2e-69711`. Native confirm was dismissed and the task stayed
  open. No claim of complete native-200%/IME validation is made from this test.
- Actual 390px Composer screenshot inspected and retained as
  `agent-composer-attachment-390.png`. Shortened the visible remove label while retaining
  its accessible name, and adjusted flex wrapping after the first screenshot showed
  filename squeezing. This TEST screenshot is partial evidence, not a final six-state
  deliverable. The existing build chunk-size warning remains.

All test images are repository synthetic fixtures; no real workspace, paid Provider,
historical Published Version, remote or push was touched. Queue dispatch, continuation,
compact Plan/tool blocks and full final regression remain incomplete.

## M3 continuation — durable follow-up inbox boundary, 2026-09-09

Previous goal turn made verified progress in 2790f5b. Inspection confirmed ordinary
follow-ups were only journal entries, with no cancellable dispatch record. Migration
0055 now records modern ordinary TaskMessage sends in an ordered coordinator inbox,
inside the existing Send transaction. It references the frozen Send input/receipt
instead of copying or re-resolving image, model, mode, task or schema context. Queue
insert failure rolls the entire Send back. Legacy messages are not backfilled.

Added owned, read-only paginated history and idempotent terminal cancellation:
`GET .../conversations/:conversation/tasks/:task/message-queue?after=:sequence`
and `POST .../message-queue/:message/cancel` with an empty, strict JSON body.
Cancellation preserves the message and its original Send receipt, changes no grant,
stops no current call/Batch and cannot be reversed by retrying the original Send.
These routes use existing same-origin workspace routing and stable Project ownership.

This is intentionally **not a finished execution queue**. Entries truthfully have
`waiting_for_dispatch` or `cancelled`; there is no pretend running/completed state.
No consumer is connected yet, and no UI says these instructions have been applied.
Candidate-scoped feedback still follows its existing explicit authorization path and
is not included in this ordinary-message inbox. Next required work: guarded dispatch
into the existing planning/feedback coordination, correct authorization for the queued
snapshot, scope-aware cancellation/redirect UI, and proving no reexecution/renewed
budget across interruption and continuation. Do not count this as requirement 8 passed.

Verification:
- Send/storage tests 5/5 passed (32753), including transaction rollback on queue failure,
  task/Project rejection, ordered snapshots, paging, cancellation/retry and restart.
- Full Storage library suite 159/159 passed (79832), including existing call-budget,
  immutable workflow, feedback, ownership, review and stop persistence regressions.
- Strict Storage/Application/Server all-target/all-feature Clippy passed (78704).
- Actual HTTP TEST model picker + Send tests 5/5 passed (53992), isolated
  `/tmp/annotagent-guided-e2e-70794`. While the Alpha planning call is reserved, the test
  admits a follow-up, reads its frozen Alpha snapshot, rejects foreign ownership and
  an injected execute field, cancels twice with the same outcome, retries the Send
  without reactivating the entry, then observes the original Alpha call complete once.
  Budget remains one reserved call; later model preference changes do not rewrite it.
- Browser production build and diff check passed; existing chunk-size warning remains.
  No Live model quality or final six-state visual evidence is claimed.

No user workspace cleanup, Published Version modification, remote change or push.
Full Agent-first goal remains active and incomplete.

## M3 follow-up planning admission — 2026-09-09

Migration 0056 binds an inbox message to one exact planning call, frozen model and
server request digest. Authorization advances the existing task ledger atomically;
it does not create another task or reset consumed calls. Admission enforces FIFO,
settled task work, unchanged request, and uncancelled source. A queued grant cannot
authorize unrelated calls even when its cumulative ceiling has remaining capacity.
Cancelled inbox entries cannot be resurrected by authorization retries or restart.
Running entries require the existing explicit task stop, not queue cancellation.
Queue reads now expose authorized/running/completed/failed/in-doubt states and the
actual call identity from persisted records instead of calling every entry waiting.

Evidence: Storage library 160/160 passed (25306); focused lifecycle test passed
(75388; status assertions also passed in 30512). Strict Storage/Application/
Server all-target/all-feature Clippy passed (34843). Tests exercise active-call
blocking, FIFO, transactional rollback, exact-call admission, preserved spend,
cancellation before reservation, restart, and completed receipt deduplication.
All data is isolated TEST SQLite data; no paid Provider call or production data edit.

Remaining: this is storage admission, not a connected queue consumer. Application
preview/consent, existing planning-service dispatch and Composer queue controls
still need integration and HTTP/browser evidence. Do not report the full queued
conversation experience or the overall Agent-first goal complete.

## M3 Composer inbox controls — 2026-09-09

The current task now has a compact, expandable inbox above Composer, backed by the
existing message-queue API. It shows saved text, image reference, Send mode and model
snapshot, real execution status and terminal cancellation history. Running calls
refer to explicit task stop; the inbox cannot disguise cancelling an active call as
removing a waiting entry. Historical completed calls do not offer cancellation.
Reads are paginated, bounded and abort on scope changes; owner-keyed component state
does not carry another task's rows. Mount/reload/poll performs GET only. Cancellation
errors remain visible across successful background reads until explicit retry or
refresh. No grant or model execution is triggered by opening the inbox.

Validation: Web 244 unit tests in 50 files passed (29422), including two new rendering
contracts. Production build/typecheck passed. Actual browser/HTTP TEST Registry +
queue test passed (52779; isolated `/tmp/annotagent-guided-e2e-73624`): inject failed
cancellation, verify the error survives the next GET, retry successfully, refresh
and observe both cancelled entries; reserved model calls remain one. Earlier run
70466 also passed. Chunk-size warning remains. Fixture is explicitly TEST, not Live
accuracy evidence. No user workspace writes, remote changes or push.

Next: connect exact queued-plan preview/consent and dispatch through existing
planning service. The inbox is usable for viewing/cancellation, but does not yet
consume supplements; it explicitly says they have not been applied. Full goal stays
active; final six-state visuals and complete stop/continue validation are pending.

## M3 queued semantic planning source — 2026-09-09

The existing Schema executor now recognizes an exact queued planning authorization.
It hashes the original task, frozen supplement receipt, full schema and remote model,
then sends original goal plus that supplement as bounded untrusted text. Newer journal
messages cannot retarget this request. No pixels are sent. Result materialization
keeps the supplement in a new Schema Draft, preserving the original Draft and task;
receipt retry after restart reuses the saved response and cumulative call ledger.
The queue source lookup is a direct owner-checked lookup, not an unbounded history scan.

Evidence: six Schema tests passed (48399, 11058), including actual TEST Provider request
capture, wrong-model rejection before inference, newer-message isolation, unchanged
old Draft, exact retry after restart and two cumulative calls. Full Application suite
155 passed / 1 explicitly ignored paid smoke (23524). Strict Application/Storage
all-target/all-feature Clippy passed (59306), after correcting test lock scope.
No Live inference, user data changes, remote changes or push.

This is the existing semantic Schema phase only, not a generic conversation response
or Workflow revision. HTTP consent/preview, prior editable-plan context, controlled
Workflow continuation and queue dispatch UI remain unfinished. Do not treat producing
a Schema Draft as satisfying the full queued instruction. Overall goal stays active.

## M3 explicit queued text-planning HTTP boundary — 2026-09-09

The existing owner-scoped queue routes now expose read-only `schema-preview` and
`schema-authorization`, plus explicit POST `schema-proposals`. Consent binds message,
model/provider configuration, exact text request digest, previous grant, cumulative
ceiling and expiry. It requires acknowledgment of unknown cost; the scope explicitly
excludes pixels, Workflow changes, publication, dataset execution and annotation
acceptance. Model configuration/credentials use existing Registry and Provider code.
The existing bounded worker and call ledger own admitted execution after HTTP loss.
Saved call recovery returns the receipt before resolving credentials or extending
the budget; in-flight duplicates cannot dispatch twice. GET never grants or executes.

Validation: strict Server all-target/all-feature Clippy passed (74291), after using
the repository-required Option style. Isolated HTTP TEST fixture test passed (95474).
Expanded queued-planning plus Registry/Composer browser suite 2/2 passed (56981,
`/tmp/annotagent-guided-e2e-76025`): reject missing cost acknowledgment, inflated
ceiling, altered request, model substitution and injected execution field; recover
authorization, reject inbox cancellation while reserved, deduplicate in-flight and
completed requests, preserve supplement in real saved Schema Draft, reject moving
consent to another message, preserve used count for the next preview, reject cancelled
and foreign queue sources. Production frontend build passed with existing chunk warning.
All model traffic was to the local TEST fixture, not a paid/live quality test.

Remaining: connect this boundary to explicit queue consent UI and carry controlled
current-plan/Artifact context into Workflow continuation. This endpoint deliberately
does not claim a semantic proposal completes a general queued instruction. No push,
remote changes or production workspace mutation; overall goal remains incomplete.
