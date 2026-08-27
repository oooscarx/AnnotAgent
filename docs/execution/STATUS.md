# Workflow Alpha Status

Last updated: 2026-08-27 08:12 CST

## Current milestone

Milestone 9 — complete. AnnotAgent Workflow Alpha passes the offline Release Gate.

## Completed

- M0 established the Git/test baseline, fail-fast acceptance script, execution ledger, and recovery documents.
- M1 completed standard tool-call replay, structured/model/UI ToolResult separation, all required typed Artifacts, direct Refiner validation/Commit, state semantics, duplicate/idempotent start, active/last Run recovery, and structured failures.
- M2 added strongly typed versioned Workflows, immutable content-addressed snapshots, precise static validation, zero/multi-Skill Projects, namespaced registries, and deterministic visual merge evidence.
- M3 added the published-snapshot DAG executor with parallel waves, retry/fallback, timeout/cancel, caching, HumanReview suspension/resume, safe Commit, checkpoints, and replayable node trace.
- M4 added Model Registry metadata and mock, OpenAI-compatible, JSON-only, HTTP JSON worker, and deterministic pixel-CV backends with typed v1 contracts and honest health/error behavior.
- M5 added durable Dataset Batches, leases, transactionally reserved global budgets, persistent checkpoints, pause/restart/resume/cancel, orphan recovery, and the 100-image no-duplicate gate.
- M6 added bounded Mock/LLM Advisors and the real Draft edit/validate/Dry Run/publish/clone/archive/compare/version-select product loop.
- M7 added RoboCup-owned hybrid templates, Validators/Refiners/evidence tools, typed detector→semantic→validation→review/Commit execution, and ground-truth evaluation.
- M8 completed Review geometry create/edit/delete/undo/redo/before-after/revision and Native/COCO/LabelMe/YOLO import/export compatibility round trips.
- M9 wired exact Published Workflow selection into product image Runs and every Dataset child Run, persisted DAG checkpoints/Artifacts/node status, exposed full GUI/TUI observability, hardened settings/endpoints/paths/archives/pixel limits/untrusted output/secrets, and added stable Generic and RoboCup hybrid offline demos.
- The Runs page no longer shows the legacy-only `run reached a terminal condition`; it derives structured Provider/Task failures, validation evidence, or an explicit legacy evidence limitation.
- All required release documents exist and describe the actual implementation rather than the pre-Alpha compatibility roadmap.

## Final acceptance

- `./scripts/acceptance.sh` exits 0.
- Strict all-target/all-feature Clippy passes with `-D warnings`.
- 113 Rust tests pass; 0 fail. The 100-image concurrency-four pause/restart/resume test completes with exactly 100 child Runs.
- 7 Web test files / 13 tests pass; typecheck and production build pass.
- Doctor reports 28 SQLite tables, migrations and mock/offline readiness.
- `demo generic-workflow` completes with 2 validated/committed Artifacts.
- `demo robocup-hybrid` produces 3 Artifacts and correctly routes `possible_white_shoe` to review with 0 unsafe commits.
- In-app browser verification confirms the Dataset Batch control, immutable version selector, active/last Run state, complete Runs audit fields, and explainable legacy errors.
- Core domain-word and repository secret-prefix scans are release blockers in `scripts/acceptance.sh` and pass.

## Live-conditional items

- Real Qwen-compatible inference requires a current credential supplied through the supported environment/keychain path. No credential from conversation history was read, restored, or used.
- Real external detector/segmenter inference requires configured weights or an endpoint. The reference worker reports degraded/`weights_unavailable` without them and is not claimed as live inference.

These are the only external conditional checks; they do not block the offline Workflow Alpha Release Gate. Product-scope limitations are documented in `docs/KNOWN_LIMITATIONS.md`.

## Recent commits

- `b3ba536 feat(release): complete Workflow Alpha execution and hardening`
- `520d307 docs(review): record milestone 8 acceptance`
- `3636e0f feat(review): complete editing and annotation round trips`
- `b200d3e docs(robocup): record milestone 7 acceptance`
- `08d3958 feat(robocup): complete hybrid skill and evaluation`
