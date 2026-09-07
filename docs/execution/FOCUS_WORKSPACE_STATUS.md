# Focus Workspace — execution record

Started 2026-09-07 on `main`, base `9b2fce6` (four local commits ahead of origin).
No AGENTS.md found in the repository or its ancestor directories. Fourteen pre-existing
modified acceptance screenshots are preserved in `/tmp/annotagent-focus-baseline.74ZElm`.
No push, remote changes, history rewrite, old conversation keys, real-data cleanup or live
model inference. Mutating browser tests use the existing isolated 8791/8796 fixture workspace.

## M0: verified baseline and screen contracts

Inspected current React route parser/builders, App shell, Project Guidance, query generation
guards, creation, sample-feedback store, Review decisions/revisions, Run/Batch control,
management/export endpoints, tokens, CLI/TUI and actual Cargo/npm scripts. Existing APIs,
immutable versions and server ownership checks remain authoritative. No second workflow,
model registry, navigation registry or ProjectStage is introduced.

| Current journey | Decisions and navigation | Concurrent regions / failure and recovery |
| --- | --- | --- |
| Ready fixture model | Projects → create dialog (images + goal + label/output) → Project → Automation → Ask → sample test → preview dialog | Sidebar + global header + project/build navigation + toolbar + gallery + modal; sample choice is local state, lost on refresh; goal/test/feedback themselves persist. |
| Missing model | Create → Project → Automation → inline Registry preparation → same Draft | Provider/Model/Plugin forms are reused already; URL stays fixed and return refreshes model availability without inference. Current creation still blocks its backdrop and does not integrate preparation. |
| Existing project | Project Review → source project Run/Debug → Review → Export; Project Runs / pipeline management / Trash | Server-owned review transitions, revisions and lifecycle management exist. Review keyboard handlers lack IME guards. Multiple simultaneously visible queue/editor/inspector and navigation regions compete with the image. |

Baseline browser execution: existing full E2E suite runs these paths against its explicit
protocol fixture (not live-model accuracy). Captures written by that suite are copied to this
task's evidence directory before restoring pre-existing screenshot files.

Baseline result: **54/54 E2E passed** (`/tmp/annotagent-focus-baseline-e2e.log`).
Evidence: `focus-workspace/before-prepare.png`, `before-sample.png`, `before-review.png`,
`before-export.png`; `before-no-model.png` captures the existing same-task preparation.
Red-first test requires canonical Projects, page-level creation, no global Sidebar in Focus,
and no mutation from mount/reload/navigation. It intentionally fails against the baseline.

| Screen | Main question / main action | Exit and recovery identity |
| --- | --- | --- |
| Projects | Which work should I continue? / new annotation project | One canonical `/projects`; existing project management and cross-project indexes retained |
| Prepare | What should these images produce? / save actual goal and uploads | Parent Projects; unsaved local files are not represented as uploaded; stable Project/Draft once saved |
| Sample | Does this result match the task? / save sample decision | Parent Project; Project + Draft + Test + stable Image ID; feedback remains sandbox-only |
| Run / Review | What needs inspection or a decision? / existing server-owned control/decision | Same Project, Run/Image/Review identity; no mount-triggered mutations |
| Export | What will be delivered? / actual exporter action | Existing Project export readiness/report, not a fabricated local folder action |

## Stages and current scope

- M0: baseline + red-first route/focus tests.
- M1: canonical inventory, page-level preparation, same-data Focus shell and sample canvas.
- M2: progressive model setup/Builder/details, authorization and cancellation using existing capabilities.
- M3: existing execution/review/export and management return paths; recoverable publication boundary.
- M4: restoration, keyboard/IME, responsive evidence and regression checks.

Known pre-existing implementation gaps: aggregate scoped sample authorization and cancellable
sample execution, durable combined publish/start receipt, sample label/new-object corrections,
and real 200% zoom evidence. Do not advertise these as solved by layout changes.
Real-person usability testing has not been performed; automated tests are not a substitute.

## M1: same-data focused preparation and sample inspection

Implemented canonical `/projects` (`/`, `/home`, `/dashboard` replace-redirect), searchable
inventory and page-level creation. `FocusHeader` is the existing App shell's alternative
header, not a second application or execution engine. It removes the global Sidebar and
repeated breadcrumb/build-step bars on task routes. The project menu retains canonical
data, automation, processing history, Review, export, labels/settings and Trash destinations.

Creation reuses real upload/schema APIs and previews selected images. Explicit dirty guards
protect locally selected files and goal fields. Saving succeeds before navigation and does
not require an LLM or initiate inference. Existing Registry forms remain available in the
same Draft. Sample inspection is a page canvas, not a fullscreen/modal preview. Stable
`image` joins Project/Draft/Test in the typed URL; refresh restores the same sample and its
server-persisted feedback. Direct bbox edits, Undo, original/candidate toggles and structured
feedback use the existing sandbox revision API. Confirmation waits for save success before
advancing. Formal Review/Annotation APIs are not used by this editor. Diagnostic stages are
collapsed, with honest confidence/geometry and sandbox scope notices near the image.
Removed the canvas's default wheel interception; explicit Zoom/Fit/Pan controls remain.

Verification: **55/55 E2E**, **71/71 unit tests**, TypeScript check, production build (E2E
preflight) and `cargo fmt --all --check`. Requested all-feature Rust clippy/test/build also
passed; Rust tests reported **500 passed, 5 ignored** including doc-test result groups.
Logs: `/tmp/annotagent-focus-{e2e,unit,clippy,rust,build}.log`. Browser evidence is isolated
fixture execution, not live model quality: `after-prepare`, `after-sample`, `after-no-model`,
`after-run`, `after-review`, `after-export` in `focus-workspace/`. These capture the current
checkpoint; later stages still need to reduce the interiors of Run/Review/Builder.

M1 limitations: creation still saves before the existing Project → automation → sample
steps, rather than automatically synthesizing and testing; there is no authorization to
silently add those calls. Sample label changes/new boxes are not supported by the existing
feedback contract. Do not count these as completed acceptance criteria. Current browser
coverage includes 1024/720/640 CSS-pixel reflow and phone paths; **native 200% browser zoom
has not been tested**. No real-person usability test performed.

Commits: M0 `a4e36f9`; M1 is recorded by the next local commit containing this ledger.

## M2 checkpoint: progressive setup and truthful Draft progress

M1 commit: `a6a68f2`. Model preparation now occupies the current task content instead of
stacking Registry forms above the entire Builder. It reuses the same forms, refreshes real
compatibility when returning, retains the Draft URL and does not resume inference. Advanced
plan editing is collapsed; explicit Open/Create Draft actions expand and focus it. Pipeline
management stays outside that disclosure and remains directly reachable. Raw running tool
actions and budget counters no longer expand automatically; phase/status/cancel and outcome
repair controls remain visible. Counters describe the real tool budget, not completion %.

Autosave now distinguishes unsaved/saving, failure/conflict and saved snapshots. Navigation
and refresh guards protect unsaved Draft edits. New browser regression forces PATCH failure,
verifies the error state and retained name, rejects a leave request, then explicitly discards
the local edit. It does not mutate real Workspace data. Four additional browser checks cover
preparation at 1440×900, 1280×720, 1024×768 and 390×844 (`prepare-*.png`).

Remaining M2 boundaries: no scope-bound sample approval receipt or cancellable synchronous
sample execution has been added. Registry panel selection itself resets on refresh, while
Project/Draft remain stable. Advanced-editor expansion resets on refresh and is one click
away. Do not claim these acceptance items complete. No new authorization or automatic paid
operation is inferred by entering, refreshing or leaving either screen.

M2 checkpoint verification: **60/60 E2E passed**, TypeScript and production build passed;
`/tmp/annotagent-focus-m2-verified.log`. Includes unchanged security, deletion/restore,
publication, same-project navigation, slow poll and dual-tab conflict regressions.

## M3 checkpoint: full-width results and continuous object review

M2 checkpoint commit: `1c7c4b1`. Run Results gives the image full width; source identity is
a compact row and the actionable Review/repair prompt follows the canvas. Pause/Resume,
Cancel, history and deletion/Trash controls are retained. Zero final candidates no longer
hide an available source image; a fixture regression proves that intermediate annotations
are not substituted and no-target is not described as proof of absence.

Review detail opens with queue and Inspector collapsed. Opening either auxiliary region
closes the other. Risk and object-level decision scope remain at the canvas; source evidence,
editable labels, attributes and revision history stay in the existing Inspector. The original
view has no overlays. Image lookup uses stable annotation Image ID, never index-zero fallback.
Box manipulation can begin directly, preserving the existing undo/save boundary. Compact
annotation-list controls retain the keyboard alternative. Missing confidence is not rendered
as 0%. Sample confirmation explicitly distinguishes a selected result from the whole image;
confirming one result no longer advances past other objects in the sample.

The existing Review → source Run → Review and export/download paths are reused, not copied.
Exports and lifecycle CRUD retain their server-owned rules. Publication and dataset start
are **still separate existing operations**. The requested durable exact-revision/test/scope
approval + idempotent publish/start receipt is not implemented; do not describe this checkpoint
as completing that boundary. The current sample feedback contract still cannot add missing
objects or relabel candidates. Run candidate selection across Results/Debug is not yet URL-persisted.

M3 checkpoint verification: **60/60 E2E passed** (`/tmp/annotagent-focus-m3-done.log`),
typecheck and production build passed. Includes zero-target source-image inspection,
queue/Inspector alternation without losing edits, scoped Review round trip, management and
export. `review-focused.png` and `sample-{1440,1280,1024,390}.png` are current fixture captures.

## M4 checkpoint: recovery, keyboard and visual verification

M3 checkpoint commit: `953e69e`. Shared shortcut guards exclude text controls, composition
events (`isComposing` and key code 229), already-handled events and open dialogs. Review
Undo does not override text undo. Browser checks type Chinese text, dispatch composition
events and press A while rejection is open, checking that no acceptance request is sent.
This is automated IME-event coverage, **not a real OS IME or screen-reader session**.
Autosave no longer updates an aria-live elapsed-seconds counter every second. File controls
inherit the design system. Output examples are explicitly illustrative, not inference results.
Classification output labels remain visible even with the equivalent annotation list closed.

Screenshot inspection caught the old generic `.breadcrumb` selector still visible on the
sample route; it is now removed in Focus. Review's SVG viewport adapts to available desktop
height with `xMidYMid meet`; source-image aspect ratio is checked in the browser. No image
filter, cropping, fullscreen API or simulated progress was added. Four sample/preparation
viewport captures use actual app APIs and isolated fixtures. Legacy samples without terminal
projection no longer present intermediate boxes as final or enable final confirmation.

### Entrance and data reuse audit

| Entrance | Treatment | Unique existing destination / data |
| --- | --- | --- |
| Home + Projects | merged | `/projects`; legacy aliases replace-redirect |
| Create Project | page-level task | same YAML/schema, uploaded images and Draft creation APIs |
| Project overview | retained | server Guidance recommendation; other active work stays accessible |
| Build step bars / global Sidebar | absent from Focus only | existing management navigation retained |
| Model preparation | on demand | existing Provider / Model / Plugin components, same Draft; no auto-run |
| Advanced Draft editor / tool trace | collapsed | same editable Draft, same persisted Agent session; explicit Open expands editor |
| Pipelines and Versions / Trash | retained | existing lifecycle preview, delete, archive, restore, clone, default-version controls |
| Run / Debug / Review / export | retained and focused | server owner IDs, existing query layer, immutable versions and formal annotations |
| Review queue / Inspector | exclusive auxiliary areas | no duplicate CRUD; edits stay in the same Review component |
| Offline example | secondary, explicitly labeled | not a live model result and not the default project inventory |

### Acceptance matrix (checkpoint, not release approval)

“Covered” means the named automated fixture path passed, not real-person usability or live
model accuracy. Rows marked partial/open remain required work; the whole requested release
is **not complete**.

| # | Status | Evidence / remaining gap |
| --- | --- | --- |
| 1 | Covered | Focus route test + screenshots: no Sidebar or duplicate Build bars |
| 2 | Partial | Project/task/parent/connection visible; business save/run states remain in task content, not uniformly in Header |
| 3 | Covered | Real file previews, generated internal IDs, output illustration, four viewport tests |
| 4 | Partial | Goal + label save without LLM; creation primary remains Save, not model-connect/authorized generation |
| 5 | Partial | Registry return keeps exact Draft URL; setup panel sub-selection is not refresh-persisted |
| 6 | Covered | No-model preparation return E2E; persisted goal/images remain; unsaved setup field guard needs further audit |
| 7 | Partial | Real phase/cancel and actual model-call summary retained; continuous prepare→Builder→sample orchestration not implemented |
| 8 | Open | No exact model/destination/scope/budget authorization receipt for sample execution |
| 9 | Covered | Current terminal projection canvas + legacy projection rejection + zero-target image test |
| 10 | Partial | Direct box edits and structured feedback work; missing-target addition and relabeling in sandbox not implemented |
| 11 | Partial | Original/result, existing Crop parent linkage pass; selected candidate/mode not fully persisted across Debug |
| 12 | Covered | Sample feedback uses sandbox endpoint; selected result vs image vs formal Review scope explicit |
| 13 | Retained | Existing Start full Run enters owning Project Batch; combined confirmation not implemented |
| 14 | Partial | Existing active-Run locks/idempotency regressions pass; durable combined publish/start idempotency absent |
| 15 | Open | Existing published version survives; no unified recoverable approval/start receipt |
| 16 | Partial | Existing Run/Batch pause/resume/cancel remain; synchronous sample API cannot safely cancel |
| 17 | Covered | Existing accepted/failure summaries + empty projection source image, no automatic filtering to Review only |
| 18 | Retained | Existing save-before-advance and dirty guard; failed Draft save explicitly browser-tested; network-failed formal Review needs dedicated expansion |
| 19 | Partial | Owner/Review/source/queue round trips and local edit retention pass; all selected-object/filter contexts not fully URL-persisted |
| 20 | Partial | Existing Review completion and export next step pass; current endpoint returns queue on last item, not retained last-item canvas |
| 21 | Covered | Existing Export readiness and completed download report E2E; no fake local-folder action |
| 22 | Covered | Existing lifecycle deletion/restore/Trash suite and project menu kept; no CRUD removed |
| 23 | Covered | Existing cross-owner rejection and deep-link suite; sample stable image refresh added |
| 24 | Covered | Existing late Agent poll, query generation and two-tab revision conflict tests |
| 25 | Partial | Sample/Review/creation dirty guards and failed Draft autosave; remaining setup-specific unsaved input audit |
| 26 | Partial | Primary-action regression passes; Builder still has more simultaneous decision regions than target |
| 27 | Partial | No fake progress or final geometry added; legacy zero-cost displays outside changed canvas surfaces still need complete audit |
| 28 | Partial | 1440/1280/1024/390 preparation/sample; existing Run/Review/export reflow tests; native 200% zoom not executed |
| 29 | Partial | Keyboard/composition simulation, focus and reduced-motion E2E; actual OS IME and screen reader not executed |
| 30 | Covered with skips | Rust all-features + Web suite; 5 explicitly ignored real model tests, no live inference |

### Outstanding work in priority order

1. Complete the requested two-screen generation/test journey with a server-enforced bounded
   scope authorization and truthful cancel/recovery behavior, reusing current application services.
2. Add exact Draft revision + current Sample Test + model/data scope approval and a durable,
   idempotent publish/start receipt. Do not present the two existing buttons as equivalent.
3. Extend sandbox feedback for missing objects and label edits; preserve queue/candidate/mode
   context and the last reviewed image. Persist setup return/panel context as a validated typed route.
4. Finish Builder decision-region convergence and full cost/saved-state audit; run actual zoom,
   IME, screen-reader and 5-person usability tests. **No real-person usability test was performed.**

No new model, plugin, general Agent or annotation engine was added. Published Versions,
credentials, original images and real Workspace history were not edited. Original unrelated
screenshot modifications are restored from the M0 backup after browser runs, not committed.

M4 browser verification: **60/60 E2E passed**, **77/77 unit tests**, TypeScript check and
production build passed. Logs: `/tmp/annotagent-focus-release-e2e.log`,
`/tmp/annotagent-focus-release-unit.log`. Production bundling retains a >500 kB chunk warning;
this checkpoint does not claim to solve bundle splitting. The live 8787 health endpoint is
OK and its HTML references the same asset hash as the tested `web/dist/index.html`; no
real-workspace server restart was required and no live model was called for verification.

## Final checkpoint verification and local history

Re-ran the requested Rust commands after the final code changes:

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo test --workspace --all-features`: **500 passed, 0 failed, 5 ignored**.
- `cargo build --workspace --all-features`: passed.
- `npm --prefix web run typecheck`, `test`, `build`, `test:e2e`: passed (77 unit, 60 browser).
- `git diff --check`: passed. `diff -qr` confirms the original screenshot directory exactly
  matches the pre-task backup, including the user's 14 pre-existing uncommitted PNG changes.

Final Rust logs: `/tmp/annotagent-focus-final-{clippy,rust,build}.log`. The five skips are the
explicitly opt-in billable Provider smoke and real PIDNet, RF-DETR, SAM, YOLOX weight tests.
They were not reclassified as passed. No native 200% zoom, real screen-reader, OS IME or
real-person usability session was performed. All screenshots remain automated fixture evidence.

| Stage checkpoint | Local commit |
| --- | --- |
| M0 baseline / red contract | `a4e36f9` |
| M1 focused same-data preparation/sample | `a6a68f2` |
| M2 progressive setup and save protection | `1c7c4b1` |
| M3 image-first results and object Review | `953e69e` |
| M4 recovery, keyboard and responsive evidence | `a64df07` |

Branch remains `main`. At M4 it is nine commits ahead of origin (four pre-existing + five
stage commits); this final record is a separate local documentation commit. No push, remote
change, reset/rebase/amend or destructive checkout was executed. Existing GitHub `origin`
and Tsinghua `tsinghua` remotes are unchanged. The **open rows in the acceptance matrix remain
open**; this is a tested implementation checkpoint, not full Focus Workspace release approval.
