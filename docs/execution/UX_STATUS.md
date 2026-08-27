# Guided Workspace UX Status

Updated: 2026-08-27

| Milestone | Status | Notes |
| --- | --- | --- |
| 0 Baseline | Complete | Inventory and execution documents created; baseline checks pass. |
| 1 Global IA | Complete | Five URL-driven entries; Pipeline is project-scoped; Models and Capabilities are Settings sections. |
| 2 Project Workspace | Complete | Server-backed readiness, blockers, counts, versions, and active/last run drive a persistent Project overview. |
| 3 Build Flow | Pending | Existing schema and workflow editors are separate surfaces. |
| 4 Pipeline UX | Pending | Label pipeline UI exists; hierarchy and version focus need restructuring. |
| 5 Run Workspace | Pending | Artifact Inspector is currently under Workflows. |
| 6 Review Integration | Pending | Review does not preserve bidirectional run/node context. |
| 7 Reliability | Pending | URL and active-run restoration are incomplete. |
| 8 Usability | Pending | Existing accessibility primitives need a full guided-workspace pass. |
| 9 Acceptance | Pending | Full release checks and manual tasks remain. |

## Existing capability inventory

- Rust HTTP server exposes project, image, workflow draft/version, run, batch, artifact, replay, review, model, skill, settings, import, and export endpoints.
- Web UI exposes the same capabilities, but navigation is a local page enum rather than path-driven routing.
- Project summaries already expose images, schemas, skills, workflows, active/last run, and active batch data.
- Published pipeline execution, artifact persistence, replay, review decisions, SSE, and active-run conflict protection have backend coverage.
- The VLM football detect-and-crop demo is a project fixture, not a valid generic empty state.

## Current branch state

- Branch: `main`
- Starting HEAD: `7b9e422 fix(web): improve artifact bbox contrast`
- Remote at start: unchanged; `origin/main` at `641b21d`
- Starting worktree: clean; local branch ahead by one commit
- Push policy: no push

## Baseline verification

- `cargo fmt --all --check`: pass
- `cargo test -p annotagent-server --lib`: 7 passed
- `npm test -- --run`: 8 files, 15 tests passed
- `npm run typecheck`: pass

## Milestone 1 verification

- Primary navigation: Home, Projects, Runs, Review, Settings.
- Legacy `/dashboard`, `/workflows`, `/models`, and `/skills` resolve to canonical guided routes.
- `npm test -- --run`: 9 files, 18 tests passed.
- `npm run build`: pass.

## Milestone 2 verification

- Application summary exposes `readiness`, `blocking_issues`, `task_count`, `review_count`, and `default_workflow_version` alongside persisted active/last run state.
- Project header exposes dataset, default Pipeline, active run, review count, readiness, and a derived next action.
- Project list uses Project Readiness instead of leaking Run status.
- `cargo test -p annotagent-application --lib`: 14 passed.
- `cargo test -p annotagent-server --lib`: 7 passed.
- Web suite: 10 files, 21 tests passed; production build passed.
