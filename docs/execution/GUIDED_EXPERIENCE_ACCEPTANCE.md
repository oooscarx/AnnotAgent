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
| Defaults to Results | OPEN | No Results/Debug mode contract. |
| Debug requires explicit switch | OPEN | Inspector is always part of the workspace. |
| Results shows result count | PARTIAL | Header counts exist; result summary DTO absent. |
| Empty result says No target found | OPEN | Generic empty copy is not outcome-specific. |
| bbox shows Label and Confidence | PASS | Canvas marks display both. |
| bbox and Crop link both directions | PARTIAL | Logic/unit path exists; E2E is skipped without Crop fixture. |
| Artifact Inspector lives in Run Detail | PASS | Inspector is scoped to Run. |
| User never enters a Run ID | PASS | History/deep links open Runs. |
| Replay starts at current node | PASS | Real Replay API and inspector action exist. |
| Node error includes repair information | OPEN | Structured errors exist, but guided repair action is missing. |

## G. Review

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Accept & Next | OPEN | Accept exists; next-item transaction/action does not. |
| Reject & Next | OPEN | Reject exists; next-item transaction/action does not. |
| Review progress | OPEN | Queue count exists; position/progress does not. |
| Explains why item needs Review | PASS | Reason and validation issue codes are shown. |
| Keyboard operation | PARTIAL | Editing shortcuts exist; inbox decision path needs coverage. |
| Skill-specific reason only for enabled Skill | PARTIAL | Skill reason support exists; visibility regression needed. |
| Review → source Run and Node | PASS | Deep link exists. |
| Run → corresponding Review | PASS | Matching review link exists. |
| Returning preserves selection | PARTIAL | URL identity exists; round-trip selection needs explicit test. |
| Last item guides Export | OPEN | No journey completion action. |

## H. Export

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Shows Export Readiness | OPEN | No readiness endpoint/DTO. |
| Unresolved Review blocks or warns | OPEN | Export report exists; preflight guidance missing. |
| Recommends Schema-compatible format | OPEN | Available formats exist; no recommendation. |
| Shows format compatibility | PARTIAL | Export protocol reports skips/warnings after execution, not as preflight. |
| Shows Export report | PASS | Real report is returned/rendered. |
| Clear completed journey state | OPEN | No terminal success guidance. |

## I. State recovery

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Refresh Project preserves Stage | PASS | Browser reload refetches the server summary and preserves the exact Guidance primary action; persisted Dry Run evidence is covered in Rust. |
| Refresh Run preserves Image | PASS | Image query state exists and E2E passes. |
| Refresh Debug preserves Node | PARTIAL | Node query exists; Debug mode does not. |
| URL reopens same Artifact | PASS | Artifact query identity is parsed and used. |
| Browser back/forward is correct | PARTIAL | `popstate` implemented; full journey regression remains. |
| Active Run restores from server | PASS | Summary and E2E cover server-owned state. |
| SSE reconnect resynchronizes | PARTIAL | refetch-on-open exists; interruption E2E remains. |
| Start disabled during active Run | PASS | Project helper/UI and E2E cover it. |
| Backend rejects duplicate start | PASS | Transactional 409 test exists. |

## J. Product and visual hierarchy

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| Guided default hides ArtifactId | OPEN | Run inspector exposes technical IDs in default layout. |
| Guided default hides full DAG | PASS | Natural step cards are the default; full graph JSON and technical node editing require opening the Expert Graph. |
| One Primary Button per page | PARTIAL | Project Overview renders exactly one server-selected solid action; remaining Build/Run/Review/Export surfaces are gated by later Milestones. |
| No nested Cards | OPEN | Requires component/layout audit after journey refactor. |
| At most three equal first-screen metrics | OPEN | Existing dashboards and Run surfaces exceed outcome hierarchy. |
| Technical metadata collapsed by default | PARTIAL | Some details collapse; inspector remains prominent. |
| Empty workspace contains no RoboCup | PASS | E2E passes. |
| Generic Project contains no RoboCup | PASS | Rust and E2E pass. |
| AnnotAgent remains global brand | PASS | Global shell/brand audit passes. |

## K. Responsive and accessibility

| Requirement | Status | Baseline evidence / gap |
| --- | --- | --- |
| 1024px has no horizontal overflow | PASS | Existing responsive E2E/browser evidence. |
| 720×450-equivalent viewport is operable | PARTIAL | 720-wide Review E2E exists; full journey and height need coverage. |
| Actual 200% Zoom | MANUAL | Must be manually verified if the environment permits. |
| Primary journey is keyboard-operable | PARTIAL | Baseline controls/focus exist; new journey needs end-to-end proof. |
| Review can be completed by keyboard | OPEN | Inbox decisions not implemented. |
| Focus is visible | PARTIAL | Base focus styles exist; new components need audit. |
| Status is not color-only | PASS | TUI/Web patterns and tests use text labels. |
| Canvas has equivalent annotation list | PARTIAL | Review/Run lists exist; equivalence needs explicit accessibility proof. |
| Reduced motion works | PASS | Existing CSS media query and prior browser verification. |

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
