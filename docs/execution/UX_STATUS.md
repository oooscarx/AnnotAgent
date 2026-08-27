# Guided Workspace UX Status

Updated: 2026-08-27

| Milestone | Status | Notes |
| --- | --- | --- |
| 0 Baseline | Complete | Inventory and execution documents created; baseline checks pass. |
| 1 Global IA | Complete | Five URL-driven entries; Pipeline is project-scoped; Models and Capabilities are Settings sections. |
| 2 Project Workspace | Complete | Server-backed readiness, blockers, counts, versions, and active/last run drive a persistent Project overview. |
| 3 Build Flow | Complete | URL-backed Data, Labels, Pipeline, and Test & Publish steps use real APIs and preserve server state. |
| 4 Pipeline UX | Complete | Guided shared/Label lanes, node Drawer, controlled Advisor proposal, same-definition graph, and focused versions are implemented. |
| 5 Run Workspace | Complete | `/runs/:runId` combines run controls, image context, visual Artifact preview, node timeline, detail, deep links, and Replay. |
| 6 Review Integration | Complete | Review source context, bidirectional deep links, labeled bbox/crop lineage, notes and decisions are connected. |
| 7 Reliability | Complete | URL/popstate restoration, SSE refetch/reconnect state, active Run/Batch locking, and retry recovery are implemented. |
| 8 Usability | Complete | Route focus, dialogs, keyboard selection, loading/retry, responsive breakpoints, zoom, and reduced motion pass browser checks. |
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

## Milestone 3 verification

- Data imports through the existing controlled workspace import API and reports imported/duplicate counts.
- Labels create a real `TaskConfig`; Core persists display name, generated internal ID, kind, Labels, and attributes.
- Pipeline Draft edits autosave after 800 ms and expose a saved-age indicator plus explicit discard.
- Dry Run report DTO now includes real aggregate image, DetectionSet, AnnotationCandidateSet, gate, failure, token, duration, and cost data.
- Test & Publish accepts 1–10 image indices, blocks Publish on invalid reports, and archives discarded drafts.
- Application tests: 14 passed; Server tests: 7 passed; Web: 10 files/21 tests and production build passed.

## Milestone 4 verification

- Shared stages say “runs once per image” and compute their Label use count from the same composition.
- Node cards expose binding, typed input/output, threshold and state; complete configuration is in a modal Drawer.
- Advanced graph serializes the same `label_pipeline` object used by guided lanes.
- Advisor server DTO includes per-image model calls, estimated latency, and cost tier; UI requires explicit Apply to Draft.
- Current Draft and Default Published Version are primary; Historical Drafts and Version History are collapsed.
- Core 26 tests, Application 14 tests, Web 10 files/21 tests, and production build passed.

## Milestone 5 verification

- Workflow page no longer renders an Artifact Inspector.
- Run history opens a Run directly; the user never enters or copies a Run ID.
- Run Detail loads its persisted checkpoint, project image, first node, and exact typed outputs.
- Image, node, and Artifact identities are encoded in the URL; zoom and image/crop modes operate on persisted geometry.
- Pause/Resume/Cancel and Replay call real control/runtime endpoints.
- Server 7 tests passed; Web 10 files/22 tests and production build passed.

## Milestone 6 verification

- Review API includes source Run, Workflow ID/version, source Node/Artifact, reason, confidence, and validation issue codes.
- Review Detail opens exact Run/Node/Artifact context; Run Detail exposes the matching Review item.
- Detection overlays render label, confidence, color legend, and non-color text cues.
- Detection and Crop marks join by stable `parent.item_id`; either click updates one selection and arrow keys cycle it.
- Crop cards expose parent Artifact and source Node and support enlarged preview.
- Server 7 tests passed; Web 10 files/23 tests and production build passed.

## Milestone 7 verification

- Route parsing initializes from location, `popstate` restores it, and canonicalization runs after each route change.
- SSE `error` marks reconnecting and refetches server state; `open` marks connected and refetches again.
- Active Run and active Dataset Batch are restored exclusively from Project Summary and both lock Start.
- Run, Review, Project, and Build selection survive refresh through path/query state.
- Errors keep prior data visible and provide Retry plus Dismiss.
- Status types are separated into Project Readiness, Workflow, Run, and Review unions.
- Server 7 tests passed; Web 10 files/24 tests and production build passed.

## Milestone 8 verification

- Route changes focus the H1; skip link and named global/Build navigations remain available.
- Node Drawer and Create Project dialog focus safely, expose dialog semantics, and close with Escape.
- bbox/crop results cycle with arrow keys; Review retains undo/redo keyboard support.
- First load and SSE state use live status text; request errors expose Retry and Dismiss.
- In-app browser at 1024×768: `scrollWidth === innerWidth === 1024`.
- In-app browser at 720×700 (desktop 200% zoom equivalent): `scrollWidth === innerWidth === 720`; sidebar becomes static top navigation.
- Web 10 files/24 tests and production build passed.
