# AnnotAgent Acceptance Evidence

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
| Published snapshot DAG execution | PASS | Runtime and product tests execute the exact immutable version with parallel scheduling, branch/review/resume, retry/fallback/timeout/cancel, cache, usage, replayable trace, persisted checkpoint, and exact selection for image and Dataset child Runs. |
| Checkpoint/restart resume/global budget | PASS | SQLite v3 batches persist full checkpoints, leases, exact consumed/reserved budgets, and monotonic events; the concurrency-4 100-image pause/restart/resume gate completes with no duplicate child Run. |
| Mixed model backends | PASS | Complete registry metadata, mock/OpenAI-compatible/HTTP JSON/deterministic CV adapters, worker discovery protocol, strict JSON-only fallback, structured errors, secret references, and GUI health are tested. Live weights remain conditional. |
| Workflow Advisor/editor | PASS | Registry-bounded Mock/live Advisor paths, full persisted edit lifecycle, selected-image sandbox Dry Run, compare/publish/clone/archive, explicit Run version selection, HTTP journey, and real browser journey pass. Live Qwen advice remains conditional. |
| RoboCup hybrid release example | PASS | Three Skill-owned templates, labelled evaluation, real hard-negative validation, typed detector/VLM evidence, Review Gate/Commit, and the stable offline hybrid demo pass. |
| Review geometry editing | PASS | Bbox move/resize, keypoint/vertex drag/add/delete, empty-canvas creation, attributes, correction reason, undo/redo, revision persistence, and split before/after are tested and browser-verified. |
| Annotation import/export round trips | PASS | Native exact/provenance/revision, representable COCO/LabelMe/YOLO, lossy warnings, dry-run/mapping, and corrupt-record isolation pass. |
| Security release gate | PASS | Path/symlink containment, ZIP rejection-before-extraction, pre-decode pixel limit, endpoint/credential metadata validation, output scope checks, image-text prompt boundary, redaction/history scans, and rejection-before-write are tested. |
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

## Milestone 9 — hardening, observability, product DAG execution, and release

Implementation commit: `b3ba536 feat(release): complete Workflow Alpha execution and hardening`

Final acceptance command:

| Command group | Exit | Evidence |
| --- | ---: | --- |
| Core domain boundary + repository secret-prefix scans | 0 | Domain vocabulary is absent from `annotagent-core`; no live-key prefix is present in repository content. |
| `cargo fmt --all -- --check` | 0 | No formatting diff. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warning or lint suppression. |
| `cargo test --workspace --all-features` | 0 | 113 Rust unit/integration tests pass; 0 fail; all doc tests pass. |
| `cargo build --workspace --all-features` | 0 | Complete workspace dev build succeeds. |
| Web typecheck/test/build | 0 | TypeScript passes; 7 files / 13 tests pass; Vite production build succeeds. |
| `cargo run -p annotagent -- doctor` | 0 | Workspace is writable; 28 SQLite tables and migrations pass; mock mode remains available. Port 8787 was occupied by the intentional local browser-test server. |
| `cargo run -p annotagent -- demo generic-workflow` | 0 | `completed`; 2 Artifacts, 2 commits, no review, 2 mock model calls across detector/segmenter/Validator/Gate/Commit trace. |
| `cargo run -p annotagent -- demo robocup-hybrid` | 0 | `completed_with_review`; 3 Artifacts, 0 unsafe commits, 2 mock model calls, real `possible_white_shoe` Validator evidence and Review routing. |
| `./scripts/acceptance.sh` | 0 | Executes every command above as one fail-fast release gate. |

Release evidence:

1. `selected_published_workflow_executes_the_generic_dag_and_persists_checkpoint` creates a zero-Skill Generic bbox Project, publishes a valid Workflow, starts the exact version, commits through `published_dag_runtime`, persists its selected content hash, typed Artifact/annotation, node events, and checkpoint, and contains no legacy engine claim.
2. `dataset_batch_executes_the_exact_published_workflow_for_every_child_run` freezes one Published Version into a two-image Batch and asserts both child Runs use the same content hash, generic DAG engine, and checkpoint. The HTTP editor journey repeats version selection through `/api/projects/{id}/batches`.
3. The pre-existing 100-image gate still runs at concurrency four, observes intermediate progress, pauses, drops the original application owner, reopens SQLite, resumes, and finishes with exactly 100 child Runs and matching consumed/history totals.
4. Settings reject an `Authorization` custom header before writing `.annotagent/settings.toml`. Provider/registry tests reject credential-bearing URLs, nested API-key-like metadata, invalid environment references, and non-HTTP worker endpoints. API keys remain write-only and outside TOML/SQLite/history.
5. Project traversal and a real symlink escape are rejected. ZIP imports are rejected before extraction, removing the traversal surface. Image header pixel limits are checked before full decode. HTTP output must match protocol/model/image/task/type/label scope before persistence.
6. OpenAI-compatible system/backend prompts state that image text is untrusted visual data and never an instruction. Redaction tests remove Authorization and inline base64; history export tests assert secrets are absent.
7. Run summaries now expose Project, immutable Workflow, current node/status, Artifact count, validation codes, retries/fallbacks, model identity, token/cost, timeout, checkpoint, review suspension, active/last state, and terminal reason. TUI `/inspect` exposes the same durable facts.
8. In-app browser release verification on the rebuilt local server confirmed `Start dataset batch`, the exact Published Workflow selector, server-owned active/last panels, all Runs observability fields, and absence of `run reached a terminal condition`; legacy gaps render an explicit evidence-limitation message instead.

Live Qwen and configured external-weight inference remain `LIVE-CONDITIONAL`. No conversation credential was read or used, and no fixture output is presented as live inference.

Milestone 9 status: `PASS`. AnnotAgent Workflow Alpha passes the complete offline Release Gate.

## Label Pipeline Alpha release-blocking matrix — 2026-08-27

The active product release path is now Label Pipeline Alpha. The completed Workflow Alpha evidence
above remains the foundation and RoboCup regression baseline, but RoboCup-specific quality is not a
primary blocker for this release.

| # | Release blocker | Status after LP1 | Evidence / remaining work |
| ---: | --- | --- | --- |
| 1 | Generic Project runs without RoboCup Skill | PASS (foundation) | Existing generic Project/DAG tests remain green. |
| 2 | Whole-image Classification Skill | PASS | Mock Image → Classification → Commit executes through the persisted application Run path. |
| 3 | Crop Classification Skill | PASS (Runtime) | Shared Detection → Core Crop → Classification → Attach → Gate → Commit executes offline. |
| 4 | Detection Skill outputs DetectionSet | PASS (Runtime) | Formal detection-only Skill emits scoped `DetectionSetArtifact`; mock and HTTP bindings pass. |
| 5 | Crop outputs parent-linked CropSet | PASS | Core Crop executes fan-out and Trace preserves exact parent references. |
| 6 | One shared detector serves three Labels once/image | PASS | The compiler emits one shared node; the Runtime executes each compiled node once and the Replay test observes the detector call count remain one. |
| 7 | Static type errors block publish | PASS | Label validator paths are merged into product validation; an unknown Model remains editable but blocks publish. |
| 8 | Advisor result is a Draft | PASS | Mock and LLM paths start from an exact target task/Label safe Draft and never publish or execute automatically. |
| 9 | Human edits Draft | PASS (API) | Editing the composition recompiles the flat DAG and survives persistence; product GUI is LP5. |
| 10 | Dry Run writes no formal annotation | PASS | Real typed Pipeline runners execute 1–10 images in a sandbox and create no Run/Annotation record. |
| 11 | Published Version is immutable | PASS | Snapshot hash now includes optional Label Pipeline composition. |
| 12 | Run pins Workflow Version | PASS (foundation) | Existing image and Dataset exact-version tests pass. |
| 13 | Node Inspector shows I/O/config/timing/error | PASS (API) | Product API exposes typed inputs/outputs, full node config, status, latency, attempts, cache, usage, and error; GUI is LP5. |
| 14 | Classifier Replay does not rerun detector | PASS (Runtime) | `crop_classification_replay_keeps_shared_detector_checkpoint` observes detector 1, classifier 2. |
| 15 | Three mock demos pass offline | PASS | Three generic example schemas and executable whole-image, detection, and crop-classification flows pass offline. |
| 16 | 100 synthetic images pass | PASS | Application test executes 100 synthetic images using one exact published Label Pipeline version. |
| 17 | Pause/Resume/Cancel/active recovery | PASS | Label Pipeline uses the same durable coordinator; control, reconciliation, exclusion, and 100-image restart/recovery gates pass. |
| 18 | All Rust/Web checks pass | PARTIAL | LP3: 124 Rust tests and strict Clippy pass; final Web/browser gate is LP5. |
| 19 | Core contains no domain/implementation branches | PASS | Core scan for RoboCup, detector product names, and domain Labels is clean. |
| 20 | No push/remote change/historical API key | PASS | Work remains local; no credential was read or used. |

## Label Pipeline Alpha LP1 — core composition and Artifact contracts

Implementation and evidence commit: recorded by the LP1 local commit containing this section.

Acceptance commands:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo test -p annotagent-core` | 0 | 26 tests pass, including four new Label Pipeline contract/compiler tests. |
| `cargo test --workspace --all-features` | 0 | 117 Rust tests pass; 0 fail; all doc tests pass. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings or lint suppression. |
| Core domain scan | 0 | No RoboCup, detector product name, or concrete RoboCup Label branch in `annotagent-core`. |

Direct evidence:

1. `LabelWorkflowComposition::compile_draft` emits exactly one `core.image_input`, each shared step
   once, and ordinary typed edges into any number of Label Pipelines. The immutable Workflow
   snapshot content hash includes the composition.
2. `LabelPipelineStaticValidator` checks target task/Label membership, global step identity,
   shared-stage ownership, input/output Artifact types, real Node and Model Registry identities,
   model capability, and enabled Skill version.
3. `DetectionSetArtifact`, `CropSetArtifact`, `ClassificationSetArtifact`, and
   `AnnotationCandidateSet` validate confidence, identity, set membership, and lineage.
4. Image + DetectionSet → CropSet expands each Detection with padding while retaining its exact
   parent item reference. DetectionSet + ClassificationSet → AnnotationCandidateSet joins by that
   exact reference and never by array order.
5. Classification records distinguish whole-image, Detection, and Crop subjects. A Crop
   classification is invalid without a parent Detection reference.
6. The full pre-existing Rust suite, including RoboCup extensions and 100-image recovery, remains
   green. Core contains no product-name or domain-label conditional logic.

LP1 status: `PASS`. Label Pipeline Alpha overall remains `INCOMPLETE` until LP2–LP5 pass.

## Label Pipeline Alpha LP2 — executable Core nodes and formal Skills

Implementation and evidence commit: recorded by the LP2 local commit containing this section.

Acceptance commands:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo test -p annotagent-skill-yolo --test label_pipeline_runtime` | 0 | Three offline executable pipelines pass. |
| `cargo test -p annotagent-provider pipeline_backends` | 0 | Generic HTTP JSON detector/classifier and OpenAI-compatible VLM classifier pass. |
| `cargo test --workspace --all-features` | 0 | 122 Rust tests pass; 0 fail; all doc tests pass. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings or lint suppression. |

Direct evidence:

1. Existing `PublishedDagExecutor` carries Pipeline Artifacts through node inputs/outputs, Trace,
   checkpoint serialization, deterministic cache keys, Human Review, Commit, and result records.
   Old `VisionArtifact` execution remains intact.
2. `CorePipelineRunner` implements domain-neutral Crop, Filter, Map Label, Attach Result, Attach
   Attribute, and Confidence Gate. Crop fans out by Detection reference; Attach Result fans in by
   the exact parent reference. Confidence Gate marks passed sets/candidates Valid and low-confidence
   sets/candidates NeedsReview.
3. `annotagent-skill-classification` accepts Image or CropSet and produces only
   `ClassificationSetArtifact`. Its mock backend supports deterministic offline acceptance.
4. `annotagent-skill-yolo` accepts Image and produces only `DetectionSetArtifact`. The crate has no
   Crop operation; the “Detect & Crop” behavior is an external graph composition.
5. `HttpJsonPipelineBackend` enforces protocol version, request id, model identity, model binding,
   image id, node id, output type, retry/cancellation, and Artifact validation for both detector and
   classifier endpoints.
6. `OpenAiCompatiblePipelineClassifier` exposes one bounded `submit_classifications` schema. It
   accepts only requested subject ids and configured Labels, rejects omissions/duplicates, and
   preserves Crop parent Detection references.
7. The whole-image path executes Image → Classification → Commit offline. The detection path
   executes Image → Detection → Filter → Confidence Gate → Commit and asserts the Skill never emits
   a CropSet.
8. The crop-classification path executes Image → shared Detection → Core Crop → Classification →
   Attach Result → Confidence Gate → Commit. Node Trace contains CropSet and ClassificationSet
   parent lineage.
9. `PublishedDagExecutor::replay_from` resets only the requested node and descendants. Replaying the
   classifier increments classifier calls from one to two while detector calls remain exactly one.
10. Full Workspace regression includes the existing 100-image recovery and RoboCup extension tests;
    both remain green. No historical credential was read or used.

LP2 status: `PASS`. Label Pipeline Alpha overall remains `INCOMPLETE` until LP3–LP5 pass.

## Label Pipeline Alpha LP3 — persisted examples and lifecycle gates

Implementation and evidence commit: recorded by the LP3 local commit containing this section.

Acceptance commands:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo test -p annotagent-application published_label_pipeline_executes_and_persists_typed_checkpoint` | 0 | Exact-version application execution, typed checkpoint/annotation persistence, and a 100-image Dataset batch pass. |
| `cargo test -p annotagent-skill-yolo --test label_pipeline_runtime` | 0 | Four offline gates pass: three executable flows and all three generic example schemas. |
| `cargo test --workspace --all-features` | 0 | 124 Rust tests pass; 0 fail; all doc tests pass. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings or lint suppression. |

Direct evidence:

1. `PublishedWorkflowRuntime` registers Core Pipeline nodes and formal Classification/Detection Skill
   runners alongside the legacy generic runner. It materializes the selected Project image as an
   `ImageArtifact` and executes only the frozen published snapshot.
2. Every Pipeline node output is persisted inside the normal DAG checkpoint and emits an
   `ArtifactCreated` event. Committed Detection, Classification, and candidate items are converted
   to formal stored annotations with their originating node and Pipeline Artifact provenance.
3. The application catalog registers mock classifier/detector Models and the real formal Skill/Core
   node descriptors. Publication therefore validates real registry identities instead of bypassing
   validation for examples.
4. `examples/label-pipelines/` contains generic Project Schemas for whole-image classification,
   YOLO-style detection, and shared detection → Crop → Classification. They do not enable RoboCup;
   synthetic PNG inputs are generated by tests rather than committing opaque fixtures.
5. `published_label_pipeline_executes_and_persists_typed_checkpoint` first proves one persisted
   whole-image classification Run, then executes 100 generated images as Dataset children pinned to
   the same immutable Workflow version. All 100 complete and store exactly one annotation.
6. The typed crop-classification integration gate observes one detector call, one classifier call,
   exact Detection → Crop → Classification parent references, and then replays the classifier. The
   classifier count becomes two while the detector stays at one.
7. Pipeline execution deliberately reuses the existing application coordinator, checkpoint, leases,
   control token, active-Run reservation, and startup reconciliation. Existing pause/resume/cancel,
   duplicate-start, stale-Run recovery, and persistent 100-image restart tests remain green.
8. Non-mock detection does not pretend local YOLO weights exist. Without a configured versioned HTTP
   detector binding, execution returns an explicit configuration error; no Rust weight loading or
   fabricated live result is claimed.
9. No conversation credential was read or used, and no remote or push operation occurred.

LP3 status: `PASS`. Label Pipeline Alpha overall remains `INCOMPLETE` until LP4–LP5 pass.

## Label Pipeline Alpha LP4 — bounded Advisor, real Dry Run, Inspector, and Replay APIs

Implementation and evidence commit: recorded by the LP4 local commit containing this section.

Acceptance commands:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo test -p annotagent-application target_label_advisor_draft_is_editable_dry_runnable_and_publish_blocking` | 0 | Target-Label Draft, human edit, real isolated Dry Run, immutability, and blocking registry error pass. |
| `cargo test -p annotagent-application published_label_pipeline_executes_and_persists_typed_checkpoint` | 0 | Persisted Inspector and classifier Replay preserve Image Input; exact-version 100-image batch passes. |
| `cargo test -p annotagent-server label_pipeline_http_advisor_dry_run_inspector_and_replay_are_real` | 0 | Complete HTTP product journey passes. |
| `cargo test --workspace --all-features` | 0 | 126 Rust tests pass; 0 fail; all doc tests pass. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings or lint suppression. |

Direct evidence:

1. `WorkflowAdvisorInput` can carry one exact `target_task_id` and `target_label`. The server rejects
   half-specified targets. The deterministic Advisor supports only registered Classification and
   Detection compositions; unsupported task kinds return a clear error.
2. Live LLM advice receives the same target-scoped Registry input and safe base Draft. Its only
   action can select enum-bounded existing node/model ids and review gates; it cannot emit code,
   URLs, arbitrary nodes, Validators, Refiners, or execute/publish the result.
3. Label authoring data remains the source of truth for methods while Project Schema remains the
   source of truth for semantics. Saving re-compiles `LabelWorkflowComposition` to a flat DAG and
   preserves Draft identity/lifecycle metadata.
4. `LabelPipelineStaticValidator` issues are merged into product `WorkflowValidationReport` paths.
   A deliberately unknown Model remains saveable for human correction but publication is rejected.
5. Label Pipeline Dry Run no longer fabricates completed node rows. It builds an ephemeral immutable
   snapshot and calls the same Core/Classification/Detection DAG runners as Published Runs for each
   of 1–10 selected images. The test verifies Pipeline output types and zero persisted Runs.
6. Published Run snapshots now retain non-secret source image identity and the exact DAG checkpoint.
   `/api/runs/{id}/pipeline-artifacts` exposes per-node configuration, typed inputs/outputs, status,
   latency, attempts, cache hit, usage, and structured error.
7. `/api/runs/{id}/replay/{node}` invokes `PublishedDagExecutor::replay_from` using the immutable
   Workflow and stored checkpoint. The classifier and descendants re-execute while the Image Input
   output remains equal and is reported as preserved upstream state.
8. Replay is a sandbox and does not create formal annotations. Non-mock historical Runs are rejected
   rather than recovering a secret from history; no historical or conversation credential is read.
9. The HTTP integration gate performs target suggestion, real Dry Run, immutable publish, exact
   Run, Artifact inspection, and classifier Replay through the routes consumed by the Web GUI.

LP4 status: `PASS`. Label Pipeline Alpha overall remains `INCOMPLETE` until LP5 passes.
