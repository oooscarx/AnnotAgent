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
- Remaining blockers are open. Milestone evidence references tests, browser paths, screenshots, and commits rather than implementation intent.

## Milestone 1 — Global IA

- `navigation.test.ts`: five destinations, legacy migration, and project Build context pass.
- Web unit suite: 9 files and 18 tests passed.
- Production build: pass.

## Manual tasks

Tasks A–D are pending until the Run and Review workspaces are integrated.
