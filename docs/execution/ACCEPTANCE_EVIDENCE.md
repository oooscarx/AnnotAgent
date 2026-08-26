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
| Published snapshot DAG execution | PASS | Generic runtime tests execute an immutable published snapshot with parallel scheduling, branch/review/resume, retry/fallback/timeout/cancel, cache, usage, and replayable I/O trace. Product version-selection UI remains M6 scope. |
| Checkpoint/restart resume/global budget | PASS | SQLite v3 batches persist full checkpoints, leases, exact consumed/reserved budgets, and monotonic events; the concurrency-4 100-image pause/restart/resume gate completes with no duplicate child Run. |
| Mixed model backends | PASS | Complete registry metadata, mock/OpenAI-compatible/HTTP JSON/deterministic CV adapters, worker discovery protocol, strict JSON-only fallback, structured errors, secret references, and GUI health are tested. Live weights remain conditional. |
| Workflow Advisor/editor | PASS | Registry-bounded Mock/live Advisor paths, full persisted edit lifecycle, selected-image sandbox Dry Run, compare/publish/clone/archive, explicit Run version selection, HTTP journey, and real browser journey pass. Live Qwen advice remains conditional. |
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

## Milestone 3 — generic published-snapshot DAG Runtime

Implementation commits: `33ab172 feat(runtime): execute immutable published DAG snapshots` and `2c05a83 test(runtime): enforce built-in commit safety`

`./scripts/acceptance.sh` was run after the implementation:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | All workspace targets/features pass formatting and `-D warnings`. |
| Rust test | 0 | 85 unit/integration tests passed; 0 failed. |
| Rust build | 0 | All workspace crates built. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production build passed. |
| Doctor | 0 | Database, migrations, workspace, example, Web build, and offline/mock checks pass. |

Gate evidence from `crates/annotagent-runtime/tests/published_dag.rs`:

1. Target graph: `published_dag_branches_suspends_resumes_caches_and_replays_trace` executes input → mock detector → deterministic refiner → Validator → confidence Gate, with pass → Commit and review → HumanReview → Commit routes.
2. Branching/suspension: high confidence skips review and commits; low confidence returns `AwaitingReview`, serializes/restores the complete checkpoint, accepts explicit approval, resumes, and commits without rerunning completed nodes.
3. Retry/fallback: `retry_limit_and_fallback_are_bounded` proves success on the exact third allowed attempt and activation of a declared fallback after exhaustion.
4. Cancellation/timeout: cancellation aborts the running node and marks not-yet-started nodes Cancelled; timeout produces structured `node_timeout` evidence.
5. Parallel scheduling: two independent ready nodes overlap, with measured maximum concurrency of two.
6. Cache/usage: repeated deterministic refinement hits the content-addressed Artifact cache; cache-hit usage and cost are zero while non-cached model usage remains accounted per node.
7. Trace/replay: checkpoints contain node statuses, exact input/output Artifact snapshots, branch routes, attempts, cache keys/hits, structured failure, timestamps, tokens, and cost and round-trip through JSON.
8. Immutable input: changing snapshot content without changing the published content hash is rejected before execution.
9. Safe Commit: built-in Commit cannot be overridden by a registered runner and rejects Artifacts that are neither Validated nor Human-reviewed.

Milestone 3 status: `PASS`.

## Milestone 4 — Model Registry and mixed vision backends

Implementation commit: `b41f55d feat(models): complete mixed vision backend registry`

`./scripts/acceptance.sh` was run after the implementation:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | All workspace targets/features pass formatting and `-D warnings`. |
| Rust test | 0 | 91 unit/integration tests passed; 0 failed. |
| Rust build | 0 | All workspace crates built. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production build passed. |
| Doctor | 0 | Database, migrations, workspace, example, Web build, and offline/mock checks pass. |
| Secret scan | 0-equivalent | `rg` found no supplied-key fingerprint in tracked/untracked repository sources; exit 1 with empty output means no match. |
| Reference worker syntax | 0 | Python AST parsing succeeds without importing optional model dependencies or creating bytecode. |

Gate evidence:

1. Registry completeness and publication safety: descriptors carry display/model/version, backend, capabilities, inputs/outputs, endpoint/path, pricing, health, limits, and secret reference; `model_capability_mismatch_blocks_publish` rejects an incompatible VLM binding.
2. Shared worker protocol: the HTTP fixture serves `/health`, `/v1/capabilities`, and `/v1/infer`; discovery advertises detector, SAM-class prompted segmentation, and semantic-segmentation capabilities through the same v1 contract.
3. Typed response parsing: `http_json_backend_uses_the_shared_wire_schema` parses and validates all nine Artifact shapes, enforces image/task scope, protocol version, and expected model identity.
4. Failure semantics: `worker_error_preserves_execution_identity_and_retry` proves model, node, task, elapsed time, retry index, worker, and structured error code survive retry exhaustion. M3 fallback tests prove declared fallback activation after an unavailable runner exhausts its bounded attempts.
5. Real/offline honesty: `deterministic_cv_executes_real_pixel_algorithm` executes an actual image-pixel detector; `examples/http_vision_worker.py` runs real Ultralytics detection only with configured local weights and otherwise returns `weights_unavailable` while identifying itself as a fixture.
6. Provider degradation: tests cover native tool calls, strict JSON Schema, registered-action promotion from JSON-only responses, actual/estimated/unknown usage, timeout/retry/cancellation paths, and secret/image redaction.
7. Secret boundary and health UI: plaintext registry secrets are rejected; persisted provider settings keep the API key only in the credential store; source secret scan is empty; `/api/models` and the Models page expose health status and detail.

Milestone 4 status: `PASS`.

## Milestone 5 — Persistent Dataset Coordinator

Implementation commit: `92a5c5b feat(batch): persist dataset coordination and recovery`

`./scripts/acceptance.sh` was run after the implementation:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | All workspace targets/features pass formatting and `-D warnings`. |
| Rust test | 0 | 98 unit/integration tests passed; 0 failed. |
| Rust build | 0 | All workspace crates built. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production build passed. |
| Doctor | 0 | SQLite reports 28 tables and migrations v1–v3; workspace, example, Web build, and offline/mock checks pass. |
| Secret scan | 0-equivalent | Supplied-key fingerprints are absent from repository sources (empty `rg` result). |

Gate evidence:

1. The application gate generates 100 synthetic images, configures concurrency 4, observes intermediate progress, pauses, destroys the original application/server owner, opens a new application against the same SQLite file, and resumes to `Completed`.
2. Exactly 100 child Runs exist after resume. The test asserts all 100 Batch image IDs are completed, no remaining image exists, and no completed image executed twice.
3. The final exact-decimal Batch ledger has no reservations; input/output tokens, request count, and cost equal the sum of all persisted child Run usage records, and processed image count is 100.
4. Checkpoints include frozen Workflow version/snapshot, Project snapshot, remaining/completed images, per-node states, Artifact references, retry counters, review suspensions, budget ledger, child Run references, and event sequence.
5. `concurrent_reservations_cannot_oversell_global_budget` releases two threads simultaneously against one SQLite transaction boundary and proves exactly one reservation succeeds while the other atomically produces `BudgetExceeded`.
6. `startup_requeues_orphaned_image_and_checkpoint_survives_reopen` proves a new process owner recovers an orphaned lease, releases stale reservations, and resumes the unfinished image while preserving the final checkpoint.
7. `failed_image_retry_preserves_usage_and_does_not_repeat_completed_work` proves explicit failed-image retry, cumulative usage, attempt count, and rejection of later claims after completion.
8. `cancellation_prevents_new_image_nodes_from_starting` proves Cancel changes every unfinished image to Cancelled and no `image_started` event can appear after the monotonic cancellation event.
9. The Batch HTTP test covers list/detail/progress/pause/cancel, Project `active_batch` recovery data, and 409 mutual exclusion for duplicate Batch or single Run starts. The React Project page renders the persisted Batch status and completed/total image progress.
10. No test sets an absolute runtime threshold; the full 100-image gate completed without an uncaught panic.

Milestone 5 status: `PASS`.

## Milestone 6 — Advisor and Workflow Editor

Implementation commit: `364c3ee feat(workflow): complete advisor and editor lifecycle`

Acceptance commands:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Rust fmt + clippy | 0 | All workspace targets pass formatting and `-D warnings`. |
| Rust test | 0 | All 100 Rust unit/integration tests and doc tests passed after repairing the async HTTP polling assertion. |
| Web typecheck/test/build | 0 | Typecheck passed; 7 files/13 tests passed; production Vite build passed. |
| In-app browser journey | 0-equivalent | Local GUI exercised clone/edit, exact invalid-port feedback, repair, one-image sandbox Dry Run, binding/gate edit, publish lock, clone, explicit Run version selection, and snapshot-derived Runs history. |
| `./scripts/acceptance.sh` | 0 | Fmt, all-target/all-feature Clippy, 100 Rust tests, workspace build, Web checks, doctor, 28-table SQLite, migrations, Web build, and offline/mock readiness all passed. |

Gate evidence:

1. `WorkflowAdvisorInput` contains Project Schema, enabled Skills, registered node/model catalogs, Validator/Refiner/resource IDs, cost/latency/accuracy constraints, and a bounded dataset profile. The GUI consumes this catalog instead of accepting arbitrary node/model IDs.
2. The deterministic Mock Advisor remains the offline acceptance path. The workspace-LLM Advisor has one strict `submit_workflow_advice` action, can only adjust registered bindings and review gates on a safe base Draft, has no Shell/URL/code/arbitrary tools, and passes full registry validation before persistence.
3. Blank/template/Advisor Drafts, full node and connection edits, parameters, bindings, retries, fallbacks, review gates, ports/issues, save, archive, publish, clone, and compare are persisted through application/storage/HTTP contracts.
4. `workflow_alpha_editor_journey_is_persistent_and_version_explicit` introduces an Artifact port mismatch, asserts the exact node-input issue path, repairs it, executes a selected synthetic sample through the isolated sandbox, publishes, rejects mutation, clones and republishes, compares versions, starts an explicit-version Run, and verifies the persisted selection beside the honest compatibility engine snapshot.
5. `workflow_designer_http_journey_validates_dry_runs_publishes_and_clones` repeats the product API journey, proves Project summaries expose the actual published version, and proves Run summaries derive the chosen Workflow name/version from persisted history.
6. Browser evidence on `qwen-live` showed `unknown_input_port edges[0].to_port` plus `missing_required_input nodes[2].inputs.from_field_region`; after repair, Dry Run reported one `color_1001525.png` sample at 544×448, eight node outputs, measured latency, and zero mock cost.
7. Publishing disabled all edit controls. Selecting the published template version and cloning produced a distinct editable Draft. Project Run selection used that exact workflow/version, and Runs history rendered `RoboCup Demo template workflow@v1` rather than the former hard-coded compatibility label.
8. Browser startup exposed a headless system-keychain block. `ANNOTAGENT_DISABLE_KEYCHAIN=1` now provides an explicit CI/test-only opt-out while leaving secure keychain persistence as the default.
9. Real Qwen Advisor execution is not claimed because no supported live credential was supplied during this milestone; it remains `LIVE-CONDITIONAL` in `BLOCKERS.md`.

Milestone 6 status: `PASS` offline; live Qwen advice remains conditional.

## Milestone 7 — RoboCup hybrid Skill

Implementation commit: `08d3958 feat(robocup): complete hybrid skill and evaluation`

Acceptance commands:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| `./scripts/acceptance.sh` | 0 | Formatting, all-target/all-feature Clippy, 104 Rust tests, workspace build, 7-file/13-test Web suite, production Web build, doctor, 28 SQLite tables, and migrations all pass. |
| `annotagent evaluate` synthetic fixture | 0 | The report contains every requested accuracy/operational metric and field-region mask IoU 0.75 passes the configured 0.70 threshold. |
| Secret scan | 0-equivalent | Supplied-key fingerprints are absent from tracked and untracked repository sources. |

Gate evidence:

1. `generic_project_and_workflow_need_no_robocup_skill` proves a zero-Skill Project has no RoboCup output or Workflow templates. The RoboCup application and HTTP journeys expose exactly the three Skill-owned templates and instantiate `accurate-hybrid` with the Project's enabled Skill version.
2. `robocup_owns_three_semantically_bounded_workflow_templates` proves the exact `vlm-bootstrap`, `detector-first`, and `accurate-hybrid` IDs, required review/Commit path, deterministic line refiner, and read-only bbox-to-classification VLM contract.
3. The synthetic evaluation fixture reports bbox IoU/precision/recall, mask IoU, keypoint distance, polyline point-to-line distance, classification/attribute accuracy, review/failure rate, cost, latency, and model calls. `labeled: false` is rejected rather than assigned fabricated accuracy.
4. The field-region mask score is 0.75 and passes `--minimum-field-region-iou 0.7`; the existing pixel fixture proves the refined line is closer to the white line than its coarse candidate.
5. RoboCup algorithm/runtime gates prove a white-shoe candidate returns bounded Retry then HumanReview rather than AutoAccept, and an absent penalty mark persists `SucceededEmpty` while the Run completes.
6. `robocup_hybrid_artifacts_usage_trace_and_hard_negative_review_are_real` executes detector Artifacts → semantic-only VLM output → actual RoboCup hard-negative validation → review → blocked Commit. It asserts three Artifacts, five trace nodes, two model calls, the exact `possible_white_shoe` issue, review routing, and no committed high-risk Artifact. The companion low-risk hybrid test commits validated detector/segmenter Artifacts automatically.
7. Real Qwen and external detector/segmenter smoke are `LIVE-CONDITIONAL`: no credential or configured weights were read from conversation history, and fixture backends are not represented as real inference.

Milestone 7 status: `PASS` offline; live Qwen and configured external-model smoke remain conditional.

## Milestone 8 — Review, editing, import, and round trips

Implementation commit: `3636e0f feat(review): complete editing and annotation round trips`

Acceptance commands:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| `./scripts/acceptance.sh` | 0 | Formatting, strict all-target/all-feature Clippy, 107 Rust tests, workspace build, 7-file/13-test Web suite, production Web build, doctor, 28 SQLite tables, and migrations pass. |
| Export/import integration tests | 0 | Five tests cover Native exact preservation, representable COCO/LabelMe/YOLO round trips, compatibility warnings, and corrupt-record isolation. |
| In-app browser Review journey | 0-equivalent | A Project-scoped queue created an `objects`/`ball` bbox with four named resize handles; undo restored the original revision, redo restored creation, and split before/after, attributes, and correction reason controls rendered. |
| Secret scan | 0-equivalent | No supplied-key prefix exists in repository sources. |

Gate evidence:

1. `AnnotationCanvas` supports bbox dragging and corner resize, keypoint/polyline/polygon/polygon-mask vertex dragging, double-click vertex creation, and Delete/Backspace vertex removal with accessible SVG button names.
2. Review creates geometry against a Project task of the matching kind instead of cloning an incompatible classification task. New Human annotations persist through the Run API; attributes, correction reason, notes, revision save, accept/reject/delete, before/after split, and current-session undo/redo are live controls.
3. Review results carry Project identity and the Web queue is filtered to the active Project, preventing a decision from being applied with another Project's policy.
4. Native import preserves valid annotations, source, provenance, and revision chains. COCO parses bbox, polygon, keypoints, and string RLE; LabelMe parses rectangle, point, line/linestrip, and polygon; YOLO detection and segmentation parse normalized text rows.
5. All importers support label mapping and dry-run, return record-scoped issues, and emit warnings for unrepresentable provenance/revision/attribute/relation data. A malformed LabelMe shape is skipped while the valid shape in the same file imports.
6. Product persistence maps known Project images to existing single-image or Batch child Runs, rejects duplicate annotation IDs, and routes imported records to `NeedsReview`. It does not invent an arbitrary owning Run for unmatched images.
7. Exporters continue to return explicit skipped/warning records for incompatible shapes. Native round-trip equality includes annotations, provenance, and revisions; representable COCO/LabelMe/YOLO geometry round trips within the normalized contract.

Milestone 8 status: `PASS`.
