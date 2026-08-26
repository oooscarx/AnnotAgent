# Workflow Alpha Acceptance Evidence

## Baseline — 2026-08-27 03:35 CST

Repository state before Milestone 0 edits:

- `git status --short --branch` → exit 0; `main...origin/main [ahead 6]`; clean worktree.
- `git log --oneline --decorate -12` → exit 0; HEAD was `e0e5cdf`.

Build and test evidence:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | No formatting diff. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings. |
| `cargo test --workspace --all-features` | 0 | 68 Rust unit/integration tests passed; 0 failed; doc tests passed. |
| `cargo build --workspace --all-features` | 0 | Complete workspace dev build succeeded. |
| `npm run typecheck` in `web/` | 0 | TypeScript check succeeded. |
| `npm run test` in `web/` | 0 | 7 files, 13 tests passed; 0 failed. |
| `npm run build` in `web/` | 0 | Production Vite build succeeded. |
| `cargo run -p annotagent -- doctor` | 0 | Config, workspace, 24-table SQLite schema, migrations, example, Web build, and port checks passed; API key was not set and mock mode remained available. |
| `./scripts/acceptance.sh` | 0 | Re-ran fmt, clippy, all Rust tests/builds, all Web checks/builds, and doctor as one fail-fast acceptance command. |

Observed baseline failures: none in the commands above.

Important limitation of this evidence: the current suites do not cover all Workflow Alpha requirements. Green baseline does not prove a typed DAG, checkpoint recovery, annotation-format imports, full Review editing, or the required demos.

## Release blocking matrix

Status values: `PASS`, `INCOMPLETE`, `LIVE-CONDITIONAL`, or `UNVERIFIED`.

| Area | Current status | Authoritative evidence / missing evidence |
| --- | --- | --- |
| Tool-call replay and structured results | PASS | Runtime/provider/storage tests cover assistant tool calls, ordered tool results, multi-call replay, and structured geometry. |
| Typed Artifact persistence | PASS | All nine required shapes, revision/replacement lineage, SQLite/history persistence, direct field-line validation, and imported-reference remapping are tested. |
| Run/Task state semantics | PASS | Empty, partial, duplicate start, active restore, stale reconciliation, distinct review suspension, and structured timeout/provider/task visibility are tested. |
| Strongly typed Workflow ports/edges | PASS | Workflow v2 defines generic node kinds, typed ports/edges/policies/resources, precise static checks, immutable published snapshots, and migration evidence. |
| Published snapshot DAG execution | INCOMPLETE | Published versions persist, but the main Run path does not select and execute them. |
| Checkpoint/restart resume/global budget | INCOMPLETE | No 100-image restart-resume gate. |
| Mixed model backends | INCOMPLETE | Mock, OpenAI-compatible, and HTTP infer adapters exist; deterministic CV, worker discovery endpoints, JSON-only fallback, and health UI remain. |
| Workflow Advisor/editor | INCOMPLETE | Suggest/save/static dry-run/publish work; node/edge lifecycle, sample-image Dry Run, clone/archive/version selection remain. |
| RoboCup hybrid release example | INCOMPLETE | Domain algorithms and an example foundation exist; required templates/evaluation CLI/complete generic DAG execution do not. |
| Review geometry editing | INCOMPLETE | Existing revision UI is not the full bbox/keypoint/polyline/polygon editor with undo/redo and before/after. |
| Annotation import/export round trips | INCOMPLETE | Export tests exist; Native/COCO/LabelMe imports and round-trip reports do not. |
| Security release gate | UNVERIFIED | Existing path/symlink and secret-redaction tests are partial; ZIP traversal, pixel limit, backend endpoint/path control, and full history/export secret scans remain. |
| Real Qwen smoke | LIVE-CONDITIONAL | No environment key was available during baseline; no live success is claimed. |
| Real external model inference | LIVE-CONDITIONAL | No model weights/path were configured; no live success is claimed. |

Milestone-specific evidence will be appended after each isolated commit.

## Milestone 1 — protocol, Artifact, and state semantics

Implementation commit: `309d31a fix(runtime): complete typed artifact and failure semantics`

`./scripts/acceptance.sh` was run after the implementation:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | Workspace formatted; all targets/features pass `-D warnings`. |
| Rust test | 0 | 72 tests passed; 0 failed. |
| Rust build | 0 | All workspace crates built. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production build passed. |
| Doctor | 0 | SQLite now reports 25 tables including `schema_migrations`; migrations and offline/mock operation pass. |

Gate evidence:

1. Tool-call replay and structured model-facing results: existing provider/runtime/storage protocol tests pass; colliding history import now additionally proves assistant call ID, tool message ID, and persisted tool-call ID remain identical after remap.
2. Multi-call order and pre-provider history rejection: `multiple_tool_calls_are_persisted_and_answered_in_order` and `tool_history_requires_one_ordered_result_per_call` pass.
3. Artifact shapes: Classification, BoundingBox, Keypoints, Polyline, Polygon, SemanticMask, InstanceMask, Attributes, and Relations validate as typed values.
4. Field-line direct path: `coarse_field_line_is_refined_validated_committed_and_revisioned` proves revision 2 replaces revision 1, both are persisted, validation/commit events exist, and only one model request occurs.
5. Empty/partial semantics: absent target and optional failure tests pass.
6. Duplicate start, navigation restore, and stale reconciliation: storage/server/application/Web tests pass.
7. Failure visibility: `task_timeout_is_structured_in_events_and_terminal_history` and `provider_timeout_preserves_provider_model_task_retry_and_elapsed` prove exact structured fields and durable terminal reasons.

Milestone 1 status: `PASS`.

## Milestone 2 — versioned strongly typed Workflow

Implementation commit: `684ce6f feat(workflow): add versioned typed workflow contracts`

`./scripts/acceptance.sh` was run after the implementation:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | All workspace targets/features pass formatting and `-D warnings`. |
| Rust test | 0 | 79 unit/integration tests passed; 0 failed. |
| Rust build | 0 | All workspace crates built. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production build passed. |
| Doctor | 0 | SQLite reports 25 tables; schema migrations v1/v2 and offline/mock operation pass. |

Gate evidence:

1. Multiple Workflows: `project_persists_multiple_workflow_drafts_and_published_versions` and the Generic application test persist two Draft identities and two published versions for one Project.
2. Multiple Skills: `two_skills_are_namespaced_and_visual_merge_is_deterministic` proves same-named node/tool/validator extensions coexist under deterministic namespaces; visual collision ownership and ignored source are explicit.
3. DAG/ports: `dependency_cycle_is_rejected` and `port_type_error_has_exact_input_path` prove cycles and incompatible Artifact edges are blocking; the latter reports `nodes[1].inputs.candidates`.
4. Binding safety: `unresolved_model_binding_blocks_publish` proves required model capability bindings cannot be omitted at publish time; validation also checks enabled Skills and registered Validators/Refiners.
5. Commit/runtime policy: static validation checks retry upper bounds, fallback cycles, unreachable nodes, terminal paths, and Validator/HumanReview barriers before Commit.
6. Immutability/hash: application publication rejects edits to a published Draft; `snapshot_serialization_is_stable_and_frozen` proves semantic hash material ignores lifecycle timestamps while frozen content is unaffected by later Draft edits.
7. Persistence/history: migration v2 adds `workflow_snapshot_json` without losing legacy Run rows; history round-trip preserves the exact frozen snapshot. Compatibility runs record the graph they actually execute and do not claim to have executed a published DAG.
8. Generic boundary: `generic_project_and_workflow_need_no_robocup_skill` creates, reads, and suggests multiple Workflows for a zero-Skill Project and asserts serialized output contains no RoboCup domain data.

Milestone 2 status: `PASS`.
