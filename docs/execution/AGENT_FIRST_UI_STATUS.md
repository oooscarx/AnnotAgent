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
