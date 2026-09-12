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
- Product owns the redistributable demo pack. P0 is fixed by the Backend catalog as
  `object-detection-review@1.0.0`; the UI reads it from the server catalog and does not
  hard-code asset paths.

## Baseline capability audit

| Area | Baseline state | Integration action |
|---|---|---|
| Agent mainline | Real project/task read model, permission cards, stop receipts and queue controls are present | Reuse; do not script multi-step execution in React |
| Review and package | Existing delivery review and server readiness/authorization components are present | Frontend 2 review/delivery seams are integrated; production registration waits for the Backend task-owned Demo projection |
| Context history | `TaskHistoryView` and `ContextArchiveView` already support trace inspection and JSON archive workflows | Make the three actions easy to find from the active task; do not rebuild archive semantics |
| Model setup | Existing `SetupRequest` preserves a validated same-task return route | Reuse for live Demo readiness; never fall back to preset candidates after a model error |
| Task usage | Snapshot has only a coarse legacy usage projection | Per-attempt TaskUsage is mounted and the HTTP reader follows the Backend numeric attempt cursor; isolated HTTP verification waits for the R6 implementation commit |
| Demo catalog/bootstrap | No production frontend service exists at the starting commit | Await Backend binding; UI may render only server catalog entries and server receipts |

## Coordination log

- `DEMO-BASE-001` sent to Backend for the shared HEAD, demo bootstrap, R3 and R6
  HTTP contracts.
- `DEMO-SEAM-F2-001` fixed the review/delivery component boundary.
- `DEMO-SEAM-F3-001` fixed the model-readiness and task-usage component boundary.
- Product P0 identity fixed to `object-detection-review@1.0.0`; P1 is intentionally
  deferred until a second licensed pack actually exists.
- Backend contract commit `0c6ceb6` was integrated as `1794b7e`; exact Rust endpoints
  remain delivery contracts until the Backend implementation commit is verified.
- Frontend 2 component commit `551a942` was integrated as `72f8630`. Its older
  formal-selection type conflicted with the current canonical `FormalVisualSelection`;
  the integration keeps the current selection type and adds only the Demo origin map.
- Frontend 3 TaskUsage commit `9cf06ed` was integrated as `a774bf2`. The component is
  mounted only when a real adapter service is registered.
- Frontend 3 effective-request and final R6 DTO commits `decd93a` and `c6a5e11` were
  integrated as `433be0a` and `8b221b9`. `f5dfa10` aligns the HTTP reader and tests
  with the numeric attempt sequence cursor; `0666877` exposes the passive R3
  effective-request read without sending a probe.
- Product pack commit `db04336` was integrated as `4ab5f07`. The only admitted P0
  identity is `object-detection-review@1.0.0`; it contains six CC0 synthetic source
  images and separately namespaced preset candidates. `06f95f5` aligns the UI copy
  and test fixture with that identity.
- Demo command recovery is scoped to the server workspace identity in `ea5c1d9`.
  Reload performs receipt lookup for the same command; it does not issue a new start.

## Verification status

Typecheck and focused onboarding/R3/R6 tests pass at `0666877`; a full Web regression
will be rerun after the Backend commit is integrated. No end-to-end Demo path is
marked passed yet because the production catalog/bootstrap and R6 Rust implementation
commit has not been delivered. Live paid-model verification has not been run.
The production server on port 8788 and the real user workspace are outside this
integration branch's test scope.
