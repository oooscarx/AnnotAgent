# Demo onboarding — Frontend 1 integration handoff

## Fixed integration boundary

- Integration branch: `codex/demo-onboarding-integration`
- Starting commit: `8a7d8007904a00af62f3b179c216b8a3b2732a1d`
- Audit reference: `96d91b6842668c10b25d94c0a9f895165b31d1e7`
- Production adapter remains fail-closed. Fixture data is not a production fallback.
- No remote push or `main` merge is authorized for this work.

The task package is currently supplied outside Git at
`/Users/oscar/Documents/my_workspace/AnnotAgent/design/annotagent-guided-demo-r1-r6/`.
It is read-only task input and is not part of this branch.

## File ownership and component seams

Frontend 1 owns `App.tsx`, `http.ts`, `adapter.ts`, public routes and shared API types.

- Frontend 2 owns delivery/review/artifact domain files and will export independently
  injectable `DemoReviewPanel` and `DemoDeliveryPanel` components. It does not register
  them in `App.tsx` or the HTTP adapter.
- Frontend 3 owns model settings and a new independently injectable `TaskUsage`
  component. It does not edit `App.tsx`, `http.ts`, `adapter.ts`, routes or shared types.
- Backend owns all Rust, persistence, migrations, demo catalog/bootstrap, R3 request
  enforcement and R6 usage projections.
- Product owns the redistributable demo pack. P0 is fixed as
  `tabletop-cup-bottle@1.0.0`; the UI reads it from the server catalog and does not
  hard-code asset paths.

## Baseline capability audit

| Area | Baseline state | Integration action |
|---|---|---|
| Agent mainline | Real project/task read model, permission cards, stop receipts and queue controls are present | Reuse; do not script multi-step execution in React |
| Review and package | Existing delivery review and server readiness/authorization components are present | Register Frontend 2 domain seams after fixed commit |
| Context history | `TaskHistoryView` and `ContextArchiveView` already support trace inspection and JSON archive workflows | Make the three actions easy to find from the active task; do not rebuild archive semantics |
| Model setup | Existing `SetupRequest` preserves a validated same-task return route | Reuse for live Demo readiness; never fall back to preset candidates after a model error |
| Task usage | Snapshot has only a coarse legacy usage projection | Register Frontend 3 per-attempt component against Backend R6 DTO |
| Demo catalog/bootstrap | No production frontend service exists at the starting commit | Await Backend binding; UI may render only server catalog entries and server receipts |

## Coordination log

- `DEMO-BASE-001` sent to Backend for the shared HEAD, demo bootstrap, R3 and R6
  HTTP contracts.
- `DEMO-SEAM-F2-001` fixed the review/delivery component boundary.
- `DEMO-SEAM-F3-001` fixed the model-readiness and task-usage component boundary.
- Product P0 identity fixed to `tabletop-cup-bottle@1.0.0`; P1 is intentionally
  deferred until a second licensed pack actually exists.

## Verification status

No Demo path is marked passed yet. Live paid-model verification has not been run.
The production server on port 8788 and the real user workspace are outside this
integration branch's test scope.
