# Workflow Alpha Status

Last updated: 2026-08-27 05:25 CST

## Current milestone

Milestone 4 — Model Registry and mixed vision backends.

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
- Added a generic executor that accepts only immutable published Workflow snapshots and rejects content-hash tampering.
- Added wave-parallel DAG scheduling, conditional gate routes, bounded node retry/timeout, fallback activation, cancellation, and safe Commit enforcement.
- Added serializable suspension checkpoints and HumanReview approval/resume without rerunning completed nodes.
- Added deterministic Artifact caching keyed by node/model/input/config/Skill snapshot material, with zero incremental cache-hit usage/cost.
- Added replayable per-node traces containing attempts, cache evidence, route, exact input/output Artifacts, structured errors, timing, tokens, and cost.

## In progress

- Expanding Model Registry metadata and execution adapters for Milestone 4.

## Next

1. Add complete model/version/input/output/cost/health metadata and capability resolution.
2. Add deterministic CV and versioned HTTP worker health/capabilities/inference contracts.
3. Add JSON-only VLM fallback parsing and mixed-backend failure isolation tests.

## Current release gaps

- Published Drafts are persisted and immutable, but the main Run path still records and executes an honest compatibility snapshot rather than an explicitly selected published DAG.
- The generic DAG executor is implemented, but the product Start flow does not yet expose explicit published-version selection (scheduled for the Editor/run integration milestone).
- Dataset coordination has in-process pause/resume but no durable per-node checkpoint, active worker lease, or restart resume.
- Workflow Dry Run is static validation, not sample-image sandbox execution.
- Workflow Editor cannot yet add/delete nodes or edges, clone versions, or archive drafts.
- Registry metadata, deterministic CV backend, HTTP health/capabilities protocol, and JSON-only Provider fallback are incomplete.
- Review lacks the full geometry editing and undo/redo gate.
- Data import is image ingestion/history import, not Native/COCO/LabelMe annotation import.
- Generic and RoboCup offline demo commands required by the release do not exist yet.

## Recent tests

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. The post-Milestone-3 acceptance run passed 85 Rust tests and 13 Web tests with all build/static checks at exit 0.

## Recent commit

- `33ab172 feat(runtime): execute immutable published DAG snapshots`
- `2c05a83 test(runtime): enforce built-in commit safety`
- `684ce6f feat(workflow): add versioned typed workflow contracts`
