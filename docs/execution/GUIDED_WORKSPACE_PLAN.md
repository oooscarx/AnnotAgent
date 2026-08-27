# AnnotAgent Guided Project Workspace Alpha

## Product outcome

The primary product journey is project-scoped and guided:

`Import data -> Define labels -> Configure pipeline -> Dry Run -> Publish -> Run -> Inspect -> Review -> Export`

Registry, DAG, artifact DTO, and provider details remain available as progressive disclosure, not as the default information architecture.

## Milestones

| Milestone | Deliverable | Verification |
| --- | --- | --- |
| 0 | Baseline inventory, decision log, status and acceptance files | Existing Web and Rust smoke tests |
| 1 | Five-item global IA, route migration, Settings sub-navigation | Navigation and legacy redirect tests |
| 2 | Server-backed project summary and persistent Project Workspace | DTO/API tests and project workspace tests |
| 3 | Four-step Build flow with URL state, autosave and validation | Build navigation and draft tests |
| 4 | Shared stages, label lanes, node drawer, advisor compare, versions | Pipeline UI and version immutability tests |
| 5 | Run Detail workspace with integrated inspector and deep links | Artifact, replay and URL tests |
| 6 | Review source context and bbox/crop linkage | Review/run navigation and linkage tests |
| 7 | Refresh/back-forward/SSE recovery and active-run locking | State recovery tests |
| 8 | Keyboard, focus, responsive, zoom and reduced-motion pass | Browser and accessibility checks |
| 9 | Full verification, browser screenshots, README and limitations | Full Rust/Web suites and manual tasks |

## Architectural boundaries

- Server DTOs are authoritative for project readiness, run context, review counts, versions, and artifacts.
- URLs are authoritative for durable navigation context; component state may cache but not replace it.
- One workflow definition powers both the guided pipeline view and the advanced graph view.
- Published versions are immutable. Any edit creates a draft.
- Inspector is contextual to Run Detail or Review Detail, never a standalone registry workflow.
- Generic and empty projects remain free of RoboCup-specific labels or content.

## Delivery discipline

Each milestone updates `UX_STATUS.md` and `UX_ACCEPTANCE_EVIDENCE.md`, runs its scoped tests, and lands as an independent local commit. The branch is not pushed and Git remotes are not changed.
