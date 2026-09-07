# First-Result Experience — execution ledger

## Scope and status

Started 2026-09-07. M0 complete; M1 next. This is the only ledger for this task.
Base commit: `734d1e3`. Fourteen existing acceptance PNG modifications are unrelated and preserved.
No push, remote change, reset, rebase, amend, real-workspace cleanup, or paid inference is authorized
by this implementation. Tests use isolated workspaces and explicitly labelled protocol fixtures.

## M0 baseline (code inspection, before changes)

| Persona | Actual route/actions | Breakpoints |
| --- | --- | --- |
| No model | Home → New project (four-part wizard) → Project → Data → Labels → Automation → Settings | Cannot choose browser images; import accepts a server path only. No same-task model preparation panel. |
| Ready model | Create → Data import → Labels → Automation/Ask → Test → open sample dialog → activate → start full Run → project Batch → Review → Export | First result spans four Build pages. Sample dialog is read-only, feedback is not persisted. Test/start buttons lack a complete authorization receipt. |
| Returning | Home → Project guidance → active Batch/Run, Review, Build or Export | Real combined readiness already exists. Home gives introduction and global metrics equal weight; no focused continue-last-work entry. |

Existing foundations: `App.tsx` BuildWorkspace/BuildTestPublish/SampleAnnotationDialog,
`navigation.ts` canonical object routes, `projectWorkspace.ts` and backend Guidance,
`queryCache.ts`, Registry Settings forms, AnnotationCanvas, persisted sample tests, immutable
publication, idempotent formal runs, project-scoped review revisions, export and lifecycle management.
No new ProjectStage or second Workflow/Provider/Router implementation is planned.

Confirmed missing: browser image upload; sample feedback persistence/editing; first-result
authorization and recoverable publish/start receipt. Unknown until tests: true browser-zoom behavior,
user usability, live-provider cancellation charging. No accuracy change is in scope.

## Planned commits

- M0: failing E2E baseline + interactive, explicitly offline three-stage route prototype.
- M1: images and goal → same Draft/sample → persisted feedback using existing Canvas.
- M2: scoped inference authorization, inline existing model setup, return context.
- M3: recoverable publish/start, batch/review/export continuation and management reachability.
- M4: image-first hierarchy, hints, error guidance, responsive/accessibility refinement.
- M5: regression evidence, screenshots, identity chain, limits and human study protocol.

## Verification and limitations

M0: `npm --prefix web run test:e2e -- first-result.spec.ts` failed first (missing Start with images), then passed (1/1). `npm --prefix web run typecheck` passed. E2E watches all API requests and asserts zero mutations while opening and interacting with the example.
The three-screen prototype uses the existing AnnotationCanvas and an explicitly illustrated asset from `examples/robocup`. Its persistent offline badge and disclosure distinguish authored geometry from inference. Native dialog supplies focus containment/Escape; returning users get existing Projects before collapsed global statistics. Existing canonical creation remains the prototype exit, not a completed first-result journey.

Five-person usability study has not been performed and cannot be substituted with E2E.
Real external inference is not authorized for this turn; protocol-fixture results must not be
presented as model accuracy. No before/after improvement comparison may be fabricated.

## Next

M1: browser image upload, persisted goal, and sample feedback. Creation currently automatically requests Builder; remove that unauthorised coupling before exposing the real new path. M2–M5 are not complete.
