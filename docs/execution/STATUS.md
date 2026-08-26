# Workflow Alpha Status

Last updated: 2026-08-27 07:35 CST

## Current milestone

Milestone 9 — Hardening and release acceptance.

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
- Completed the bounded Advisor input/output contract for Mock and optional workspace-LLM modes; live advice has only one strict registered submission action and is revalidated before persistence.
- Added persisted blank/template Draft creation, node/edge/parameter/binding/retry/fallback/review editing, archive, immutable publish, version clone, and version comparison.
- Added real selected-image sandbox Dry Run with bounded decode, registered backend calls, typed node output classes, latency/cost/issues, and no annotation writes.
- Replaced compatibility-only Project Workflow lists with real published versions and added explicit version selection to image Runs and honest history summaries.
- Passed the full HTTP Workflow Designer journey and an in-app browser journey covering invalid port paths, repair, one-image Dry Run, publish immutability, clone editability, version selection, and history attribution.
- Added `ANNOTAGENT_DISABLE_KEYCHAIN=1` for headless CI/browser testing; production continues to use the system keychain by default.
- Added three RoboCup-owned typed Workflow templates: `vlm-bootstrap`, `detector-first`, and `accurate-hybrid`, exposed only for Projects that enable the Skill.
- Kept specialist geometry in typed Artifacts; hybrid VLM nodes consume bbox evidence read-only and emit classification or attributes instead of rewritten coordinates.
- Extended hybrid execution evidence with aggregate model-call/compute/latency usage and proved both low-risk Commit and RoboCup white-shoe review paths.
- Added a ground-truth-only evaluation CLI with bbox/mask/keypoint/polyline/classification/attribute and operational metrics, explicit thresholds, and synthetic fixtures.
- Proved field-region mask IoU 0.75 passes the configured 0.70 gate, field-line refinement improves its coarse candidate, white-shoe candidates cannot auto-commit, and absent penalty marks are `SucceededEmpty`.
- Added a real Review editing session for bbox resize/move, keypoint and vertex dragging, vertex add/delete, empty-canvas geometry creation, attributes, correction reasons, before/after comparison, and undo/redo.
- Added persisted Human annotation creation and kept every imported annotation in `NeedsReview` with its original or explicit Imported source.
- Added Native, COCO, LabelMe, YOLO detection, and YOLO segmentation importers with label mapping, dry-run, per-record issues, and explicit compatibility warnings.
- Added Project HTTP/Web import flows and a CLI `import-annotations` command; import sources are restricted to the workspace sandbox.
- Proved Native annotation/provenance/revision round trips, representable COCO/LabelMe/YOLO round trips, and recovery from a corrupt LabelMe shape without aborting the file.
- Verified the Review editor in the in-app browser: Project-scoped queue, correctly bound new bbox, four accessible resize handles, undo/redo state restoration, split comparison, attributes, and correction reason.

## In progress

- Auditing the complete Milestone 9 security, observability, demo, documentation, and release matrix.

## Next

1. Close path/symlink/ZIP/image-limit and untrusted-model-output security gates.
2. Add the two required stable offline demo commands and expand automated API/browser acceptance.
3. Complete the release documentation and run the full blocking matrix.

## Current release gaps

- Product Run selection and history identify an immutable Workflow Version, but the image executor still records and executes the compatibility Skill graph rather than interpreting that selected generic DAG. M7 proves the shared hybrid executor and domain Artifact chain directly; product published-DAG execution remains a release gap and is not claimed.
- Generic and RoboCup offline demo commands required by the release do not exist yet.
- The final security/observability matrix and release documentation have not yet been completed.

## Recent tests

See `ACCEPTANCE_EVIDENCE.md` for commands and counts. The post-Milestone-8 acceptance script passes 107 Rust tests, 13 Web tests, all-target/all-feature Clippy, workspace/Web builds, doctor, and the 28-table SQLite migration check.

## Recent commit

- `3636e0f feat(review): complete editing and annotation round trips`
- `08d3958 feat(robocup): complete hybrid skill and evaluation`
- `364c3ee feat(workflow): complete advisor and editor lifecycle`
- `92a5c5b feat(batch): persist dataset coordination and recovery`
- `b41f55d feat(models): complete mixed vision backend registry`
- `33ab172 feat(runtime): execute immutable published DAG snapshots`
- `2c05a83 test(runtime): enforce built-in commit safety`
- `684ce6f feat(workflow): add versioned typed workflow contracts`
