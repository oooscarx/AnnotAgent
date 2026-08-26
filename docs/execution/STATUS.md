# Workflow Alpha Status

Last updated: 2026-08-27 03:58 CST

## Current milestone

Milestone 2 — versioned strongly typed Workflow design and migration.

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

## In progress

- Designing the explicit typed node/port/edge model and backward-compatible Workflow migration for Milestone 2.

## Next

1. Introduce namespaced multi-Skill Project bindings without breaking Project Schema v1.
2. Replace dependency-only Draft nodes with explicit typed ports, edges, retry/fallback/review/resource policies.
3. Add precise static validation and immutable Run snapshot persistence tests.

## Current release gaps

- Existing Workflow Draft nodes use a compact dependency list, not explicit typed ports and edges.
- Published Drafts are persisted and immutable, but the main Run path does not yet execute an explicitly selected published snapshot.
- The existing hybrid executor is a linear foundation, not the required branching, resumable DAG Runtime.
- Dataset coordination has in-process pause/resume but no durable per-node checkpoint, active worker lease, or restart resume.
- Workflow Dry Run is static validation, not sample-image sandbox execution.
- Workflow Editor cannot yet add/delete nodes or edges, clone versions, or archive drafts.
- Registry metadata, deterministic CV backend, HTTP health/capabilities protocol, and JSON-only Provider fallback are incomplete.
- Review lacks the full geometry editing and undo/redo gate.
- Data import is image ingestion/history import, not Native/COCO/LabelMe annotation import.
- Generic and RoboCup offline demo commands required by the release do not exist yet.

## Recent tests

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. The post-Milestone-1 acceptance run passed 72 Rust tests and 13 Web tests with all build/static checks at exit 0.

## Recent commit

- `309d31a fix(runtime): complete typed artifact and failure semantics`
- `9a19176 chore(release): establish Workflow Alpha acceptance ledger`
