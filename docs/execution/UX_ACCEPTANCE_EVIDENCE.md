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
- 7 — PASS: default Pipeline view renders Shared Stages and independent Label Pipeline lanes.
- 8 — PASS: each Shared Stage labels run frequency and Label reuse count.
- 20 — PASS: Published Versions render read-only; editing is exposed only through Clone Version to Draft.
- 21 — PASS: Historical Drafts and Version History are closed `<details>` by default.
- 10 — PASS: Artifact inspection is rendered only inside Run Detail (and later Review context), not Workflow.
- 11 — PASS: opening a Run loads its checkpoint; there is no manual Run-ID field.
- 16 — PASS at route level: image/node/artifact query parsing has unit coverage; browser refresh verification remains Milestone 7.
- 12 — PASS: SVG bbox groups visibly render Label and confidence text.
- 13 — PASS: `artifactDetectionMarks` and `artifactCropMarks` join on parent item identity; UI shares selection both directions.
- 14 — PASS: Review `Open run context` includes source node and Artifact.
- 15 — PASS: Run Detail fetches matching queue item and exposes `Open review item`.
- 17 — PASS at implementation/unit level: `popstate` reparses full location; browser exercise remains Milestone 9.
- 18 — PASS: Project state is refetched on initial load, SSE transitions, and SSE reconnect.
- 19 — PASS: `deriveProjectRunView` locks Start for both active Run and active Batch; 2 tests pass.
- 24 — PASS: distinct TypeScript unions and DTO fields represent Project, Workflow, Run, and Review state.
- 25 — PASS: browser-measured 1024px document width has no horizontal overflow.
- 26 — PASS: browser-measured 720px effective viewport (200% desktop zoom equivalent) has no horizontal overflow and keeps guided controls available.
- 27 — PASS: route focus, skip link, native controls, dialog Escape, bbox arrows, and Review shortcuts cover the primary keyboard path.
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

## Milestone 4 — Pipeline UX

- `WorkflowSuggestion` carries and serializes real controlled-Draft estimates; application assertions pass.
- Advisor UI presents compare delta, rationale, bindings, estimates, warnings, unresolved items, and alternatives before Apply.
- Core: 26 tests passed. Application: 14 tests passed. Web: 10 files/21 tests and production build passed.

## Milestone 5 — Run Workspace

- `navigation.test.ts` restores `/runs/:runId?image=&node=&artifact=` context.
- Run Detail exposes real status/version/duration/usage/cost, image browser, overlay/crop modes, zoom, node timeline, error, input/output/config, model usage, controls, and Replay.
- Server artifact/Replay suite: 7 tests passed. Web: 10 files/22 tests and production build passed.

## Milestone 6 — Review Integration

- HTTP Review assertions cover source Run, Workflow Version, reason, and validation issue array.
- `labelPipelineUi.test.ts` verifies label/confidence/source and Crop parent/source linkage from real DTO shapes.
- Review edits, reviewer note, reason, accept/reject/delete continue using persisted revision and decision APIs.
- Server: 7 tests passed. Web: 10 files/23 tests and production build passed.

## Milestone 7 — Reliability

- `navigation.test.ts` covers durable build and Run/Artifact links; browser history is handled through `popstate`.
- `runState.test.ts` verifies backend Run and Batch restoration and duplicate-Start lock.
- SSE lifecycle refetches authoritative `/api/projects` state on reconnect and open.
- Server: 7 tests passed. Web: 10 files/24 tests and production build passed.

## Milestone 8 — Usability and accessibility

- Browser at 1024×768: five named navigation links, focused Home H1, no document overflow.
- Browser at 720×700: static top navigation, no document overflow, Project → Build → Data remains operable.
- Build DOM exposes four named step buttons and disabled corrupt-image reporting honestly.
- Web: 10 files/24 tests and production build passed.

## Manual tasks

Tasks A–D are pending until the Run and Review workspaces are integrated.
