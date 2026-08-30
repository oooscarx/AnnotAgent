# Guided Experience Alpha Master Plan

Updated: 2026-08-30

## Objective

Turn AnnotAgent's already-real Workflow, Run, Artifact, Review, and Export capabilities into one guided journey:

```text
Create Project
→ add data
→ define Labels
→ choose Automation
→ test samples
→ activate an immutable Workflow Version
→ run the Dataset
→ review uncertain results
→ export a compatible dataset
```

Guided Mode is the default presentation. Expert Mode remains available for graph editing, node inspection, Artifact lineage, Replay, and runtime diagnostics. Both modes edit or inspect the same persisted Project, Workflow, Run, Artifact, Annotation, and Review records.

## Non-negotiable architecture

- Rust Application owns the deterministic Guidance Engine and DTOs.
- Storage, Runtime, Provider, Agent, Skill, and HTTP protocol boundaries remain intact.
- React renders server truth; it does not infer the Project journey from a parallel client-only state machine.
- Published Workflow Versions remain immutable and every Run remains pinned to its executed version.
- Core remains domain-neutral. RoboCup remains an optional Skill/demo, never the product identity.
- Global navigation remains Home, Projects, Runs, Review, and Settings.
- Secrets, API keys, image payloads, and hidden reasoning are not persisted in product telemetry or committed.
- No push and no remote modification are part of this execution.

## Verified baseline

### Git

- Branch: `main`
- Starting HEAD: `3e06c91 fix(run): space image browser controls`
- Starting worktree: clean
- Remote: `origin git@github.com:oooscarx/AnnotAgent.git`
- Starting divergence: 12 local commits ahead, 0 behind

### Current routes

| Route | Current responsibility | Guided Experience target |
| --- | --- | --- |
| `/` | aggregate workspace dashboard | journey-oriented Home and resumable work |
| `/projects` | Project inventory and creation dialog | inventory plus guided creation wizard |
| `/projects/:id` | Project overview, actions, readiness | Guidance Hero, Journey Timeline, one primary action |
| `/projects/:id/build/data` | image import | guided Data step |
| `/projects/:id/build/labels` | task/Label creation | user-language Label step |
| `/projects/:id/build/pipeline` | Workflow Designer | Automation Recipe by default, Expert graph on demand |
| `/projects/:id/build/test` | Dry Run and publish | outcome-first Test & Activate |
| `/runs` | global Run history | unfiltered global Run history |
| `/runs/:id?image=&node=&artifact=` | combined result/inspector workspace | Results by default, Debug on demand |
| `/review` and `/review/:id` | queue plus editor | inbox workflow with next-item actions |
| `/settings`, `/settings/models`, `/settings/capabilities` | provider/models/Skills | retain under Settings |

Legacy `/dashboard`, `/workflows`, `/models`, and `/skills` already canonicalize into the routes above.

### Current API and DTO inventory

The server already exposes real APIs for Projects, schemas, Skills, images/import, Workflow catalog/drafts/advice/dry-run/publish/clone/compare, Runs/Batches/control/events, Artifacts/Replay, Annotations/Revisions, Review decisions, Export/import, Models, Settings, and SSE.

Existing DTOs include `ProjectSummary`, three-state `ProjectReadiness`, `ProjectBlockingIssue`, Workflow summaries/versions, Run history/checkpoints, typed pipeline Artifacts, dry-run summaries, Review items, and Export reports. Missing baseline contracts are `ProjectStage`, `ProjectGuidance`, `GuidedAction`, `GuidanceBlocker`, result-first Run summary, next-item Review actions, and Export readiness.

### Current user-path findings

- The five-entry navigation, project-scoped Build URLs, immutable versions, real Dry Run, Artifact inspection, Replay, Review editing, and Export are functional.
- `deriveProjectNextAction` currently runs in TypeScript, so the backend does not yet own journey truth.
- Project Overview offers Review, Start, and version selection as competing first-level actions instead of one server-selected action.
- Project loading briefly renders zero counts and `No project opened` before server state arrives.
- Automation uses technical Workflow terminology before a new user sees a recommended recipe.
- Dry Run has real metrics, but it starts from draft selection rather than an outcome-first result workspace.
- Run Detail is one dense inspector; there is no explicit Results/Debug presentation contract.
- Review supports decisions and editing but not the required inbox `Accept & Next`/`Reject & Next` flow.
- Export can execute but does not yet expose a server-owned readiness/recommendation summary.

## Milestones

| Milestone | Outcome | Status |
| --- | --- | --- |
| 0 | verified baseline and acceptance ledger | complete |
| 1 | deterministic Rust Guidance domain, engine, and API | complete |
| 2 | task-centered global information architecture | complete |
| 3 | guided Project creation | complete |
| 4 | Guidance-led Project Journey workspace | complete |
| 5 | guided four-step Build | complete |
| 6 | Automation Recipe and controlled Advisor proposal | complete |
| 7 | outcome-first sample testing and activation | complete |
| 8 | Results-first Run workspace with optional Debug | complete |
| 9 | inbox Review | complete |
| 10 | guided Export endpoint and completion state | complete |
| 11 | durable URL, SSE, and server-state recovery | complete |
| 12 | responsive, accessible, documented Release gate | complete |

## Verification policy

Each Milestone updates the status and acceptance ledger, runs proportionate Rust/Web/browser tests, fixes regressions, and creates the exact independent local commit specified by the master prompt. A matrix item becomes `PASS` only when linked to code plus executable or browser evidence. Existing green tests are a baseline, not proof of new Guided Experience behavior.
