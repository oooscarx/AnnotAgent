# Workflow Alpha Status

Last updated: 2026-08-27 06:11 CST

## Current milestone

Milestone 6 — Advisor and Workflow Editor.

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
- Completed the Model Registry with version, capability, input/output, pricing, health, limit, endpoint/path, and secret-reference metadata.
- Added real mock, OpenAI-compatible, HTTP JSON, and deterministic pixel-CV backends behind typed Artifact contracts.
- Added the versioned `/health`, `/v1/capabilities`, and `/v1/infer` worker protocol with bounded inline images, timeout/cancellation metadata, identity checks, usage, warnings, timings, and structured errors.
- Added a reference Python worker that is explicitly a fixture without weights and performs real Ultralytics detection only when a local model path is configured.
- Added strict JSON-only action schemas and promotion into registered tool calls, plus actual/estimated/unknown usage handling and secret redaction.
- Exposed model health on the Models page and proved incompatible capabilities block Workflow publication.
- Added independent durable Batch identities with SQLite v3 batch, image, and monotonic event tables.
- Added configurable multi-image concurrency, renewable worker leases, startup orphan recovery, and explicit failed-image retry.
- Added atomic global budget reservations with exact-decimal consumed/reserved ledgers for tokens, requests, images, cost, and optional wall-clock deadlines.
- Persisted Workflow/Project snapshots, per-image and per-node status, Artifact references, retry counters, review suspensions, child Run references, and event position in recoverable checkpoints.
- Added persistent pause/resume/cancel semantics; cancelled batches cannot claim or start later nodes and completed images are never repeated.
- Added Batch HTTP APIs, progress summaries, active Batch state on Project pages, and mutual exclusion between Batch and single-image Run starts.
- Passed the generated 100-image concurrency-4 pause → server restart → resume gate with exactly 100 child Runs and matching history/budget totals.

## In progress

- Completing constrained Advisor output and the persisted Workflow editing lifecycle for Milestone 6.

## Next

1. Constrain LLM Advisor suggestions to the Workflow schema and validate every binding.
2. Add all node/edge editing actions, clone/archive/version lifecycle, and explicit Run version selection.
3. Execute sample-image Dry Runs and compare candidate Workflow versions with persisted evidence.

## Current release gaps

- Published Drafts are persisted and immutable, but the main Run path still records and executes an honest compatibility snapshot rather than an explicitly selected published DAG.
- The generic DAG executor is implemented, but the product Start flow does not yet expose explicit published-version selection (scheduled for the Editor/run integration milestone).
- Workflow Dry Run is static validation, not sample-image sandbox execution.
- Workflow Editor cannot yet add/delete nodes or edges, clone versions, or archive drafts.
- Review lacks the full geometry editing and undo/redo gate.
- Data import is image ingestion/history import, not Native/COCO/LabelMe annotation import.
- Generic and RoboCup offline demo commands required by the release do not exist yet.

## Recent tests

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. The post-Milestone-5 acceptance run passed 98 Rust tests and 13 Web tests with all build/static checks at exit 0.

## Recent commit

- `92a5c5b feat(batch): persist dataset coordination and recovery`
- `b41f55d feat(models): complete mixed vision backend registry`
- `33ab172 feat(runtime): execute immutable published DAG snapshots`
- `2c05a83 test(runtime): enforce built-in commit safety`
- `684ce6f feat(workflow): add versioned typed workflow contracts`
