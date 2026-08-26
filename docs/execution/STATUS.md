# Workflow Alpha Status

Last updated: 2026-08-27 05:12 CST

## Current milestone

Milestone 3 — generic published-snapshot DAG Runtime.

## Completed

- Verified branch `main` is clean and six commits ahead of `origin/main` before this milestone.
- Recorded the latest twelve local commits without rewriting history.
- Ran the complete current Rust and Web baseline.
- Confirmed current baseline is green: 68 Rust tests and 13 Web tests.
- Confirmed `annotagent doctor` exits successfully in offline/mock-capable mode.
- Added and executed `./scripts/acceptance.sh`; the complete baseline runner exits 0.
- Derived the initial Workflow Alpha gap list from source inspection rather than existing green tests.
- Completed the Milestone 1 protocol/state audit and closed the discovered gaps.
- Added SemanticMask, Attributes, and Relations to the typed Artifact/annotation data plane.
- Added immutable Artifact revision/replacement lineage and persisted original/refined field-line Artifacts.
- Added a distinct non-terminal `AwaitingReview` Run state with a one-time backward migration for legacy terminal rows.
- Preserved task runs and remapped tool-call, annotation, Artifact, event, and revision references during history import.
- Added structured Provider and Task failure events; exact timeout/task/provider/model/retry/elapsed details now persist in trace and terminal history.
- Completed the Workflow v2 schema with generic node kinds, explicit typed ports/edges, bounded retry/fallback/review/resource policies, and precise static validation.
- Added immutable published snapshots with stable semantic content hashes and frozen Skill, Model, and prompt/resource bindings.
- Added schema migration v2 and history import/export support for immutable Run workflow snapshots.
- Added backward-compatible zero/multi-Skill Projects, namespaced extension catalogs, and deterministic visual-profile merge conflict evidence.
- Proved Generic Project creation and Workflow suggestion without RoboCup, while keeping the legacy single-Skill executor explicitly separated from the upcoming DAG executor.

## In progress

- Designing the persisted node-execution/checkpoint contract for Milestone 3 before replacing compatibility execution.

## Next

1. Execute an immutable published Workflow snapshot rather than a mutable Project/Skill graph.
2. Add typed node input/output snapshots, cache keys, retries, fallback and gate branch decisions.
3. Add durable HumanReview suspension/resume and cancellation tests around the generic DAG executor.

## Current release gaps

- Published Drafts are persisted and immutable, but the main Run path still records and executes an honest compatibility snapshot rather than an explicitly selected published DAG.
- The existing hybrid executor is a linear foundation, not the required branching, resumable DAG Runtime.
- Dataset coordination has in-process pause/resume but no durable per-node checkpoint, active worker lease, or restart resume.
- Workflow Dry Run is static validation, not sample-image sandbox execution.
- Workflow Editor cannot yet add/delete nodes or edges, clone versions, or archive drafts.
- Registry metadata, deterministic CV backend, HTTP health/capabilities protocol, and JSON-only Provider fallback are incomplete.
- Review lacks the full geometry editing and undo/redo gate.
- Data import is image ingestion/history import, not Native/COCO/LabelMe annotation import.
- Generic and RoboCup offline demo commands required by the release do not exist yet.

## Recent tests

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. The post-Milestone-2 acceptance run passed 79 Rust tests and 13 Web tests with all build/static checks at exit 0.

## Recent commit

- `684ce6f feat(workflow): add versioned typed workflow contracts`
- `309d31a fix(runtime): complete typed artifact and failure semantics`
