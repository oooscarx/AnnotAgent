# Workflow Alpha Status

Last updated: 2026-08-27 03:35 CST

## Current milestone

Milestone 0 — baseline and execution ledger.

## Completed

- Verified branch `main` is clean and six commits ahead of `origin/main` before this milestone.
- Recorded the latest twelve local commits without rewriting history.
- Ran the complete current Rust and Web baseline.
- Confirmed current baseline is green: 68 Rust tests and 13 Web tests.
- Confirmed `annotagent doctor` exits successfully in offline/mock-capable mode.
- Added and executed `./scripts/acceptance.sh`; the complete baseline runner exits 0.
- Derived the initial Workflow Alpha gap list from source inspection rather than existing green tests.

## In progress

- Preparing the isolated Milestone 0 commit and the requirement-by-requirement Milestone 1 audit.

## Next

1. Commit Milestone 0 independently.
2. Audit Milestone 1 requirement by requirement and add missing Artifact types, failure surfaces, and boundary tests before changing Workflow abstractions.
3. Design the persisted strongly typed Workflow schema and backward-compatible migration for Milestone 2.

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

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. At this checkpoint all baseline commands exit 0.

## Recent commit

`e0e5cdf feat: add typed hybrid vision workflow runtime` (pre-Milestone-0 baseline).
