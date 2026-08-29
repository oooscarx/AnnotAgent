# Guided Experience Alpha Status

Updated: 2026-08-30

## Current state

- Active Milestone: 6 — Automation Recipe and Advisor
- Last completed Milestone: 5 — Guided Build
- Latest Milestone commit: this document's containing Milestone 5 commit
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
| 6 Recipe + Advisor | In progress | Registry-bounded Advisor and label lanes exist; guided recipe/proposal hierarchy needs completion. |
| 7 Dry Run | Pending | Sandbox execution is real; result-first summary/activation flow remains. |
| 8 Run Workspace | Pending | Artifact/Replay context is real; Results/Debug modes and repair guidance remain. |
| 9 Inbox Review | Pending | Geometry editing/decisions are real; next-item actions/progress remain. |
| 10 Guided Export | Pending | Export is real; readiness, recommendation, compatibility, and completion UX remain. |
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
| `npm run test:e2e` | PASS — 13 passed, 1 explicitly skipped because the isolated fixture has no Crop Artifact |

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
- Project Overview exposes several equally prominent actions.
- Run and Review default surfaces remain technically dense.

## Release Blocking remaining

- `PASS`: 54
- `PARTIAL`: 21
- `OPEN`: 19
- `MANUAL`: 1 (actual browser 200% zoom, only if the environment permits)

Counts are recalculated from `GUIDED_EXPERIENCE_ACCEPTANCE.md` after each Milestone.
