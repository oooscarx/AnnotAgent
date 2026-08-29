# Guided Experience Alpha Status

Updated: 2026-08-30

## Current state

- Active Milestone: 4 — Guidance-led Project Journey
- Last completed Milestone: 3 — Guided Project Creation
- Latest Milestone commit: this document's containing Milestone 3 commit
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
| 4 Project Journey | In progress | Existing overview has real state but lacks Guidance Hero/timeline/one backend action. |
| 5 Guided Build | Pending | Four persistent routes exist; language and server-guided completion remain. |
| 6 Recipe + Advisor | Pending | Registry-bounded Advisor and label lanes exist; guided recipe/proposal hierarchy needs completion. |
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
| `npm run test:e2e` | PASS — 11 passed, 1 explicitly skipped because the isolated fixture has no Crop Artifact |

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

## Latest browser audit

The running product at `http://127.0.0.1:8787` was opened at Home, Projects, Project Overview, all four Build steps, Runs, Review, and Settings.

- Five global destinations are visible on every audited route.
- Project-scoped Build routes retain the Project URL.
- Settings contains Models and Capabilities.
- Existing backend data and Artifact/Review paths render.
- Initial route content waits for dashboard state; the false zero-Project loading flash is no longer present.
- Guided Project creation presents annotation intent, data source, priority, and a recommended Automation before exposing generated YAML or internal IDs.
- Project Overview exposes several equally prominent actions.
- Run and Review default surfaces remain technically dense.

## Release Blocking remaining

- `PASS`: 53
- `PARTIAL`: 20
- `OPEN`: 21
- `MANUAL`: 1 (actual browser 200% zoom, only if the environment permits)

Counts are recalculated from `GUIDED_EXPERIENCE_ACCEPTANCE.md` after each Milestone.
