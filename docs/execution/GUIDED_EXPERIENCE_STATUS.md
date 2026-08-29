# Guided Experience Alpha Status

Updated: 2026-08-30

## Current state

- Active Milestone: 10 — Guided Export
- Last completed Milestone: 9 — Inbox Review
- Latest Milestone commit: this document's containing Milestone 9 commit
- Push policy: local commits only; no push
- Remote policy: unchanged
- Live external models: not required for the offline Release gate and not used in this task

## Milestone ledger

| Milestone | Status | Evidence summary |
| --- | --- | --- |
| 0 Baseline | Complete | Repository, routes, APIs/DTOs, browser behavior, tests, and Release Matrix verified against code. |
| 1 Guidance Domain | Complete | Rust derives one action from persisted data, Automation, sample-test, model, Run, and Review state; three HTTP projections pass. |
| 2 Global IA | Complete | Five task entries, canonical legacy routes, Project-scoped Workflow, Settings registries, and explicit global filters pass. |
| 3 Project Creation | Complete | Four-step wizard creates a real generic Project, imports real data, persists a model choice, and creates a registry-bounded Draft without requiring internal IDs. |
| 4 Project Journey | Complete | Project Overview consumes one server summary containing Guidance, an eight-step Journey, readiness, blockers, repairs, Active Run, and at most two secondary actions. |
| 5 Guided Build | Complete | Four URL-restorable steps share server Journey completion, forward/back navigation, prerequisite gates, real data diagnostics, user-language Labels, autosaved Automation, and Test/Activate vocabulary. |
| 6 Recipe + Advisor | Complete | Natural Recipe, Shared Stages, per-Label Pipelines, Node Drawer, same-Draft Expert Graph, and a compare/apply Advisor proposal are real and never auto-publish. |
| 7 Dry Run | Complete | Rust SampleTestSummary now reports outcomes, empty/failure counts, usage, Review workload, and Full Run estimates; the UI leads with Gallery and keeps diagnostics collapsed. |
| 8 Run Workspace | Complete | Results is the default outcome workspace; Debug is an explicit URL-backed mode with Inspector, deep links, Replay, provider context, and repair actions. |
| 9 Inbox Review | Complete | Server-owned progress and next-item decisions drive a keyboard-operable Inbox with controlled reasons, correction impact, deep links, and terminal guidance. |
| 10 Guided Export | In progress | Export is real; readiness, recommendation, compatibility, and completion UX remain. |
| 11 Reliability | Pending | URL/SSE/server recovery foundations exist; new guided state needs end-to-end recovery coverage. |
| 12 Release | Pending | Full matrix, documentation, responsiveness, accessibility, and E2E expansion remain. |

## Latest automated tests

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo test --workspace --all-features` | PASS — 159 tests, 0 failures |
| `npm run typecheck` | PASS |
| `npm test` | PASS — 12 files, 31 tests |
| `npm run build` | PASS — production bundle built |
| `npm run test:e2e` | PASS — 14 passed, 1 explicitly skipped because the isolated fixture has no Crop Artifact |

Full Clippy and all-feature build are scheduled for the Release Milestone and may also run at risky intermediate boundaries.

Milestone 1 focused checks:

- Application: 20 passed.
- Storage: 8 passed, including migration v5 and sample-test round trip.
- Server: 9 passed, including Guidance/Readiness/Summary HTTP parity.
- Strict Clippy for Application, Storage, and Server: PASS.

Milestone 2 focused checks:

- Web typecheck: PASS.
- Web unit suite: 12 files, 31 tests PASS.
- Web production build: PASS.
- Chromium E2E: 11 passed; 1 Crop-data conditional test skipped.
- Browser journey proves `/runs` and `/review` ignore hidden local Active Project state and apply Project scope only from the URL.

Milestone 3 focused checks:

- The browser creates a Classification Project without opening Advanced IDs; the generated workspace/task/Label IDs remain stable and the Label display name remains user-facing.
- The same journey invokes the real Settings, Project creation, image import, and constrained Advisor endpoints, then opens the persisted Project.
- Mock is the offline path; external presets require both a concrete model ID and a workspace-private credential before creation. Custom-model placeholders cannot be persisted as model IDs.
- Chromium at 720×450 has no horizontal overflow and keeps the scrollable wizard actions operable.

Milestone 4 focused checks:

- Rust Guidance emits eight ordered Journey steps, one current/attention/ready step, exactly one primary action, and at most two secondary actions for every covered state.
- Project Overview reads `/api/projects/:id/summary`; React no longer uses its former parallel `deriveProjectNextAction` decision helper.
- Project header always exposes images, Labels, active Automation, Active Run, Review count, and readiness. The Guidance Hero precedes activity and technical details.
- A no-data Project shows the server blocker and `Add images` repair path; an Active Run restores as `Open active run` across reload without exposing a second Start action.
- Focused Rust suites: Application 20 and Server 9 passed. Web typecheck, 31 unit tests, production build, and 12 executable Chromium E2E scenarios passed.

Milestone 5 focused checks:

- Build navigation reads the server Journey, marks completed steps, exposes Back/Continue, and renders a blocking repair surface when a direct URL skips a real prerequisite.
- Image import now returns discovered, imported, duplicate, corrupt, unsupported, source, and format facts after bounded decode validation. Project copies can be listed with path/size and removed through a contained API.
- Labels display annotation purpose, user-facing Labels, and output shape by default; internal task IDs and attribute types stay under Advanced.
- Automation Draft autosave remains the real PATCH path and now refreshes server Build readiness. Test actions use `Test samples` and `Activate automation` while invoking the existing sandbox Dry Run and immutable publication APIs.
- Focused Rust suites: Application 21 and Server 9 passed. Web typecheck, 31 unit tests, production build, and 13 executable Chromium E2E scenarios passed.

Milestone 6 focused checks:

- Automation defaults to a natural Recipe: Shared Stages explicitly explain one execution per image, each Label owns a readable Pipeline, and step cards summarize Model, input, output, threshold, and validation without exposing IDs.
- Node configuration lives in a Drawer; the advanced technical graph edits the same autosaved Draft and is collapsed by default.
- The registry-bounded Advisor opens a Draft-only `Proposed Changes` preview with recipe, comparison, rationale, unresolved bindings, warnings, alternatives, estimated calls, latency, and cost. `Apply to Draft` never publishes or starts a Run.
- Published history and version comparison are scoped to the open Project, preventing unrelated Project versions from expanding the Automation workspace.
- Web typecheck, 31 unit tests, production build, and 14 executable Chromium E2E scenarios passed. One Crop lineage test remains conditional.

Milestone 7 focused checks:

- The Rust Sample Test contract aggregates Candidate, Classification, and Detection outputs into per-image business outcomes, de-duplicates repeated downstream Artifact states, records valid empty results, and keeps node failures distinct.
- SampleTestSummary includes duration, nested token/cost usage, and a Dataset-size Full Run projection with estimated duration, cost, and Review range. Existing persisted reports remain readable through serde defaults.
- Test & Activate leads with three outcome metrics, then Full Run Estimate, Results Gallery, Uncertain Results, and four closed diagnostics sections. Successful tests activate the tested Draft as an immutable Version; Dry Run creates no Run or Annotation.
- Application 21, Core 28, Storage 8, and Server 9 focused tests passed; strict focused Clippy passed. Web typecheck, 31 unit tests, production build, and 14 executable Chromium E2E scenarios passed. One Crop lineage test remains conditional.
- The complete Test & Activate workspace has no horizontal overflow at 720×450 and keeps its primary activation action operable.

Milestone 8 focused checks:

- Rust `RunResultSummary` derives image, result, ready, Review, no-target, failure, Label, duration, and usage facts from persisted Run history, Annotations, and typed checkpoint Artifacts; `RunDebugSummary` separately projects execution state and retryable issues.
- Run Detail defaults to Results. Debug requires the explicit URL-backed switch and is also inferred for legacy node/Artifact deep links, so reload preserves the selected Image, Node, and Artifact.
- Results contains exactly three first-level outcome metrics, an Image Browser, Label summary, Original/Result/Compare/Crop views, a truthful `No target found` state, and exact Review/Debug repair destinations. Technical IDs and payloads remain in Debug.
- Debug retains the real Pipeline Steps, Artifact lineage, node input/output/configuration, redacted Provider context, raw error, Replay, and failure repair actions without duplicating the runtime definition.
- Application 21 and Server 9 focused tests pass; strict focused Clippy passes. Web typecheck, 31 unit tests, production build, and 14 executable Chromium E2E scenarios pass. One Crop lineage test remains conditional on fixture availability.
- Browser evidence covers Results and Debug at desktop size and verifies the Results workspace has no horizontal overflow at 720×450.

Milestone 9 focused checks:

- `GET /api/reviews/:id/next`, `POST /accept-and-next`, and `POST /reject-and-next` return server-owned queue progress and the exact next persisted Review item; the existing decision endpoint remains compatible.
- Review defaults to one decision Inbox: reviewed/total/remaining progress, previous/next navigation, Original/Result toggle, one Accept primary action, and a reason-gated Reject path. Loading never presents false zero counts.
- Details defaults to Why, Confidence, Source Run, Automation Version, and Source Step. Execution events, validation evidence, revision history, and technical metadata remain closed under Execution details.
- Generic reject reasons are always available. Skill-specific reasons are added only from enabled Skill registries and correction evidence remains scoped to a real enabled Skill when one exists.
- `A`, `R`, `E`, `Space`, and arrow-key paths are browser-covered without stealing keys from form controls. Manual edits show their correction impact before a decision.
- Server 9 tests and strict Server Clippy pass. Web typecheck, 31 unit tests, production build, and 15 executable Chromium E2E scenarios pass in a fresh temporary workspace; one Crop lineage scenario remains conditional.
- Desktop and 390px browser evidence show no horizontal overflow, and the last decision restores a Project-scoped completed Inbox with a Continue to export action after reload.

## Latest browser audit

The running product at `http://127.0.0.1:8787` was opened at Home, Projects, Project Overview, all four Build steps, Runs, Review, and Settings.

- Five global destinations are visible on every audited route.
- Project-scoped Build routes retain the Project URL.
- Settings contains Models and Capabilities.
- Existing backend data and Artifact/Review paths render.
- Initial route content waits for dashboard state; the false zero-Project loading flash is no longer present.
- Guided Project creation presents annotation intent, data source, priority, and a recommended Automation before exposing generated YAML or internal IDs.
- Project Overview now presents one server-selected next action, Journey progress, and blocker repairs before Recent Activity and Usage; schema, bindings, versions, import/export, and image records are collapsed under Advanced Project Details.
- Build now behaves as one gated sequence rather than four unrelated management screens; URL refresh preserves the step and server prerequisites prevent manual URL bypass.
- Sample Test now leads with outcomes and Full Run impact; node statuses, timings, usage, and Artifact types are collapsed diagnostics.
- Run defaults to outcome-first Results and Review is now a fast decision Inbox. Export remains the next management-heavy surface to convert into the journey endpoint.

## Release Blocking remaining

- `PASS`: 79
- `PARTIAL`: 10
- `OPEN`: 5
- `MANUAL`: 1 (actual browser 200% zoom, only if the environment permits)

Counts are recalculated from `GUIDED_EXPERIENCE_ACCEPTANCE.md` after each Milestone.
