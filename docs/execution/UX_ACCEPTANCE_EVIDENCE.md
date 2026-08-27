# Guided Workspace Acceptance Evidence

Updated: 2026-08-27

Evidence is added only after the relevant behavior has been implemented and exercised.

## Milestone 0 — Baseline

- Repository state and remote recorded in `UX_STATUS.md`.
- Existing route, DTO, API, and test inventory recorded in the plan and status files.
- `cargo fmt --all --check`: pass.
- `cargo test -p annotagent-server --lib`: 7 passed.
- `npm test -- --run`: 8 files and 15 tests passed.
- `npm run typecheck`: pass.

## Release blockers

- 1 — PASS: `PRIMARY_NAVIGATION` contains exactly five destinations; covered by `productIdentity.test.ts`.
- 2 — PASS: Workflow authoring renders only at `/projects/:projectId/build/pipeline`; legacy `/workflows` migrates.
- 3 — PASS: Models and Capabilities render within Settings routes; legacy registry URLs migrate.
- 4 — PASS: `/projects/:projectId` maintains project-scoped header and Overview/Build/Runs/Review/Export navigation.
- 5 — PASS: `deriveProjectNextAction` is covered for images, labels, invalid Pipeline, active Run, review, and ready states.
- 18 — PASS at API/logic level: Project Summary restores persisted `active_run`/active batch; browser verification remains in Milestone 7.
- 19 — PASS at logic level: active server work produces `Open active run` and existing `deriveProjectRunView` locks Start; browser verification remains.
- 6 — PASS: Build routes and shared navigation implement Data → Labels → Pipeline → Test & Publish.
- 9 — PASS: Test & Publish first renders eight real result metrics before diagnostics and node traces.
- Remaining blockers are open. Milestone evidence references tests, browser paths, screenshots, and commits rather than implementation intent.

## Milestone 1 — Global IA

- `navigation.test.ts`: five destinations, legacy migration, and project Build context pass.
- Web unit suite: 9 files and 18 tests passed.
- Production build: pass.

## Milestone 2 — Project Workspace

- Rust DTO and persistence tests: application 14 passed; server 7 passed.
- `projectWorkspace.test.ts`: ordered primary actions and server-owned active Run precedence pass.
- Web suite: 10 files and 21 tests passed; production build passed.

## Milestone 3 — Build Flow

- `POST /api/projects/:id/schema/tasks` stores generated IDs, display names, kinds, Labels, and attributes; application and HTTP assertions pass.
- Dry Run HTTP assertion verifies `summary.image_count`; label pipeline runtime derives Detection/Candidate/gate/usage totals from real trace artifacts.
- Draft autosave uses the existing PATCH endpoint; Test, Publish, and Discard use existing persisted APIs.
- Rust: application 14 and server 7 tests passed. Web: 10 files/21 tests and production build passed.

## Manual tasks

Tasks A–D are pending until the Run and Review workspaces are integrated.
