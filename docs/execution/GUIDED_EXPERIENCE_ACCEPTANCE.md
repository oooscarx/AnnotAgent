# Guided Experience Alpha Acceptance Ledger

Updated: 2026-08-30

Status values: `PASS`, `PARTIAL`, `OPEN`, `MANUAL`.

Milestone 0 records verified baseline behavior. `PARTIAL` means a real foundation exists but the exact Guided Experience requirement is not yet satisfied.

## A. Information architecture

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Global primary navigation has at most five entries | PASS | `PRIMARY_NAVIGATION` and browser audit show exactly five. |
| Workflows is not a primary entry | PASS | Project Build owns Pipeline; legacy route canonicalizes. |
| Models is in Settings | PASS | `/settings/models`. |
| Skills/Capabilities is in Settings | PASS | `/settings/capabilities`. |
| Global Runs is not silently filtered by active Project | PASS | URL/E2E proves hidden local state is ignored; an explicit filter is visible. |
| Project maintains persistent context | PASS | Project and Build use canonical Project URLs; route content waits for server load. |
| No duplicate Workflow or Inspector entry | PASS | Automation edits only under Project Build; Artifact Inspector only under Run Detail. |

## B. Guidance

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Rust backend computes Project Guidance | PASS | Application `derive_project_guidance` and persisted projection are authoritative. |
| Exactly one Primary Action per Project state | PASS | DTO has one required `primary_action`; state-priority tests cover every stage. |
| No data → Add images | PASS | Engine and HTTP endpoint return `add_images` with the Data URL. |
| No Label → Define labels | PASS | Deterministic state test passes. |
| No Automation → Choose automation | PASS | Real application lifecycle reaches `needs_automation`. |
| Missing model has a repair action | PASS | `needs_model_binding` returns a Settings repair action. |
| Untested → Test samples | PASS | Persisted sample-test absence returns `ready_for_sample_test`. |
| Active Run → Open active run | PASS | Active Run/Batch have highest priority; deterministic conflict test passes. |
| Review exists → Review results | PASS | Completed Run with pending review returns `needs_review`. |
| Completed work → Export | PASS | Completed reviewed Run returns `ready_to_export`. |

## C. Creation and Build

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| User need not enter an internal ID | PASS | Browser journey creates and reopens a Project using generated workspace/task/Label IDs; Advanced overrides remain optional. |
| New Project wizard creates a real Project | PASS | Four guided steps call real Settings/create/import/Advisor APIs and open the persisted Project. |
| Build is a continuous four-step flow | PASS | Data, Labels, Pipeline, Test & Publish routes exist. |
| Refresh restores Build step | PASS | Step is encoded in URL and covered by route tests. |
| Labels default to user language | PASS | Intent and Label display name are primary; generated schema/YAML and IDs are collapsed under Advanced. |
| Automation defaults to Recipe | PASS | The default surface is a natural Recipe with Shared Stages and per-Label Pipelines; technical graph JSON is collapsed under Expert Mode. |
| Expert Graph edits the same Workflow | PASS | Existing guided lanes/graph share `label_pipeline`. |
| Draft autosaves | PASS | Existing PATCH autosave is implemented. |
| Published Version is immutable | PASS | Backend and UI prevent mutation. |

## D. Advisor

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Contextual suggestion | PASS | Registry-bound advisor consumes Project/catalog context. |
| Proposed Changes presentation | PASS | Browser coverage proves a dedicated Draft-only proposal with Recipe, comparison, rationale, bindings, warnings, alternatives, and estimates before Apply. |
| Compare | PASS | Version compare API/UI exists. |
| Apply only to Draft | PASS | Suggestions remain editable Drafts. |
| Never auto-publishes | PASS | Publication is separate. |
| Never auto-runs full Dataset | PASS | Advisor lifecycle does not start Runs. |
| Never references unknown Node/Model | PASS | Registry validation blocks unknown resources. |
| Shows cost and latency tiers | PASS | DTO exposes estimated calls/latency/cost tier. |

## E. Dry Run

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| First screen shows images and result count | PASS | Sample Test hero leads with tested images and de-duplicated business outcomes from Candidate, Classification, or Detection Artifacts. |
| First screen shows review count | PASS | Needs-attention metric and Full Run Estimate show measured and projected Review workload. |
| First screen shows failures | PASS | Node execution failures are counted separately from valid empty/no-target results. |
| First screen shows duration and cost | PASS | Dry Run DTO/UI contains both. |
| Node status is not first-level visual content | PASS | Browser proof shows all node status/timing/Artifact content closed under Diagnostics after Gallery and Uncertain Results. |
| Open uncertain result directly | PASS | The outcome-dependent primary action scrolls directly to the dedicated uncertain-result cards. |
| Success can Activate Automation | PASS | A valid Sample Test exposes one primary Activate action backed by immutable publication. |
| Dry Run writes no formal Annotation | PASS | Sandbox behavior has Rust/HTTP/E2E coverage. |

## F. Run

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Defaults to Results | PASS | Run Detail opens the persisted outcome summary without selecting or exposing a node. |
| Debug requires explicit switch | PASS | Inspector, node payloads, Provider context, and Replay render only in the explicit URL-backed Debug mode. |
| Results shows result count | PASS | Rust `RunResultSummary` aggregates formal Annotations and typed checkpoint result Artifacts without double counting. |
| Empty result says No target found | PASS | A valid empty Run result uses the explicit non-failure outcome in Results. |
| bbox shows Label and Confidence | PASS | Canvas marks display both. |
| bbox and Crop link both directions | PASS | Isolated E2E creates a real generic Project, applies the shared-detector Detect & Crop template through the UI, Dry Runs, publishes, executes Mock YOLO → Filter → Core Crop → Artifact Cache, and selects bbox/Crop in both directions. |
| Artifact Inspector lives in Run Detail | PASS | Inspector is scoped to Run. |
| User never enters a Run ID | PASS | History/deep links open Runs. |
| Replay starts at current node | PASS | Real Replay API and inspector action exist. |
| Node error includes repair information | PASS | Debug maps structured retryability to Replay failed step or Fix automation and preserves the raw recorded error. |

## G. Review

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Accept & Next | PASS | The server applies acceptance and returns the exact next persisted item plus updated progress; browser and HTTP tests pass. |
| Reject & Next | PASS | Reject requires a controlled reason, applies once, and advances using the same server-owned queue contract. |
| Review progress | PASS | Reviewed, total, remaining, and current-position facts come from persisted statuses and render without false loading zeros. |
| Explains why item needs Review | PASS | Reason and validation issue codes are shown. |
| Keyboard operation | PASS | Browser coverage exercises A, R, E, Space, and arrow navigation while form controls retain native keyboard behavior. |
| Skill-specific reason only for enabled Skill | PASS | Generic reasons are always present; registry taxonomy options appear only for Skills enabled by the selected Project. |
| Review → source Run and Node | PASS | Deep link exists. |
| Run → corresponding Review | PASS | Matching review link exists. |
| Returning preserves selection | PASS | Review ID and optional Project scope are canonical URL state; Run round-trip and reload preserve the exact item. |
| Last item guides Export | PASS | The completed Project-scoped Inbox survives reload and offers Continue to export through Project Guidance. |

## H. Export

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Shows Export Readiness | PASS | The server DTO and dedicated route show image, processed, accepted, unresolved Review, blocker, format, and output facts before execution. |
| Unresolved Review blocks or warns | PASS | Any persisted `NeedsReview` Annotation makes readiness false, provides a Project-scoped Review repair link, and makes POST export fail closed. |
| Recommends Schema-compatible format | PASS | Recommendation chooses only an enabled format whose real exporter supports every active task kind, preferring YOLO for pure detection and Native for mixed/classification schemas. |
| Shows format compatibility | PASS | Every configured format displays preflight support, purpose, warnings, and unsupported task kinds; incompatible choices cannot execute. |
| Shows Export report | PASS | The real exporter result is rendered as counts, output files, warnings, and result path; technical file detail is collapsed by default. |
| Clear completed journey state | PASS | Successful export shows an explicit terminal state and restores the persisted fingerprint-matched report after reload. |

## I. State recovery

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Refresh Project preserves Stage | PASS | Browser reload refetches the server summary and preserves the exact Guidance primary action; persisted Dry Run evidence is covered in Rust. |
| Refresh Run preserves Image | PASS | Image query state exists and E2E passes. |
| Refresh Debug preserves Node | PASS | `view=debug`, Image, Node, and Artifact are canonical URL state; legacy technical links infer Debug. |
| URL reopens same Artifact | PASS | Artifact query identity is parsed and used. |
| Browser back/forward is correct | PASS | E2E traverses Data ↔ Labels with native back/forward and restores the matching URL, content, and server-gated Build state. |
| Active Run restores from server | PASS | Summary and E2E cover server-owned state. |
| SSE reconnect resynchronizes | PASS | A failed-first SSE connection sets a resync latch; E2E mutates Review truth during the interruption and observes Export invalidate its stale success state after reconnect. |
| Start disabled during active Run | PASS | Project helper/UI and E2E cover it. |
| Backend rejects duplicate start | PASS | Transactional 409 test exists. |

## J. Product and visual hierarchy

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Guided default hides ArtifactId | PASS | Results contains no Artifact or Node IDs; technical identity is available only after entering Debug. |
| Guided default hides full DAG | PASS | Natural step cards are the default; full graph JSON and technical node editing require opening the Expert Graph. |
| One Primary Button per page | PASS | Release E2E audits every main route and all guided Project surfaces; each rendered state has at most one visible solid primary action. |
| No nested Cards | PASS | Release-wide DOM audit across twelve routes finds no `.panel .panel`; hierarchy is expressed with spacing, borders, and sections instead. |
| At most three equal first-screen metrics | PASS | Home, Project Header, Data, Sample Test, Run Results, and Export use at most three equal metrics; usage and operational context moved to secondary text. |
| Technical metadata collapsed by default | PASS | Automation hides the graph and Run Results hides the Inspector; payloads and Provider metadata require explicit Expert/Debug entry. |
| Empty workspace contains no RoboCup | PASS | E2E passes. |
| Generic Project contains no RoboCup | PASS | Rust and E2E pass. |
| AnnotAgent remains global brand | PASS | Global shell/brand audit passes. |

## K. Responsive and accessibility

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| 1024px has no horizontal overflow | PASS | Release E2E audits Home, Projects, Project, all Build steps, Runs, Run Detail, Review, Export, and Settings at 1024px. |
| 720×450-equivalent viewport is operable | PASS | Release E2E traverses Project, all Build steps, Run, Review, and Export at 720×450, checks reflow, and scrolls to a real enabled control on each surface; an independent in-app browser audit confirms Crop and annotation-list visibility. |
| Actual 200% Zoom | MANUAL | Must be manually verified if the environment permits. |
| Primary journey is keyboard-operable | PASS | Browser coverage uses Enter through Project → Build → Labels, operates the Run annotation list and Crop view, selects Export format with Space, and retains the complete Review shortcut path. |
| Review can be completed by keyboard | PASS | The decision Inbox exposes documented A/R/E/Space/arrow shortcuts and browser coverage completes its keyboard path. |
| Focus is visible | PASS | E2E focuses the Project Build control and asserts the computed focus outline is solid and at least 2px; shared `:focus-visible` styles cover native controls. |
| Status is not color-only | PASS | TUI/Web patterns and tests use text labels. |
| Canvas has equivalent annotation list | PASS | Review Canvas and Run Result Canvas expose labelled structured lists; E2E selects the Run annotation entry by keyboard and observes the same selected geometry/Crop lineage. |
| Reduced motion works | PASS | E2E emulates `prefers-reduced-motion: reduce` and asserts the Guidance transition duration is at most 0.01ms. |

## Milestone 0 evidence

- `cargo fmt --all --check`: PASS.
- `cargo test --workspace --all-features`: PASS, 159 tests.
- `npm run typecheck`: PASS.
- `npm test`: PASS, 12 files and 30 tests.
- `npm run build`: PASS.
- `npm run test:e2e`: 10 passed, 1 skipped for missing Crop fixture.
- Browser audit exercised all primary and Project Build routes against the running product.

## Milestone 1 evidence

- Added `ProjectStage`, `ProjectGuidance`, `GuidedActionKind`, `GuidedAction`, `GuidanceBlocker`, `ProjectReadinessSummary`, and `ProjectWorkspaceSummary` in the Rust Application boundary.
- The pure engine covers `NeedsData`, `NeedsLabels`, `NeedsAutomation`, `NeedsModelBinding`, `ConfigurationIssue`, `ReadyForSampleTest`, `SampleTestNeedsAttention`, `ReadyToActivate`, `ReadyToRun`, `Running`, `NeedsReview`, and `ReadyToExport` with exactly one primary action.
- Migration v5 persists complete sample-test reports per Draft; restart evidence preserves `ReadyToActivate` and publication advances to `ReadyToRun`.
- `GET /api/projects/:id/guidance`, `/readiness`, and `/summary` return consistent projections; HTTP equality assertions pass.
- `cargo test -p annotagent-application -p annotagent-storage -p annotagent-server --lib`: 20 + 8 + 9 passed.
- Strict Clippy for all targets/features of those crates: PASS.

## Milestone 2 evidence

- `PRIMARY_NAVIGATION` remains exactly Home, Projects, Runs, Review, and Settings.
- Workflow authoring remains `/projects/:id/build/pipeline`; Models and Capabilities remain Settings sections; legacy routes canonicalize.
- Run/Review route DTOs now preserve optional `project_id` query scope.
- Global Runs and Review render an explicit `All projects` filter and no longer consume the remembered Active Project.
- Detail context derives from the Run/Review record; Project-origin Review links carry an explicit query scope.
- Main route content waits for the initial dashboard response, eliminating simultaneous loading and false empty-state rendering.
- Web: 31 unit tests, production build, and 11/11 executable E2E scenarios pass; the independent Crop fixture test remains conditional.

## Milestone 3 evidence

- Replaced the YAML-first modal with four guided steps: annotation intent and Label, data source, speed/accuracy/cost constraints, and recommended Automation/model connection.
- Internal workspace/task/Label IDs are generated deterministically and are only exposed as Advanced overrides. The E2E journey deliberately creates its Project without touching those controls.
- Finishing the wizard persists provider settings, creates the real Project, imports the selected image source with duplicate/error reporting, and creates a registry-validated Advisor Draft. `Customize` opens that same Draft in Project Build.
- Priority and optional maximum cost map into real Advisor latency, accuracy, cost, and Review Gate constraints. Mock remains the deterministic offline option; external providers require a concrete model and a workspace-private credential.
- `npm run typecheck`, 31 Web unit tests, production build, and 11 executable Chromium E2E scenarios pass. One Crop-link scenario remains explicitly conditional on an available Crop Artifact fixture.
- Browser evidence: `docs/execution/screenshots/02-guided-project-wizard.png`; the same journey verifies no horizontal overflow at 720×450.

## Milestone 4 evidence

- Extended the Rust Guidance projection with eight ordered `ProjectJourneyStep` records and semantic states while retaining the deterministic precedence of Active Run, missing prerequisites, sample evidence, activation, Review, and Export.
- The server summary test proves Guidance, readiness, blocker repair, Journey detail, and Project data are returned as one coherent snapshot.
- Project Overview removed the TypeScript primary-action derivation. It displays the Project Header, one Guidance Hero action, at most two server secondary actions, server blockers, and the Journey before Recent Activity and Usage.
- Workflow selection, Schema, model bindings, Skills, import/export, Agent evidence, and image records remain functional under collapsed Advanced Project Details. Active Run/Batch controls still invoke their distinct real APIs.
- Browser coverage creates a no-data Project and verifies its server-owned repair action; restores a mocked server-owned Active Run across reload; verifies one primary action and no duplicate Run action; and checks the 720px Project journey for horizontal overflow.
- Application 20 tests, Server 9 tests, Web typecheck, 31 Web unit tests, production build, and 12 executable Chromium E2E tests pass. One Crop lineage test remains explicitly conditional on fixture availability.
- Browser evidence: `docs/execution/screenshots/03-project-guidance.png`.

## Milestone 5 evidence

- Build navigation consumes the server Journey to show Complete/Current states, disables later steps, preserves the step in the URL, and provides explicit Back/Continue actions.
- Browser coverage opens `/build/test` on a no-data Project and receives the real `Add images` blocker rather than a usable Test surface; a complete Project traverses all four steps and survives a Data-step reload.
- Data import performs bounded decode checks and reports discovered/imported/duplicate/corrupt/unsupported facts. Rust and HTTP tests prove a valid image imports, corrupt/unsupported files do not, a duplicate is skipped, path/size are returned, and the contained Project copy can be removed.
- Data UI lists thumbnails, dataset-relative paths, sizes, supported formats, import status, quality issues, and the real Remove action. There is no disabled placeholder for corrupt diagnostics.
- Labels default to `What do you want to annotate?`, purpose, Labels, and output shape. Internal IDs and raw attribute typing remain in Advanced controls.
- Automation retains real debounced Draft PATCH autosave and refreshes readiness after saving. Test & Activate maps user language onto the real Draft → validation/Dry Run → immutable publish lifecycle.
- Application 21 tests, Server 9 tests, Web typecheck, 31 Web unit tests, production build, and 13 executable Chromium E2E tests pass. One Crop lineage test remains conditional.
- Browser evidence: `docs/execution/screenshots/04-build-data.png` and `docs/execution/screenshots/05-build-labels.png`.

## Milestone 6 evidence

- Automation renders one natural-language Recipe over the persisted Label Pipeline composition. Shared Stages state that they run once per image and name their Label consumers; Label lanes describe Find, Filter, Crop, Verify, Gate, Review, and Save outcomes.
- Default node cards expose user-meaningful Model, input, output, threshold, and validation facts without node IDs. The real editable configuration is in a Node Drawer and retains debounced PATCH autosave.
- Expert Graph is closed by default and applies valid technical JSON back to the same Draft. There is no second Workflow representation and no publication action on the Automation page.
- Advisor output is presented as `Proposed Changes`, including a natural Recipe, current-Draft diff, rationale, unresolved bindings, warnings, alternatives, model-call estimate, latency, and cost tier. Apply selects the persisted editable Draft; Dismiss never activates it.
- Project-scoped filtering prevents unrelated Published Versions from appearing in the open Project's Automation history or compare controls.
- `npm run typecheck`, 31 Web unit tests, and the production build pass. Chromium E2E proves recommendation preview, compare, Apply-to-Draft, Node Drawer autosave, collapsed Expert Graph, and absence of a Publish action; 14 scenarios pass and one Crop fixture scenario is explicitly conditional.
- Browser evidence: `docs/execution/screenshots/06-automation-recipe.png`.

## Milestone 7 evidence

- Added the domain-neutral `SampleTestSummary`, `UsageSummary`, `FullRunEstimate`, `SampleTestOutcome`, and outcome status contracts. Persisted older WorkflowDryRun reports remain compatible because every new projection field has a serde default.
- Label Pipeline sample execution aggregates the latest state of repeated Candidate, Classification, and Detection Artifacts by stable item identity. Whole-image Classification is a first-class result even when the Recipe commits a ClassificationSet without an AnnotationCandidateSet.
- Every sample now exposes image identity, result/accept/review/failure/empty facts and safe result geometry. Summary estimates scale measured sample duration, exact decimal cost, and Review count to the real Project image count.
- Test & Activate uses three equal first-screen outcome metrics, a separate Full Run Estimate, real image Gallery, valid `No target found` language, a dedicated Uncertain Results section, and closed Pipeline/Usage/Timing/Artifact diagnostics.
- The primary action is outcome-dependent: Fix Automation on failures, Review Uncertain Result when human attention is predicted, otherwise Activate Automation. Activation invokes the existing immutable publish endpoint; sandbox execution still writes no formal Run or Annotation.
- Application 21, Core 28, Storage 8, and Server 9 focused tests pass. Strict Clippy for those crates passes. Web typecheck, 31 unit tests, production build, and 14 executable Chromium E2E scenarios pass; one Crop fixture test remains conditional.
- Browser E2E verifies the outcome hierarchy, one Gallery result, zero-uncertain success state, collapsed Diagnostics, exactly three equal metrics, immutable activation, and no horizontal overflow at 720×450.
- Browser evidence: `docs/execution/screenshots/02-dry-run-summary.png`.

## Milestone 8 evidence

- Added domain-neutral `RunResultSummary`, `RunResultLabelSummary`, `RunDebugSummary`, and `RunDebugIssue` Application contracts. Result projection prefers formal non-rejected Annotations and falls back to persisted Candidate, Classification, or Detection output Artifacts using stable item identity.
- Added `GET /api/runs/:runId/result-summary` and `GET /api/runs/:runId/debug-summary`. HTTP tests execute a real published Label Pipeline and assert both persisted projections.
- Run Detail defaults to Results and exposes Debug through `?view=debug`. Existing `image`, `node`, and `artifact` deep links remain compatible; technical parameters automatically select Debug and survive reload.
- Results presents Run outcome, three metrics, Label totals, Image Browser, `Original`/`Result`/`Compare`/linked `Crop` views, Review attention, and valid `No target found` copy. It does not render node or Artifact IDs.
- Debug retains real Pipeline Steps, Artifact canvas, Node Inspector, input/output/configuration, redacted Provider request context, raw error, Replay, and retry/fix actions. Results and Debug read the same immutable Run checkpoint.
- Application 21 and Server 9 focused tests pass; strict Clippy for both crates passes. Web typecheck, 31 unit tests, production build, and 14 executable Chromium E2E scenarios pass, with one fixture-dependent Crop test explicitly skipped.
- E2E proves Results is the default, Debug requires a switch, a technical deep link canonicalizes to Debug, selection survives reload, Review remains reachable, and 720×450 Results has no horizontal overflow.
- Browser evidence: `docs/execution/screenshots/07-run-results.png` and `docs/execution/screenshots/08-run-debug.png`.

## Milestone 9 evidence

- Added stable queue ordering, `ReviewQueueProgress`, adjacent Review navigation, and decision-and-next responses. Progress derives reviewed and pending counts from persisted `HumanAccepted`, `Rejected`, and `NeedsReview` statuses rather than client session counters.
- `GET /api/reviews` accepts explicit Project scope and returns queue plus progress. `GET /api/reviews/:id/next` returns previous/next/current position. Accept-and-next and reject-and-next select the next item from the pre-decision order and return the post-decision progress.
- Generic decisions no longer fail for Projects without a correction taxonomy. An explicitly supplied Skill must still be enabled; otherwise the server rejects it. Review items expose the source Skill when runtime lineage records one.
- Review renders Queue, Canvas, and Details as one Inbox. Details defaults to Why, Confidence, Source Run, Automation Version, and Source Step; Execution details is closed. Reject first opens a required controlled reason step, and Skill reasons are absent in the generic Classification browser fixture.
- Editing is explicit and retains existing geometry, undo/redo, Label, attribute, zoom, and comparison behavior. Any unsaved manual correction displays: `This correction will make similar candidates more likely to be reviewed.`
- The final decision navigates to a Project-scoped completed Inbox, remains completed after reload, and exposes Continue to export. Run → Review → Run retains the exact selected Review URL.
- Server 9 tests pass, including real reject-and-next and accept-and-next transactions over persisted annotations; strict Server Clippy passes. Web typecheck, 31 unit tests, production build, and 15 executable Chromium E2E scenarios pass in a fresh temporary workspace; one external Crop fixture scenario is explicitly skipped.
- Browser evidence: `docs/execution/screenshots/09-review-inbox.png`, `docs/execution/screenshots/10-review-reject.png`, and the responsive `docs/execution/screenshots/03-review-mobile.png`.

## Milestone 10 evidence

- Added `ExportReadiness`, `ExportBlocker`, `ExportFormatCompatibility`, and `ProjectExportResult` at the Application boundary plus `GET /api/projects/:id/export-readiness`. The POST endpoint now delegates to the same Project snapshot/readiness service instead of rebuilding a one-Run export in HTTP code.
- Snapshot assembly selects the newest terminal Run for each Project image, exports only `AutoAccepted`/`HumanAccepted` candidates (plus legacy completed Drafts), excludes rejected/pending records, retains accepted revisions, and represents processed negative images without inventing annotations.
- Real exporter compatibility drives the configured Native, COCO, YOLO Detection, YOLO Segmentation, and LabelMe choices. Pure bbox and segmentation Projects receive task-specific recommendations; classification or mixed schemas prefer lossless Native.
- Unprocessed images, unresolved Reviews, absent data, and lack of a compatible enabled format produce fail-closed blockers with exact repair destinations. A valid no-target dataset is exportable and is not confused with a failed Run.
- Export writes `export-report.json` beside output files. Its deterministic source fingerprint restores the terminal success state after reload only while images, accepted Annotations, and revisions still match the exported snapshot.
- `/projects/:id/export` is canonical URL state and part of the Project workspace tabs. The default screen exposes exactly three readiness metrics, compatibility cards, one solid export action, repair actions, result path/copy action, and a collapsed readable report; the old duplicate Advanced export controls were removed.
- Application 21/21 and Server 9/9 tests execute the real Native exporter, prove pending Review blocks POST, accept the Review, verify accepted-only counts, and reload the persisted report; strict all-target/all-feature Clippy passes for both crates. Web typecheck, 32 unit tests, production build, and Chromium E2E pass with 16 scenarios plus one conditional Crop skip.
- Browser E2E covers Review blocker → repair → ready → export → refresh and asserts no horizontal overflow at 390px. Visual evidence: `docs/execution/screenshots/11-export-complete.png`.

## Milestone 11 evidence

- Runs list `status` is canonical query state beside explicit `project_id`; refresh restores both selectors. Existing Run detail `view`, `image`, `node`, and `artifact` context remains URL-owned, and Review item/scope plus Build step remain canonical paths.
- Selecting another Project while inside Build now retains the exact Data/Labels/Automation/Test step. Selecting another Project from Export retains Export. Project Overview refetches its combined server summary when data, schema, Automation, Run, Batch, Review, or readiness dashboard facts change.
- Export readiness reloads when server dashboard counters change and still validates the persisted source fingerprint. It cannot keep showing `Dataset exported successfully` after a newly pending Review changes the exportable Project snapshot.
- SSE records errors that occur before either the first or a later successful connection. On the next open it re-fetches dashboard truth; this closes the failed-first-connection gap where the old code treated recovery as an initial connection and skipped synchronization.
- Browser E2E uses native `goBack`/`goForward`, switches Project context while retaining the Build step, persists the Runs status/Project filters over reload, aborts and restores `/api/events`, mutates a real Review during interruption, and observes Export transition from completed to blocked without manual refresh.
- Active Run restoration and duplicate Start locking remain covered by server-derived Project state; Review item selection survives Run round trips and reload; Build direct URLs remain prerequisite-gated. Web typecheck, 32 unit tests, production build, and 18 executable Chromium scenarios pass with one conditional Crop fixture skip.

## Milestone 12 evidence

- The formerly conditional Crop scenario is fully isolated and executable. Browser setup creates a generic bounding-box Project, imports a real image, requests a registry-bounded shared-detector Draft, applies **Detect & Crop** through the product UI, waits for real autosave, Dry Runs, publishes, starts a Mock Run, and inspects the persisted DetectionSet and CropSet checkpoint.
- Shared YOLO detection is correctly recognized as the detector source. The template composes `shared detector → filter → Core Crop → Artifact Cache` while the filtered DetectionSet remains the bbox commit path; YOLO itself still does not own Crop.
- Run Results de-duplicates geometrically identical pass-through Detection/Crop Artifacts without changing checkpoint history. It shows one result, one linked Crop, and one keyboard-operable structured annotation-list entry.
- Release-wide browser audits cover twelve desktop routes at 1024px and the complete Project journey at 720×450. They assert no horizontal overflow, at most one visible primary action, no nested Panels, at most three equal metric blocks, accessible controls after reflow, visible focus, keyboard activation, structured Canvas alternatives, reduced motion, real loading, and recoverable error states.
- Error recovery names the failed action, explains that saved data remains server-owned, and reloads the latest server state. E2E fails Export readiness, restores the endpoint, retries, and observes a valid ready page.
- In-app browser validation against the isolated Release workspace confirms one Project primary action, three Project numeric facts, no nested Panels, no horizontal overflow, one de-duplicated ball result, one Crop, a structured annotation list, and the same controls visible at 720×450 with zero console errors.
- `./scripts/acceptance.sh`: PASS. It includes boundary/secret scans, Rust fmt, strict workspace Clippy, 166 Rust tests, all-feature build, Web typecheck, 32 Web tests, production build, doctor, and all three offline demos.
- `npm --prefix web run test:e2e`: PASS — 22/22 executable Chromium scenarios, 0 skipped.
- Release documentation now includes the README journey plus `GUIDED_EXPERIENCE.md`, `PROJECT_GUIDANCE.md`, `GUIDED_PROJECT_SETUP.md`, `RUN_AND_REVIEW_UX.md`, `DEMO_GUIDED_EXPERIENCE.md`, updated design/hierarchy/visual integration, and honest Known Limitations.
- Automated acceptance totals are `PASS 94`, `PARTIAL 0`, `OPEN 0`; native browser 200% zoom remains the one explicit `MANUAL` item allowed by the task.
