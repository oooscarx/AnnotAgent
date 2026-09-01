# AnnotAgent Acceptance Evidence

## Run Preview Selection Focus Hotfix — 2026-09-02

- On Run `1f40eaed-2b3b-479b-866c-82a55ce4cf31`, clicking the visible ball bbox previously focused
  `.run-artifact-canvas`; computed style showed a 2 px primary-blue outline around the whole result
  viewer even though the bbox stroke itself was only 2.6 px.
- The result viewer is now a labelled, non-focusable region. Its interactive controls and SVG bbox
  marks remain in the tab order, and scoped SVG focus styling does not invoke the global component
  outline.
- Browser verification after the fix shows no blue preview frame after clicking the same bbox. The
  selected mark retains its annotation color, a 2.2 px non-scaling stroke and a 9% matching fill.
- Verification: 41 Web tests passed; production Web build, TypeScript and `git diff --check`
  completed successfully.

## Dataset Run History Grouping Hotfix — 2026-09-02

- Persisted Batch `0a24985b-8ced-4843-a578-1585e05d85eb` records one launch, four images and four
  child Run identities; this proves the reported rows were per-image execution records rather than
  repeated Dataset launches.
- The Batch list API retains its existing record fields and adds `progress` plus `child_run_ids`.
  The Server API regression verifies those fields for a durable one-image Batch.
- Runs filters and sorts Dataset Runs alongside legacy standalone Runs. Child Runs owned by a Batch
  are removed from the top level and remain directly navigable after expanding their Dataset Run.
- Browser verification confirms the completed four-image execution renders as one collapsed
  Dataset Run with an explanatory four-item child group.
- Verification: 17 Server tests and 41 Web tests passed; production Web build, strict Server
  Clippy, TypeScript, Rustfmt and `git diff --check` completed successfully.

## Full Dataset Run Entry Hotfix — 2026-09-02

- Activated Sample Test evidence includes a visible `Start full Run` action. Project Overview also
  renders it in the empty Active Run card, without requiring the user to discover an Advanced
  control or infer that Guidance is clickable.
- Both entry points call `POST /api/projects/{project_id}/batches` without a sample limit and send
  only the exact immutable Published Workflow identity. Success navigates to the Project-filtered
  Runs page; no billable Run is started by automated verification.
- Regression `project_guidance_uses_persisted_sample_test_and_published_state` persists a terminal
  Run from an older Workflow and proves Guidance remains `ReadyToRun`, then persists a matching
  frozen Version and proves Guidance advances to `ReadyToExport`.
- Verification: 53 Application tests (one opt-in billable smoke ignored), 17 Server tests and 41
  Web tests passed; the production Web build, focused strict Clippy, TypeScript, Rustfmt and
  `git diff --check` completed successfully.

## Persistent Sample Test Recovery Hotfix — 2026-09-02

- `GET /api/workflow-drafts/{draft_id}/sample-test` returns the persisted report and whether it is
  current relative to the editable Draft timestamp.
- The Workflow Designer HTTP journey proves a completed Sample Test survives a separate GET,
  becomes stale after a Draft edit, remains available as audit evidence, and becomes current again
  after a new Dry Run.
- The Web client restores current reports when the Test & Activate route mounts or the selected
  Draft changes. It shows an explicit stale-state message instead of reusing outdated activation
  evidence.
- Published Drafts remain visible as `Activated` read-only choices, and the HTTP journey proves
  their saved passing report remains current after publication's status-only timestamp update.
- Verification: 17 Server tests and 41 Web tests passed; production Web build, strict Server
  Clippy, Rustfmt and `git diff --check` all completed successfully.

## Pinned Qwen VLM Revision — 2026-09-02

- Workspace execution settings, the checked-in DashScope example, the Server Provider preset and
  the Web Provider catalog all name `qwen3.7-flash-2026-07-15` exactly.
- Registry Model Profile `b9c5bbe8-e21a-5784-9c52-cade259b434f` advanced from revision 1
  (`qwen3.7-flash`) to revision 2 (`qwen3.7-flash-2026-07-15`) without mutating historical
  revision or Published Workflow snapshots.
- No billable active probe or Sample Test was initiated as part of the rename.
- Verification: 17 Server tests and 40 Web tests passed; the Web production build, strict Server
  Clippy and Rustfmt also completed successfully.

## Provider Registry Dry Run Credential Hotfix — 2026-09-02

- Root cause: `dry_run_workflow_samples_with_provider` forwarded the resolved credential only to
  Label Pipeline execution. Its compatibility flat-Workflow branch rebuilt the OpenAI-compatible
  backend without that credential, causing an incorrect `ANNOTAGENT_API_KEY` environment fallback.
- `workflow_catalog_with_api_key` now injects the transient in-memory credential into the model
  provider. Static catalog consumers retain the credential-free wrapper.
- Regression test
  `flat_workflow_dry_run_uses_registry_credential_without_environment_fallback` starts a local
  OpenAI-compatible endpoint, requires the exact injected bearer token, leaves the configured
  environment locator unset, and verifies the Classification node completes in the sandbox.
- No credential value is persisted by the fix or included in production descriptors, reports, or
  logs. The test credential is a fixture-only literal.
- Verification: `cargo test --workspace --all-features` passed 308 tests with one explicitly
  ignored billable smoke; `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` all exited successfully.

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

## Expert Vision SDK M3 — typed SAM-compatible conversion chain

- `ArtifactConversionRegistry` returns the explicit DetectionSet → BoxPromptSet → MaskSet →
  DetectionSet cycle only when all three generic nodes are registered.
- Runtime test `sam_artifact_chain_preserves_original_prompt_mask_and_refined_box` verifies exact
  source Detection, Box Prompt, Mask and refined geometry lineage plus independent detector and
  prompted-segmentation evidence.
- `capability.segment` consumes Image plus one Box/Point PromptSet and returns exactly one scoped
  MaskSet through mock or bounded protocol-v1 HTTP execution.
- Python Worker SDK MaskSet serialization and prompt references pass 14/14 tests. Web typecheck and
  40/40 unit tests pass with prompt/mask/polygon Artifact support.
- No real SAM availability is claimed: generated adapters remain `missing_weights` until health,
  contracts, weights and sample conversion all pass.

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
8. Historical evidence: browser startup exposed a headless system-keychain block and originally
   added a CI opt-out. This design is superseded by the workspace-local credential implementation
   documented below; the opt-out and new keychain writes have been removed.
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
| 9 | Human edits Draft | PASS | GUI edits typed sources, Model Binding, thresholds, padding, class mapping, fallback, and JSON parameters before Save. |
| 10 | Dry Run writes no formal annotation | PASS | Real typed Pipeline runners execute 1–10 images in a sandbox and create no Run/Annotation record. |
| 11 | Published Version is immutable | PASS | Snapshot hash now includes optional Label Pipeline composition. |
| 12 | Run pins Workflow Version | PASS (foundation) | Existing image and Dataset exact-version tests pass. |
| 13 | Node Inspector shows I/O/config/timing/error | PASS | Product GUI renders API-backed typed inputs/outputs, full node config, status, latency, attempts, cache, usage, and error. |
| 14 | Classifier Replay does not rerun detector | PASS (Runtime) | `crop_classification_replay_keeps_shared_detector_checkpoint` observes detector 1, classifier 2. |
| 15 | Three mock demos pass offline | PASS | Three generic example schemas and executable whole-image, detection, and crop-classification flows pass offline. |
| 16 | 100 synthetic images pass | PASS | Application test executes 100 synthetic images using one exact published Label Pipeline version. |
| 17 | Pause/Resume/Cancel/active recovery | PASS | Label Pipeline uses the same durable coordinator; control, reconciliation, exclusion, and 100-image restart/recovery gates pass. |
| 18 | All Rust/Web checks pass | PASS | 126 Rust tests, strict Clippy, Web typecheck/build, and 8-file/15-test Web suite pass. |
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

## Label Pipeline Alpha LP5 — product GUI and final release acceptance

Implementation and evidence commit: recorded by the LP5 local commit containing this section.

Final acceptance commands:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `./scripts/acceptance.sh` | 0 | Core domain scan, secret-prefix scan, fmt, strict Clippy, all Rust tests/builds, all Web checks/build, doctor, and offline generic demo pass. |
| `cargo test --workspace --all-features` | 0 | 126 Rust tests pass; 0 fail; all doc tests pass. |
| Web typecheck/test/build | 0 | TypeScript passes; 8 files / 15 tests pass; Vite production build succeeds. |
| In-app browser product journey | 0-equivalent | Label creation, target Draft, real Dry Run, immutable publish, Run, Inspector, image preview, and classifier Replay pass. |
| Core/secret scans | 0 | Core contains no RoboCup/YOLO/concrete Label branch; repository contains no live-key prefix. |

Direct evidence:

1. Project Schema has a real `Add Label` product API and GUI. The server validates the complete
   schema and enabled Skill catalog before rewriting `project.yaml`; duplicate/invalid Labels fail
   instead of appearing only in client state.
2. Workflow Designer renders `Shared Stages` and one lane per Label Pipeline. Shared stages are
   visually distinguished and preserve one flat node identity when compiled.
3. Node Catalog additions create typed Pipeline steps. The editor exposes source node/port, Model
   Binding, confidence threshold, minimum confidence, Crop padding, class mapping/parameters,
   fallback, and removal. Invalid intermediate edits remain Drafts and publication stays blocked.
4. `Apply Detect & Crop template` is enabled only on a Detection Pipeline and constructs detector →
   filter → Core Crop → Classification → Attach Result → Confidence Gate → Commit. The Web helper
   test asserts Crop remains `core.crop` and Detection remains `detection_set`.
5. Save persists the authoring projection and recompiles it. Dry Run executes 1–10 images through
   the actual typed Runtime. Publish freezes the complete composition and disables every edit
   control; the browser gate confirmed an exact Published Version was selected for the Run.
6. Inspector chooses a persisted Pipeline Run and exact node. It renders the original Project image,
   Detection bbox overlays, Crop views clipped from that image, typed input/output JSON, Model/config,
   status, latency, attempts, cache, usage, and structured error. Replayed nodes show only their
   latest trace rather than duplicate Inspector rows.
7. Browser classifier Replay completed in the sandbox and reported classifier, confidence Gate, and
   Commit re-executed while `core.image_input` was preserved. No formal annotation was written by
   Replay.
8. The browser exercise found and fixed a real binding bug: `mock-classifier` and `mock-detector`
   now select their offline backends even when workspace settings default to a live provider. No
   absent API key is requested for an explicitly mock-bound node.
9. Desktop visual inspection confirmed the Pipeline lanes and Artifact Inspector remain readable;
   responsive CSS collapses Inspector and headings at smaller breakpoints. Unavailable actions stay
   disabled with an explicit reason; no disabled control claims missing Runtime behavior.
10. The full existing RoboCup, persistent Batch recovery, Review, import/export, and security suites
    remain green. No push or remote mutation occurred and no conversation credential was read or
    used.

LP5 status: `PASS`. AnnotAgent Label Pipeline Alpha passes all 20 offline Release Blocking gates.

## Live VLM Detection and Crop product smoke

Date: 2026-08-27 CST.

Direct evidence:

1. `annotagent-skill-vlm-detection` defines one formal Image → DetectionSet operation. It owns no
   filtering, cropping, review, or commit behavior.
2. The OpenAI-compatible request adapter places the grounding prompt and Base64 image in the same
   user content, accepts legacy string and content-part responses, and prevents a node-level
   `enable_thinking=false` switch from conflicting with a saved global `reasoning_effort` value.
3. Qwen grounding uses its documented 0–1000 xyxy coordinate convention. The adapter validates
   labels, confidence, bounds, and geometry before normalizing the box into Core's `NormalizedRect`.
4. One-image Dry Run on B-Human `color_771292.png` executed Image Input → VLM Detection → Filter →
   Core Crop → Artifact Cache and Confidence Gate → Commit with no issues.
5. Immutable Workflow `6095280f-9c7c-45ce-908d-9980ee8c77fe@1` produced formal Run
   `367e9a0e-5fea-485a-adf7-b437502c2727`. The Run completed with exactly one `football` detection,
   confidence `0.98`, normalized rect `[0.432, 0.356, 0.046, 0.046]`, and exactly one Crop retaining
   the detection item as its parent reference.
6. A final product-named clone, `VLM Football Detect & Crop`, is the most recently published and
   therefore default Workflow for Project `bhuman-vlm-football`. The GUI server remains available
   at `http://127.0.0.1:8787` for click-to-run use.
7. `cargo test --workspace --all-features` passes 131 Rust tests plus doc tests; strict workspace
   Clippy and Web tests/typecheck/build also pass. No detector weights were loaded, no remote was
   modified, and no conversation credential was read, printed, or persisted by this work.

Live VLM Detection and Crop smoke status: `PASS`.

## RoboCup Ball-only scope reset — 2026-08-28

The earlier broad RoboCup template evidence above is retained as historical execution evidence but
is no longer the current product contract. The active contract now requires:

1. `examples/robocup/project.yaml` contains exactly one bounding-box task and only label `ball`;
2. the compatibility Skill graph contains exactly one `objects` node;
3. the layered Domain Skill contributes exactly one Ball Validator and two Ball templates;
4. no current RoboCup template contains field, robot or penalty annotation nodes;
5. the visual label map contains only `ball`;
6. the active local workspace contains only the five-image `robocup-ball` Project, with a fresh
   database; removed Projects/history/test residue remain in a recoverable hidden archive.

Final verification:

| Command / journey | Exit | Evidence |
| --- | ---: | --- |
| `./scripts/acceptance.sh` | 0 | Domain and secret scans, fmt, strict Clippy, all Rust tests/builds, all Web checks/build, doctor, and all three offline demos passed. |
| `cargo test --workspace --all-features` | 0 | 148 Rust unit/integration tests passed; 0 failed; all doc tests passed. |
| Web typecheck/test/build | 0 | TypeScript passed; 10 files / 24 tests passed; Vite production build succeeded. |
| In-app browser smoke on port 8791 | 0-equivalent | Home showed one Project and five images; Project Schema showed only `objects` / `ball`; Workflow Designer showed only the two RoboCup Ball templates and `ball_hard_negative`. |

The previous `qwen-live`, `robocup-demo`, B-Human legacy exports, pre-reset history database, and
`e2e-guided` test residue are stored under
`workspace/.annotagent/deleted-projects/2026-08-28/`. No live provider request was made during the
final smoke, and the temporary GUI server was stopped after inspection.

RoboCup Ball-only scope reset status: `PASS`.

## OpenAI-compatible action recovery and workspace-local credentials — 2026-08-28

1. The provider request builder sends native tools without a simultaneous JSON-schema response
   constraint. Qwen thinking mode remains compatible because no unsupported forced `tool_choice`
   is sent.
2. A valid registered JSON action returned in assistant content is promoted to a normal tool call,
   while native `tool_calls` remain unchanged and malformed/plain content remains untrusted text.
3. Live Run `76e0ed20-771c-4e53-ab97-b682070b38e6` completed on
   `color_771292.png`, passed deterministic Ball validation with no issues, and committed annotation
   `5b2b9c79-b288-47ac-979e-fe759d724848`. Each of five model responses produced one Runtime-visible
   tool call; total usage was 20,792 tokens and `$0.032276`.
4. `GET /api/settings` reports `credential_store=workspace_private_file` and continues to report the
   migrated key as configured without returning it. Filesystem inspection showed a `0700`
   credential directory and `0600` key file; the matching legacy keychain entry was absent after
   startup.
5. Strict workspace Clippy passed. All 149 Rust tests plus doc tests passed. Web typecheck, 24 tests,
   and the Vite production build passed.

OpenAI-compatible action recovery and workspace-local credential status: `PASS`.

## Bounded auxiliary-tool convergence — 2026-08-28

1. Failure Run `709bae51-d2d8-45d7-b713-89b1c8dfdc33` consumed all eight configured turns on eight
   successful `evaluate_ball_hard_negative` calls and then failed without a submission. This proves
   the failure was model action selection, not Provider parsing or tool execution.
2. `repeated_auxiliary_calls_reserve_a_bounded_convergence_turn` proves that two successful
   auxiliary calls add structured `convergence_required` feedback and the next submission commits.
   `finalization_turn_exposes_terminal_actions_without_auxiliary_tools` proves evidence tools are
   absent only during a bounded finalization turn.
3. Live Qwen Run `6df70d25-e1fe-4233-8ec1-cd4314f665ca` completed with exactly two evidence calls
   followed by `submit_annotation_candidates`. Deterministic validation accepted the candidate
   without issues and committed annotation `ba406c34-1ab8-437d-912f-622c5f7e7c7e`.
4. The live Run used 9,343 input and 6,298 output tokens across three requests, costing `$0.034535`.
5. Strict workspace Clippy passed. All 151 Rust tests plus doc tests passed. Web typecheck, 24 tests,
   and the Vite production build passed.

Bounded auxiliary-tool convergence status: `PASS`.

## Formal Annotation overlay in Run detail — 2026-08-28

1. Annotation inspection for Run `6df70d25-e1fe-4233-8ec1-cd4314f665ca` returned Project
   `robocup-ball`, image index `4`, and committed Annotation
   `ba406c34-1ab8-437d-912f-622c5f7e7c7e` with normalized rect
   `[0.422, 0.334, 0.055, 0.067]` and confidence `0.95`.
2. The in-app browser Run page rendered `1 Annotations`, selected `color_771292.png`, displayed a
   `ball` color legend, and exposed an interactive overlay labelled `ball 95 percent`.
3. Visual inspection confirmed the box is aligned over the football and uses the Project annotation
   color rather than a black outline. The previous `No replayable Artifact` empty state is absent.
4. The server journey test exercises the new read endpoint after formal and human annotations are
   persisted. The Web helper test verifies committed Annotation geometry becomes a preview mark.
5. Strict workspace Clippy passed. All 151 Rust tests plus doc tests passed. Web typecheck, 25 tests,
   and the Vite production build passed; browser console inspection returned no warnings or errors.

Formal Annotation overlay status: `PASS`.

## RoboCup Ball foreground Refiner — 2026-08-28

1. `RoboCupBallForegroundRefiner` accepts only Ball bounding-box candidates, searches a bounded
   padded region, separates non-field foreground, rejects thin field-line-only axis support, and
   applies geometry/center/quality guards before rewriting the box.
2. `ball_foreground_refiner_tightens_a_coarse_box_and_ignores_a_field_line` proves a synthetic ball
   is tightened despite an intersecting white field line.
   `ball_foreground_refiner_preserves_original_box_when_evidence_is_missing` proves deterministic
   fallback with a structured Review issue.
3. Live Run `d1500707-18a8-4d30-87be-2a379e65e34f` used the saved workspace credential without
   returning or logging it. Events show `refinement_started`, original Candidate Artifact creation,
   revision-2 Refined Candidate creation, `refinement_completed`, successful validation, annotation
   commit, Artifact validation, and Artifact commit.
4. Later live candidates safely fell back because oversized/upward-biased boxes exposed unrelated
   foreground. The final implementation first resolves the dense object-height band, then computes
   horizontal support only inside that band. Replaying exact rect `[0.438, 0.335, 0.065, 0.075]`
   against the same image produced `[0.4375, 0.35714287, 0.036764707, 0.04017857]`, quality `0.544`,
   with no issue. This is deterministic pixel refinement, not a claimed SAM call.
5. RoboCup Skill tests (13 total unit/integration including 11 algorithms), the six storage RoboCup
   loop tests, and the full 153-test Rust workspace pass. Strict workspace Clippy and doc tests pass.
   Browser verification shows the successful live Run's colored `ball 95%` overlay and reports no
   console warning or error.

RoboCup Ball foreground Refiner status: `PASS`.

## RoboCup Ball SAM2 prompted Refiner — 2026-08-28

1. `RoboCupSamHttpRefiner` calls `PromptedSegmentation` over HTTP Vision Protocol v1 with the exact
   Run/Image/Task scope, an inline PNG, a typed bounding-box input Artifact, cancellation, and a
   bounded 120-second timeout.
2. The local worker loaded the official SAM2.1 Hiera Tiny checkpoint on Apple MPS. Health and
   capability endpoints reported `healthy`, `prompted_segmentation`, bounding-box Artifact input,
   and instance-mask output. The checkpoint and Python environment live under ignored
   `workspace/.annotagent`; no model binary enters Git.
3. Runtime persisted all three lineage elements for live Run
   `acc947fa-48e9-4dc8-a412-799f723004b0`: original VLM bounding-box Artifact, SAM COCO-RLE
   instance-mask Artifact, and revision-2 refined bounding-box Artifact. The Annotation provenance
   references both mask and refined box.
4. The final live input bbox `[0.44, 0.335, 0.055, 0.065]` was foreground-seeded to
   `[0.4375, 0.3549107, 0.03676471, 0.04241071]`. SAM returned 305 mask pixels at score
   `0.91769314`; the independently decoded tight bbox was
   `[0.44117647, 0.35714287, 0.03308824, 0.04464286]`. Deterministic validation accepted it with no
   issue. Review remained requested only by correction-memory policy.
5. Browser verification displayed the correct B-Human source image, one colored tight football box,
   `ball 92%`, `3 Artifacts`, and `1 Annotations`. The earlier false `ball 0%` display was fixed by
   propagating SAM confidence to the formal Annotation.
6. Unit tests cover COCO column-major RLE decoding and malformed/empty masks.
   `./scripts/acceptance.sh` passes domain/secret boundaries, formatting, strict Clippy, all
   workspace Rust tests and doc tests, Web typecheck/25 tests/build, doctor, and all offline demos.

RoboCup Ball SAM2 prompted Refiner status: `PASS`.

## Published VLM → SAM → editable Review → Commit — 2026-08-29

1. Workflow Draft `97f29188-7891-42a1-91d0-cc9b32a83178` passed static validation with no issues
   and published as immutable version 1 with content hash
   `6f730fb8dfc2228c12e574df00d9a51b8a4c7a2ddde6809822c392da101fc576`.
2. Live Run `11312a03-f9ba-402b-af0c-0e89252a4ec7` executed Qwen VLM Detection, Core Filter,
   `annotation_refiner` backed by the local SAM2.1 worker, typed DetectionSet validation, confidence
   routing, Human Review, and Commit. After acceptance, persisted checkpoint inspection reports
   `succeeded` for `image`, `detector`, `filter`, `refine_ball`, `validate_ball`, `gate`, `review`,
   and `commit`; the Run status is `completed`.
3. Live Run `52988688-84ba-4892-86e1-ef29f8c0195d` proves intermediate mask evidence no longer
   pollutes the Review queue: Run annotation inspection contains one item, kind `bounding_box`, at
   image index 4. The SAM mask remains in persisted Artifact lineage.
4. In-app browser verification shows `Edit bounding box` on the Run page and four accessible resize
   handles (`nw`, `ne`, `sw`, `se`) in Review. Review displays the exact source Run, Workflow version,
   source node, confidence, and `Accept & commit` action.
5. The Review canvas obtains natural image dimensions before drawing and converts screen pointers
   through the SVG transform, so normalized geometry remains aligned for non-1000×650 images and
   while zoomed or panned.
6. Human acceptance is idempotent. A partial failure can be retried without another annotation
   revision or correction-memory record, and the Published resume executor registers only Review
   descendants, so it never requires or calls the original live Provider.
7. `./scripts/acceptance.sh` completed successfully with strict Clippy, all workspace Rust and doc
   tests, 25 Web tests, the production build, doctor, and all three offline demos.

Published editable-review pipeline status: `PASS`.

## Multi-prompt SAM recovery for imprecise VLM boxes — 2026-08-29

1. The most recent four Review candidates contained `sam_prompted_refiner` lineage; two older
   candidates contained only `ball_foreground_refiner`. This distinguishes historical fallback
   output from the current SAM path and proves that the inaccurate latest result was not caused by
   skipping SAM.
2. For live Run `1d20cd51-3c04-4d4b-912f-e43f83e31d6a`, Qwen returned the coarse bounding box
   `[0.44, 0.41, 0.035, 0.04]`, vertically below the visible football. The updated worker returned
   multimask results for several bounded search prompts without re-encoding the image.
3. The Ball Skill accepted five plausible SAM candidates and selected prompt index 1, mask index 2,
   SAM score `0.7824226`, selection score `0.6322746`, and tight bounding box
   `[0.4375, 0.357142857, 0.038602941, 0.051339286]`. Visual comparison against the 544×448 source
   image places this box on the football rather than the field line.
4. Persisted Run inspection reports `completed_with_review`, 6/7 executed nodes, 15 intermediate
   Artifacts, and exactly one bounding-box Annotation. The 15 masks remain inspectable evidence and
   do not become duplicate Review annotations.
5. In-app browser verification displayed the corrected `ball 68%` overlay and an editable Review
   bounding box with four corner handles. The Review inspector resolved the candidate to
   `refine_ball` and displayed `SAM 2.1 multi-prompt` instead of leaving the refinement path
   ambiguous.
6. `./scripts/acceptance.sh` passed the Agent/Skill boundary check, formatting, strict workspace
   Clippy, all workspace Rust/doc tests (including the 100-image durable Batch), Web typecheck, all
   25 Web tests, production build, doctor, and all three offline demos. A focused server check also
   passed after adding Pipeline Artifact lineage reporting.

Multi-prompt SAM recovery status: `PASS`.

## Grid-assisted Qwen grounding experiment — 2026-08-29

1. Control Run `f6aa2b5b-e933-4681-9ddb-0ff646edc06a`, after fixing target-description delivery but
   without a grid, returned VLM box `[0.430, 0.375, 0.040, 0.040]` in 8,963 ms and SAM box
   `[0.439338, 0.366071, 0.034926, 0.040179]` at confidence `0.68752`.
2. Grid Run `b6f77746-8913-40f2-a49c-3ef0e9bf62ad` returned VLM box
   `[0.440, 0.340, 0.040, 0.050]`. After the ranking fix, SAM selected its 91.5% mask and produced
   `[0.441176, 0.357143, 0.033088, 0.044643]` at final confidence `0.824372`. The selected mask had
   selection score `0.785485`; all plausible candidates now retain their scores as Artifacts.
3. The first normalized-coordinate grid batch found two of three images accurately but placed the
   small ball in `color_1001525.png` one grid row too low. This disproves the hypothesis that a grid
   alone is sufficient and motivated testing Qwen's native grounding coordinate convention.
4. Final Batch `9d1a5900-ab11-46e5-b652-66c988ef9be7`, using immutable Workflow
   `83c2af7b-9ae2-4a37-b91a-8e5c47795494@v1`, produced:
   - `color_1001525.png`: VLM `[0.496,0.438,0.021,0.024]` in 2,364 ms; SAM
     `[0.496324,0.4375,0.023897,0.029018]`, confidence `0.818036`.
   - `color_289771.png`: VLM `[0.425,0.528,0.098,0.119]` in 2,668 ms; SAM
     `[0.428309,0.533482,0.099265,0.116071]`, confidence `0.928246`.
   - `color_548575.png`: VLM `[0.498,0.556,0.064,0.067]` in 3,743 ms; SAM
     `[0.509191,0.5625,0.049632,0.0625]`, confidence `0.855573`.
5. Original-image inspection and GUI overlay verification place all three boxes on their visible
   footballs. Browser verification of Run `74dc3137-cf22-4896-8c9f-c85260565592` showed the exact
   final Workflow, 6/7 nodes, 15 Artifacts, one editable `ball 82%` overlay, VLM latency 2,364 ms,
   and SAM latency 689 ms.
6. `./scripts/acceptance.sh` passes the domain boundary check, formatting, MSRV-aware strict Clippy,
   all workspace Rust/doc tests (including the durable 100-image batch), production build, all 25
   Web tests, doctor, and three offline demos. Tests explicitly cover dual-image grid preservation,
   Qwen coordinate normalization, and rejection of an oversized lower-confidence SAM distractor.

Grid-assisted Qwen grounding exploratory status: `PASS` (3/3 inspected images; no claim beyond
this bounded sample).

## Review priority rendering and API latency — 2026-08-29

1. Repeated live measurements before the fix showed `/api/projects` at 5.08–5.31 seconds and
   `/api/reviews` at 5.17 seconds. `/api/projects/robocup-ball/images` and the 358 KB image content
   endpoint each completed in roughly 1 ms, isolating the bottleneck to Review aggregation.
2. The Dashboard now obtains `review_queue` through one direct `review_status` count. Five live
   post-fix requests completed in 81–102 ms, and a storage test covers the count semantics.
3. Full Review aggregation now filters Runs without pending annotations before loading evidence,
   parses each Run snapshot directly, constructs the Project SHA-to-index map once, hashes encoded
   files without pixel decoding, and retains the legacy inspection fallback only when indexed
   metadata is unavailable. Three live requests completed in 566–590 ms.
4. `GET /api/reviews/{id}` now performs a targeted lookup. For Review
   `f28eb9bf-4ee4-4693-8dbb-32c9946d9a01`, three requests completed in 95–105 ms with the same
   source Run, Workflow, image index, refinement lineage, confidence, and validation evidence as
   the full queue response.
5. The Review frontend requests routed detail and the full queue concurrently. Browser verification
   after restarting the live 8787 service reached a decoded 544×448 image and editable overlay in
   399 ms with one priority-rendered queue item; the full 16-item queue hydrated about 0.6 seconds
   later without losing selection.
6. The first SSE open no longer triggers a duplicate Dashboard refresh; reconnects still refresh
   after the connection is re-established. Eight server tests, 30 Web unit tests, production build,
   and the full E2E suite pass (9 passed, 1 fixture-dependent test skipped).

Review priority-rendering status: `PASS`.

## Open-Vocabulary + Specialist Detection M7 — 2026-08-30

1. Core Advisor tests build two registry-valid `Suggested` Drafts without model-brand branching:
   open-vocabulary → Crop verification → Review for cold start, and specialist → bounded Recovery
   for an available model whose declared label space covers the Project Label.
2. Recovery Runtime tests prove accepted 0.93 specialist evidence makes zero fallback calls; an
   empty specialist DetectionSet makes exactly one call; a structured Domain Validation Issue also
   requests fallback; and a cost reservation above remaining budget makes zero calls and routes
   Review.
3. A low specialist score plus geometrically agreeing score-less open-vocabulary evidence changes
   the initial Evidence decision from `fallback` to `accept`. Independent score semantics and boxes
   remain in Candidate Cluster evidence and are never averaged.
4. The Recovery policy has explicit step/tool/cost budgets, maximum fallback calls, action and stop
   condition. Backend error, disabled policy, missing queries or another requested fallback preserve
   the primary result and stop at Human Review rather than entering another model loop.
5. The exact published DAG integration covers both branches. The primary fast path commits one
   annotation with `fallback_invoked=false`; an edited Mock-empty specialist Run invokes the
   registered Mock Open Vocabulary backend once and persists the review result.
6. Application history persists the Recovery `AgentSession`. Its structured steps expose reason
   codes, query IDs, model/capability, counts, timing, final decision and stop condition; serialized
   trace tests confirm no hidden chain-of-thought field, query text, image bytes or raw Worker body.
7. `cargo test --workspace --all-features` passes 216 Rust tests and doc tests. Strict workspace
   Clippy/build, Web typecheck, all 34 Web tests, production Web build, Python syntax checks,
   domain-brand boundary scan and diff secret scan pass.

Detection Advisor and bounded Recovery status: `PASS`. Real LocateAnything/RF-DETR inference
remains `LIVE-CONDITIONAL`; no API key, model weight, download or remote mutation was used.

## Open-Vocabulary + Specialist Detection M8 — 2026-08-30

1. `RoboCupBallSkill` publishes two Ball-only templates. The hybrid template has unresolved model
   bindings and generic `ObjectDetection`, `OpenVocabularyDetection`, and `Classification`
   requirements; a test rejects any concrete backend-brand string inside it.
2. Draft creation resolves those requirements from Project-owned `capability.*` configuration.
   The live example maps real Registry IDs, while the Mock example maps three in-process models and
   requires no API key or weight.
3. Exact published-Workflow execution with a 0.92 Mock specialist detection commits the candidate.
   Recovery records zero fallback calls and the Crop classifier node remains skipped.
4. The same immutable graph with an empty specialist result invokes one Mock open-vocabulary call,
   projects the Candidate Cluster without fabricating a score, validates it, creates a Crop with
   preserved parent lineage, executes crop Classification and ends `CompletedWithReview` rather
   than failing the Run.
5. Runtime tests prove a 0.76-IoU agreement can change fallback to accept and a geometry conflict
   takes the bounded verification route. The cost-budget case makes zero fallback calls. Published
   execution also proves an opted-in specialist Worker interruption becomes a structured empty
   result, invokes fallback and reaches Review without a failed Run or panic.
6. Ball validator tests cover white-shoe and penalty-mark risks and Project-aware field relation.
   The Classification verifier and Published Workflow prove `not_football` takes the explicit
   reject route, retains the rejected Artifact and never executes Commit while preserving the
   original crop subject reference. Published validation reads Correction Memory only from the
   exact Project/Skill/task/Label scope.
7. `examples/robocup-ball-hybrid-mock/scenarios.yaml` records high specialist, empty fallback,
   agreement, geometry conflict, white shoe, insufficient budget and Worker crash cases. This is
   deterministic Mock/contract evidence and does not claim live model quality.
8. `cargo test --workspace --all-features` passes 219 Rust tests and all doc tests. Strict Clippy,
   workspace build, formatting, all 34 Web tests, Web typecheck/build, diff checks and a zero-hit
   Core/Runtime brand/domain scan pass.

RoboCup Ball capability-bound hybrid status: `PASS`. Real LocateAnything/RF-DETR inference remains
`LIVE-CONDITIONAL`; no push, API key, model weight, download or remote mutation was used.

## Open-Vocabulary + Specialist Detection M9 — 2026-08-31

1. New Project queries the real Model Registry and recommends a label-space-compatible specialist
   first or an open-vocabulary cold-start recipe when no specialist exists. Both routes still become
   editable Advisor Drafts; unavailable live bindings are explained rather than simulated.
2. Detection Worker Settings now supports an arbitrary local collection and persists capability,
   endpoint, registry/model identity, version, score semantics, request cost, timeout, checkpoint,
   label-space, license and remote-opt-in facts. Models exposes matching metadata and real health /
   capability actions.
3. Run Results, Debug and Dry Run summaries show fallback/cache counts and source evidence. A
   score-less source is rendered as `confidence not provided`; multi-source agreement retains IoU
   and each original box rather than manufacturing an aggregate score.
4. Review API returns structured queue explanations plus source Detection Evidence. Choosing a source
   box changes only the editable revision until Save/Accept and persists the selected evidence in
   annotation attributes so Correction Memory can retain the reviewer decision.
5. TUI renders model availability and score semantics and provides real model-test, Artifact and
   cache-aware Replay commands alongside existing lifecycle controls.
6. In-app browser verification against a locally rebuilt server covered Models, Settings, New
   Project, Results, Debug and Review at 1024px and 760px. There was no horizontal overflow; the
   exact Debug query restored after reload; source-box selection created an unsaved edit and reload
   correctly discarded it. Native 200% zoom remains an explicit manual M10 check.
7. `cargo test --workspace --all-features` passes 220 Rust tests and doc tests. Strict workspace
   Clippy, formatting, Web typecheck, all 35 Web tests, production build and diff checks pass.

Guided mixed-detector evidence UX status: `PASS`. No source box, confidence, model availability or
live inference result was fabricated; real GPU inference remains `LIVE-CONDITIONAL`.

## Open-Vocabulary + Specialist Detection M10 — 2026-08-31

1. `detector_cache_is_model_query_mapping_and_config_aware` executes the same specialist and
   open-vocabulary inputs twice and observes one backend call plus `Cached` traces. A Gate-only edit
   preserves both caches; a grounding query edit reruns only open vocabulary; specialist model
   version, checkpoint SHA-256, backend protocol and class mapping changes each invalidate the
   specialist key.
2. The Cache Key is explicit production material: canonical image content reference and dimensions,
   input Artifacts, model ID/version/checkpoint/protocol/configuration, node-configuration hash,
   queries, Project Label mapping, target labels and enabled Skills. Product detector runners are
   cacheable; evidence-dependent Recovery remains non-cacheable.
3. The exact RoboCup hybrid Run is sandbox-Replayed from `commit_evidence`. Replay reports
   specialist, primary validation and Recovery as preserved upstream nodes, re-executes only Commit
   in the sandbox and leaves the persisted Annotation count exactly one.
4. The durable 100-image test now publishes and pins
   `robocup.ball.specialist_with_open_vocab_fallback`, pauses while work is active, closes and
   rebuilds Application state, resumes to all 100 images, and verifies 100 unique child Runs, exact
   published content hashes and exact image-budget accounting.
5. The versioned Worker contract suite passes malformed/oversized response, invalid coordinate,
   missing-score, timeout, runtime cancellation and forwarded cancel cases. Separate no-weight
   LocateAnything and RF-DETR processes returned `unavailable` health plus truthful discoverable
   capabilities; Python 3.14 parsed all four tracked Workers.
6. The full Chromium suite passes 24/24 scenarios. It renders independent RF-DETR and
   LocateAnything evidence, agreement, geometry conflict and no-score wording; Review can adopt
   either source box through a normal unsaved revision; Models exposes unavailable and timeout;
   Generic routes contain no RoboCup copy; URL restore, keyboard controls and compact reflow pass.
   The in-app browser independently confirmed the real Models page and no 1024px overflow. Native
   browser 200% zoom remains `MANUAL`.
7. Server Review now loads persisted Validation Issues, correlates them by Annotation ID and renders
   the Skill-owned message without matching a domain-specific code/string in the generic layer.
   The final boundary script scans production sources while deliberately excluding cfg(test)
   concrete-label fixtures; the production scan is clean.
8. Release commands pass: `cargo fmt --all -- --check`, strict workspace Clippy, all 221 Rust tests
   plus doc tests, workspace build, 35 Web unit tests, TypeScript, production Web build, doctor and
   all three offline demos. Core/generic Runtime model/domain scan, repository secret scan and
   `git diff --check` return no findings.
9. Added `SPECIALIST_DETECTION.md`, `MODEL_LICENSE_METADATA.md`,
   `HYBRID_DETECTION_WORKFLOWS.md`, and `DEMO_HYBRID_DETECTION.md`; required architecture,
   product, evidence, guidance, RoboCup and limitations documentation was updated.
10. Detection acceptance is 88 `PASS`, zero `OPEN`, one `LIVE-CONDITIONAL`. Real five-image
   LocateAnything/RF-DETR GPU smokes were not run: the host is Darwin arm64 without NVIDIA runtime,
   configured local model paths, or a versioned specialist checkpoint and legal weight metadata.
   No Mock output is presented as real model accuracy, and no key, weight, download, push or remote
   mutation was used.

Open-Vocabulary + Specialist Detection Alpha repository release status: `PASS`. External real-model
accuracy remains `LIVE-CONDITIONAL`; native 200% browser zoom remains `MANUAL`.

## Provider Registry + Pipeline Builder Alpha M8 — 2026-08-31

1. Migration 9 and the explicit Registry import API/UI convert the compatibility Provider, model
   string and default Project bindings in one rollback-safe transaction. Repeated apply is a no-op;
   collisions roll back; existing user bindings win. The import never reads, copies or deletes a
   secret and reports zero historical Run changes.
2. Publication freezes exact Model Profile revisions plus Provider adapter, endpoint, remote model,
   capabilities, limits and generation defaults into the immutable Workflow hash. Credentials and
   prices stay outside the semantic snapshot. Disabling the current Provider blocks a new Run but
   does not rewrite the Published Version or Run history.
3. Draft Dry Run resolves the Registry Provider type and SecretStore credential and executes from
   the same frozen Profile snapshots. A Server regression proves a session-only credential reaches
   Runtime construction without appearing in serialized Provider data.
4. Published Runtime integration executes two Classification nodes of the same operation with two
   distinct frozen Model Profiles on one Provider and records the correct model for each node.
   Workflows spanning multiple Provider credentials fail closed and are documented as a limitation.
5. Provider Registry Chromium coverage proves protected deletion, disable removal from compatible
   results, health restoration, session key rotation without echo, explicit idempotent migration and
   responsive Settings. The complete 31-test suite also covers Generic Project, Published Run,
   Artifact, Replay, Review, Export, keyboard recovery and compact layouts.
6. Release commands pass: `cargo fmt --all --check`, strict workspace/all-target/all-feature Clippy,
   all-feature build, 275 Rust tests plus doc tests (one explicitly billable smoke ignored), Web
   typecheck, 38 Vitest tests, production build and 31 Chromium tests. API-key-shaped source scan,
   browser-storage inspection and `git diff --check` have no secret or formatting finding.

Provider Registry + Pipeline Builder Alpha offline release status: `PASS`. Real Qwen/OpenAI/
OpenRouter/Gemini-compatible calls, external network behavior and native unlocked system Keyring
remain `LIVE-CONDITIONAL`; no credential, push or remote mutation was used.

## Expert Vision SDK M3 — 2026-09-01

1. Typed Box/Point Prompt, Mask and Polygon Artifacts validate exact set/item lineage.
2. The capability-neutral Conversion Registry finds the explicit Detection → Box Prompt → Prompted
   Segmentation → Mask → Detection path only when every executable node is registered.
3. `published_prompted_segmentation_pipeline_runs_end_to_end_offline` publishes and executes that
   complete chain through the real application Runtime; the refined box reaches human review while
   retaining the original Detection, prompt and mask evidence.
4. `cargo fmt --all -- --check`, strict all-target/all-feature Clippy and the complete 291-test Rust
   workspace pass; one explicitly billable Provider smoke remains ignored.
5. Python Worker SDK tests pass 14/14. Web typecheck, 40 tests and production build pass.

## Expert Vision SDK M4 — 2026-09-01

1. Core tests cover all nine `AnnotationFailureClass` values and prove that Provider failure,
   no-candidate and geometry error remain distinct.
2. Vision-language Detection geometry defaults to `coarse_hypothesis`; semantic confidence remains
   a separate score field. Refiner metrics compare original and mask-refined boxes without replacing
   either Artifact.
3. Generic Detection Dry Run exposes per-result geometry reports plus the aggregate quality/failure
   summary through the Application path. Deterministic variants prove Provider failure is not
   counted as no-candidate, while a successful terminal empty DetectionSet is.
4. The HTTP review regression patches an editable bbox, receives geometry metrics, resolves review,
   and finds manual center-shift and area-change evidence in persisted Correction Memory.
5. `cargo test --workspace --all-features`: 294 passed, zero failed, one opt-in billable smoke
   ignored. Strict Clippy/Rustfmt, Web typecheck and all 40 Web tests pass.
## Registry-only execution admission — 2026-08-31

1. Settings no longer renders `Legacy Run fallback`; New Project creates only Project Schema plus an
   editable Draft and directs Model binding to Automation/Registry.
2. Compatibility Project task graphs are no longer marked Published/default, so readiness and
   Guidance block Run until a real immutable version exists.
3. Run and Batch request DTOs require `workflow_id` plus `version`, reject the former `provider`
   override, and resolve credentials exclusively through frozen Model Profile snapshots.
4. Model-bearing Draft Dry Run and publication fail with an actionable binding error when no
   Registry Model Profile is frozen; model-free Core workflows remain deterministic and offline.
5. Empty workspaces receive a formal built-in Mock Provider plus capability-specific Model Profiles.
   They are resolved and frozen like every other Registry entry; no runtime reads the old Settings
   Provider as an execution fallback.
6. `formal_execution_rejects_legacy_provider_fallback_requests` proves the HTTP boundary rejects
   provider overrides and missing versions. The Web API test proves Run and Batch serialize only the
   exact Published Workflow ID/version.
7. Release verification passes strict workspace/all-target/all-feature Clippy, all-feature Rust and
   doc tests (one explicitly billable smoke ignored), 39 Vitest tests, production Web build and all
   31 Chromium scenarios. `git diff --check` is clean.
8. Local build cleanup removed 893,011 reproducible files (158.0 GiB logical size). Dev/test profiles
   now disable incremental compilation and use bounded debug symbols; a clean all-feature test build
   occupies about 3.5 GiB instead of retaining the former unbounded cache.

Registry-only execution admission status: `PASS`.

## Persistent workspace Provider credentials — 2026-08-31

1. `WorkspaceFileSecretStore` writes a Provider-scoped secret atomically to
   `.annotagent/credentials/registry-provider-<id>.key`, rejects traversal and symbolic links, and
   enforces owner-only directory/file permissions on Unix.
2. `workspace_file_registry_credential_survives_server_restart` creates a Provider through HTTP,
   saves a write-only workspace credential, destroys and reconstructs Server state, resolves the
   same credential and verifies that neither save nor read APIs expose its value.
3. The same regression proves an expired `session_only` reference reports `credential_configured`
   as `false`, resets stale health to `unknown`, and returns a repair message that explains the restart
   lifecycle and points to Local workspace file.
4. Provider setup defaults to Local workspace file. Environment variable, temporary session and
   opt-in system credential storage remain explicit alternatives; the UI explains each lifetime.
5. Strict workspace Clippy, all-feature Rust/doc tests, 40 Vitest tests, production Web build and
   Chromium Provider Registry coverage pass. Secret-shaped diff and formatting checks are clean.

Persistent workspace Provider credentials status: `PASS`.

## Expert Vision SDK M7 — guided setup and SAM evidence — 2026-09-01

1. Settings → Vision Workers presents one six-step Expert Model onboarding flow for SAM, known
   detector/segmenter presets, a generic HTTP protocol Worker and the existing offline Mock.
2. Discovery reads health, capabilities, models and contracts and persists stage-specific
   evidence. Registration remains disabled until health, protocol, contracts, immutable
   weights identity and selected-image conversion all pass.
3. Environment Worker authentication persists only the variable reference. Restart-time Manifest
   construction resolves it again and cannot retain stale `Available` state without the secret.
4. Selected-image sampling reads a real Project image and exposes input, bounded raw summary,
   typed converted Artifacts, normalized coordinates, score/geometry semantics, duration and
   warnings. SAM sampling uses the explicit Image + BoxPromptSet → MaskSet contract.
5. The reference SAM Worker preserves exact prompt set/item lineage, exposes all discovery
   resources and never downloads or claims a checkpoint. Real quality remains live-conditional.
6. Live model/version/checkpoint/license identity is authoritative. Missing identity is copied from
   discovery, conflicting local identity is rejected, and discovery is repeated before sampling.
7. Desktop and 480 × 760 Browser verification passes on the direct Vision Workers route with no
   console errors. Full verification passes 299 Rust tests, strict Clippy/Rustfmt, all 40 Web unit
   tests, the production build and 16 Python SDK/SAM tests; one billable smoke remains ignored.

## Expert Vision SDK M8 — RoboCup and release closure — 2026-09-01

1. The Release Matrix has no `PENDING` or `PARTIAL` item. Generic Projects stay independent of
   RoboCup; the Ball Skill and its public template name capabilities rather than backend brands.
2. Deterministic Rust/Agent coverage distinguishes Provider failure, no candidate, semantic/domain
   risk, missing score and geometry error. Only the geometry case with a complete conversion path
   and Available refiner produces Detection→Prompt→Mask→BBox Draft changes.
3. Specialist-first and one bounded open-vocabulary fallback remain the RoboCup execution policy.
   White-footwear risk uses semantic/Crop verification before geometry refinement and preserves
   Human Review when evidence remains unresolved.
4. The test-only multi-model HTTP fixture contains no model implementation or weights and labels
   every response as fixture evidence. Chromium nevertheless exercises the real Server protocol,
   Settings persistence and typed Artifact conversion for Generic, SAM, YOLO, RF-DETR and
   LocateAnything registration.
5. All 34 Chromium journeys pass, including MissingWeights/discovery failure, selected-image SAM
   MaskSet conversion, model identity, Run, Review, Replay, Export, Provider Registry,
   accessibility/responsive behavior and Generic isolation.
6. `scripts/acceptance.sh` passes Agent/Skill and Core model/domain boundary scans, secret-prefix
   scan, Rustfmt, strict Clippy, 299 Rust tests, all-feature build, Web typecheck/40 tests/build,
   doctor and four offline demos. Python SDK/SAM tests pass 16/16 and all Worker files compile.
7. All required documentation exists. Real SAM, YOLO, RF-DETR, LocateAnything, PIDNet and
   Grounding DINO accuracy remains `LIVE-CONDITIONAL` on user-supplied legal weights, dependencies
   and hardware. No credential, weight, download, push or remote change occurred.

Expert Vision SDK + Evidence-Driven Pipeline Builder Alpha offline release status: `PASS`.

## Product Mock Session Cleanup Hotfix — 2026-09-02

1. Live API inspection proves both configured Providers are OpenAI-compatible, `/api/models`
   exposes no Mock model, and all 11 RoboCup Drafts/versions contain zero `mock-*` bindings.
2. Eight pre-purge Pipeline Builder sessions still contained structured Registry observations such
   as `remote_model_id = mock-detector`; that persisted audit data was the source rendered by the
   Agent panel.
3. `purge_mock_agent_sessions` removes an Agent session only when its selected Provider, model-call
   identity or structured Tool arguments/results contain a canonical `mock`, `mock-*` or `mock_*`
   identity. A normal session whose explanatory warning says Mock models are disabled is retained.
4. The existing Registry purge test now proves fixture Registry entries, bindings, Drafts and Agent
   sessions are removed while real Agent authoring state survives. Server startup tests remain green.

## Pipeline Builder Provider Resilience Hotfix — 2026-09-01

1. Persisted session `bce120c9-629a-4575-aca3-541e91f485ec` proves configuration was valid before
   the incident: three GLM-5.2 calls succeeded and only the fourth ended after a bounded 502 retry
   sequence. No credential value was read or printed during diagnosis.
2. `PipelineBuilderModelRuntime::openai_compatible_config` now forwards both retry-delay bounds from
   the selected Provider Profile instead of using an internal fixed delay.
3. `retries_transient_gateway_failures_and_records_attempt_count` returns 502 twice, succeeds on the
   third attempt, and verifies `retry_count = 2`.
4. `exhausted_gateway_failure_is_actionable_and_hides_html` proves a repeated 502 becomes a concise
   saved-Draft recovery message and contains neither the HTML body nor the nginx banner.
5. `retry_after_and_connection_policy_bound_retry_delay` proves `Retry-After` is honored without
   exceeding the registered connection policy.
6. A non-billable live connection check returned reachable and protocol-compatible with 26 models
   at 41 ms. The affected workspace Profile was hardened from two 250 ms-start retries to four
   bounded retries with 1–5 second backoff; no secret was moved or exposed.
7. `cargo test --workspace`, strict all-target/all-feature Clippy, Rustfmt and `git diff --check`
   pass. The only ignored test remains the explicitly opt-in billable Provider smoke.

## Product Mock removal — 2026-09-01

1. Production `ServerState` migrates a legacy Mock default to `openai_compatible` and calls the
   transactional Registry purge before serving requests. The purge removes fixture Providers,
   Model Profiles, Project bindings, Mock global defaults, fixture probe records and Mock-backed
   unpublished Drafts while preserving immutable Run snapshots.
2. The Provider presets API returns only OpenAI-compatible live integrations. Provider creation and
   mutation reject the Mock adapter, and the Web UI contains no Mock Provider, Advisor or Expert
   Worker choice.
3. Workflow suggestion defaults exclusively to `llm`. `advisor=mock` returns HTTP 400 in the
   production binary, and a second boundary rejects any generated, saved or published Draft whose
   executable model binding starts with `mock`.
4. Controlled Label Pipeline generation no longer writes `mock_label`, `mock_class_id`,
   `mock-classifier` or `mock-detector` fallbacks. Missing models stay unresolved until the Agent or
   user binds a real Registry Model/Worker.
5. Published runtime fails closed for missing classification, detection, Grounding and prompted
   segmentation bindings. Legacy YOLO fixture nodes cannot execute as product Workflows; real
   specialist inference uses the versioned HTTP Vision Worker path.
6. `purging_mock_registry_removes_active_bindings_defaults_and_fixture_drafts` proves durable
   cleanup. `no-mock-product.spec.ts` proves an empty production workspace exposes no fixture
   Provider, all presets are live adapters, Mock Advisor requests are rejected and the Provider UI
   offers exactly one live adapter type.
7. Verification: `cargo test --workspace` passes 300 tests with one explicitly billable smoke
   ignored; Vitest passes 40/40; TypeScript and production Vite build pass; the focused Chromium
   regression passes; the restarted live workspace lists only configured Qwen and GLM Provider
   profiles and contains no Mock-backed unpublished Draft.

Product Mock removal status: `PASS`.

## Pipeline Builder Progress-Safety M6 — 2026-09-01

1. `pipeline_builder_baseline_reproduces_repeated_inspection_budget_exhaustion` captures the old
   failure deterministically: 48 successful read-only calls, 95,326 input tokens, no Draft and the
   generic `step or tool-call budget exhausted` outcome.
2. Persisted `PipelineBuilderPhase`, `PipelineBuilderBudget`, `PipelineBuilderOutcome`, typed stop
   reason, phase counters and finalization reserve reject phase regression and keep six calls
   available for a recoverable outcome.
3. `get_pipeline_builder_context` returns one revisioned credential-safe snapshot. Canonical
   observation keys include Tool, arguments, context revision and Draft revision; the regression
   performs only two underlying resource reads, records cache reuse and blocks repeated inspection.
4. Deterministic feasibility returns Runnable, degraded, blocked or unsupported. Missing Detection
   capability persists a typed blocked Draft in four calls; static validation blocks Dry Run and
   Publish until the unresolved Model requirement is fixed.
5. The original inspection fixture now finishes in eight calls and two model turns with
   `ProviderSetupRequired`, one editable blocked Draft and 27,236 input tokens—a 71.4% reduction.
6. Retry creates a new Agent Session and resets budgets/cache counters while preserving the exact
   Draft ID and unresolved requirements; the recovery fixture reaches the same blocked Draft in
   three calls.
7. `model_profile_satisfies_node_contract` accepts a Qwen-style VLM for structured VLM Detection
   only when image and structured-response requirements are met. It remains incompatible with
   native Object Detection unless that separate capability is declared.
8. The live Builder prompt exposes only phase-valid actions and a compact context digest. Broad
   catalog inspection disappears near the finalization reserve, and creation tools disappear after
   Drafting so a model cannot restart the Draft loop during validation.
9. HTTP/API serialization and the GUI expose phase, outcome, budget, reserve, duplicate/cache
   counts, Draft identity, unresolved bindings and next action. Recovery links open the Draft,
   Provider or Model settings, and retry from persisted state.
10. The deterministic local OpenAI-compatible browser fixture exercises real HTTP Provider,
    Profile probe, Builder tool protocol, Classification, Qwen-coordinate VLM Detection and typed
    subject/parent references. It is explicitly a protocol fixture, not model-quality evidence.
11. Browser coverage proves structured Draft Diff/Undo, blocked-Draft recovery navigation, Sample
    Test, immutable publication, Run, Review, Replay, VLM Detection → Filter → Core Crop, Provider
    coexistence and responsive one-primary-action behavior. All 35 Chromium scenarios pass.
12. Release commands pass: `scripts/acceptance.sh`, Rustfmt, strict workspace/all-target/all-feature
    Clippy, 304 Rust tests plus doc tests, all-feature build, Web typecheck, 40 Vitest tests,
    production build, doctor and four offline demos. `git diff --check` is clean.
13. The opt-in `real_openai_compatible_pipeline_builder_smoke_when_explicitly_enabled` remains
    ignored because no separately authorized legal credential was provided for this release run.
    External Provider behavior is therefore `LIVE-CONDITIONAL`; no conversation credential was
    read, stored or sent.
14. Milestone commits are `3b3dd63` (baseline), `6a89d40` (phases/budgets), `f19b01e`
    (context/cache), `eae15e8` (feasibility/blocked Draft), `87597d7` (VLM Detection semantics), and
    `1563fa6` (prompt/API/UI/retry). M6 release validation is recorded by the following local commit.

Pipeline Builder Progress-Safety offline release status: `PASS`. No push or remote mutation was
performed.
