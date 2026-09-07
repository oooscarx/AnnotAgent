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
