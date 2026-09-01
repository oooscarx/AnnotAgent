# Geometry Safety Acceptance

Status values: `open`, `implemented`, `verified`, `live-conditional`.

| Case | Status | Current evidence |
|---|---|---|
| 1. High semantic score cannot pass geometry safety | verified | Core regression now rejects the exact M0 graph with semantic-score and uncalibrated-geometry blockers. |
| 2. VLM plus mandatory Review is legal | verified | Core dominance-path test and published Runtime regressions pass. |
| 3. Healthy SAM refinement path | live-conditional | Complete typed Detection→Prompt→Mask→BBox→Evaluation→Decision execution passes offline; a real SAM Worker is not configured. |
| 4. Unavailable SAM becomes setup alternative | verified | Refinement Tool excludes Mock and unavailable models from runnable bindings; Qwen-only regression returns missing-weights SAM only as unapplied setup and keeps Review. |
| 5. Provider failure does not suggest SAM | verified | Builder contract and typed revision assessment classify Provider failure as infrastructure evidence and reject prompted refinement. |
| 6. No candidate does not suggest SAM | verified | Builder contract and typed revision assessment reject segmentation without a candidate prompt and direct the Agent to search/detector/Review alternatives. |
| 7. Wrong object does not use SAM as primary fix | verified | Builder contract directs semantic hard negatives to classification/domain validation/Correction Memory/Review; typed assessment does not mistake them for geometry evidence. |
| 8. Bbox edit creates geometry evidence | verified | HTTP Review regression persists typed report/evidence records with original/corrected geometry, five requested metrics, reason and lineage, then reads them through Project and Run APIs. |
| 9. Calibration can pass with sufficient evidence | verified | Core aggregation and an Application end-to-end fixture build a `Passed` report from exact reviewed Run/model/node evidence under Project thresholds. |
| 10. Configuration changes stale calibration | verified | Core exhaustively classifies staleness dimensions; the Application fixture changes grid/node configuration and observes effective `Stale`. |
| 11. Qwen-only first Draft requires Review | verified | Scripted live Builder reads all five geometry Tools, observes coarse/uncalibrated Qwen and unavailable real SAM, then saves a valid Draft with Human Review and no segment node. |
| 12. Registered SAM enables improvement Draft | live-conditional | An available prompted-segmentation binding creates a typed refinement/evaluation/decision revision and executes offline; live SAM registration/quality remains conditional. |
| 13. No measured improvement means no recommendation | verified | Typed comparison requires independent holdout, sufficient image count, recall safety, median/P10 IoU gain, lower manual adjustment and review/cost/latency/failure guards. Four images never recommend and five remain provisional. |
| 14. Legacy Workflow remains immutable and new Run is guarded | verified | Application test preserves the serialized version, blocks a new Run, creates a safe Draft, and publishes it as Safe. |
| 15. Small objects are evaluated separately | verified | Reference pixel area is bucketed using small/medium/large thresholds and each bucket retains sample count, adjustment count, mean IoU and mean center shift. |
| 16. Generic Project remains domain-neutral | verified | Generic Core/Application workflows pass without RoboCup content. |

## M0 evidence

- Historical fixture commit: `93882e7`; current regression:
  `workflow::tests::unsafe_vlm_semantic_score_auto_commit_is_blocked`.
- Predicted box: `[0.40, 0.40, 0.30, 0.30]`.
- Human reference: `[0.48, 0.48, 0.10, 0.10]`.
- Declared semantic score: `0.99`; geometry: `coarse_hypothesis`.
- M0 result: validation was valid and had no geometry-safety blocker. M2 retains the same graph and
  now requires four exact blocking codes.
- Verification: `cargo test -p annotagent-core` passed all 74 Core tests; formatting and diff
  whitespace checks passed.

## M1 evidence

- `ScoreSemantics` now separates semantic confidence, detection confidence, calibrated
  probability, relative confidence, ranking-only, absent and unknown scores.
- `GeometryCalibrationStatus` is independent from `GeometrySemantics`; only `Passed` permits a
  calibration-dependent score path.
- Default `vlm_detection.detect` contract: coarse hypothesis + semantic confidence +
  never-from-score-alone.
- Default specialist contract: predicted geometry + detection confidence + Project calibration.
- Default prompted-segmentation contract: refined geometry + no fabricated score + Project
  calibration.
- `DetectionQuality` keeps optional model scores separate from a report reference and validation
  state.
- Model Profile snapshots freeze effective contracts; missing legacy JSON fields derive
  conservative defaults.
- `/api/model-profiles/:modelId/quality-contracts` exposes effective revision-bound contracts.
- Provider test proves new VLM Detection output is `semantic_confidence` and `coarse_hypothesis`.

## M2 evidence

- `ProjectGeometryPolicy::conservative_default` maps bbox tasks to `TrainingBoundingBox` and
  `RefinerOrReview`.
- Static validation emits `semantic_score_used_as_geometry_evidence`,
  `uncalibrated_geometry_auto_commit`, `geometry_acceptance_path_missing`, and a calibration state
  error for the former production graph.
- Separate tests prove mandatory Review and an available Detection → Box Prompt → Prompted
  Segmentation → Mask to BBox → Geometry Evaluation → Geometry Decision path are legal. A refiner
  plus mask conversion without the measured decision remains blocked.
- Stale calibration emits `geometry_calibration_stale` rather than being silently reused.
- Draft Dry Runs keep blockers inspectable as non-blocking warnings; publication and new formal Runs
  enforce them.
- `unsafe_legacy_workflow_is_immutable_blocked_and_clonable_as_safe_draft` proves historical JSON is
  unchanged, formal execution is blocked, safe clone creation works, and the replacement snapshot
  is marked `safe`.
- Application, Core, Server, RoboCup and 100-image batch/control-plane regressions pass.

## M3 evidence

- `GeometryCorrectionReason` contains the eight common Review reasons and the four requested
  RoboCup hard-negative reasons; registered Skill taxonomies remain available as controlled
  extensions.
- `build_geometry_correction_evidence` derives IoU, normalized/pixel center shift, predicted and
  reference pixel area, area ratio, width ratio and height ratio from original/reference boxes.
- `GeometryIssueCode` is typed and includes loose/tight/shift/width/height/aspect/partial/background,
  refiner-conflict and insufficient-evidence states.
- SQLite migration 10 persists a `GeometryQualityReport` and its `GeometryCorrectionEvidence` in
  one transaction with Project, Run, image, annotation, node and model-revision indexes.
- `GET /api/projects/{project_id}/geometry-corrections` and
  `GET /api/runs/{run_id}/geometry-quality` return bounded reports, evidence and aggregate size
  buckets for Dry Run, Builder, Improve and future calibration readers.
- Legacy Runs without a frozen Model Profile are retained with an explicit
  `insufficient_evidence` issue and are ineligible for calibration rather than receiving a
  fabricated model identity.
- Core metric/bucket tests, Storage round-trip/migration tests and the complete HTTP Review edit,
  accept, persist and reread regression pass.

## M4 evidence

- `GeometryCalibrationKey` binds Project, task/Label, exact Model Profile revision, node
  definition/config hash, prompt, preprocessing, dataset profile, Label Schema and downstream
  refinement configuration. Its tests enumerate every stale dimension and prove secret material is
  absent.
- Calibration aggregation reports robust percentiles and rates rather than a single average. Zero
  evidence is Uncalibrated, insufficient evidence is Collecting/Provisional, sufficient good
  evidence is Passed and sufficient poor evidence is Failed.
- `reviewed_geometry_calibrates_only_the_exact_published_model_and_node` publishes a revisioned
  Workflow, persists an accepted annotation and small-object correction, creates a passing report,
  injects Passed into validation, then changes grid configuration and observes Stale.
- Static validation separately proves Passed calibration plus a semantic Confidence Gate remains
  blocked; only an explicit Geometry Quality Evaluation → Geometry Decision consumes calibration.
- SQLite migration 11 persists Project policies and immutable reports. HTTP tests cover Project
  policy GET/PUT, report list/detail, fail-closed stale display without a matching current Version,
  and rejected calibration creation without explicit Evidence Runs.

## M5 evidence

- The Pipeline Builder registry now has 64 bounded Tools. The five geometry-safety additions have
  explicit read-only Project/Registry permissions, cacheable observations and feasibility-only
  visibility; forbidden publish, Run, credential, shell, Python, download and arbitrary-URL tools
  remain absent.
- `inspect_model_quality_contract` returns effective operation-scoped contracts and passive
  availability without credentials. For the Qwen fixture it reports semantic confidence,
  `coarse_hypothesis`, `never_from_score_alone` and `score_only_auto_accept_allowed=false`.
- Project policy, correction-memory and calibration Tools are scoped to the requested task/Label.
  Correction output is bounded and aggregate-only; exact calibration output distinguishes Passed
  current evidence from Stale or insufficient evidence.
- `find_geometry_refinement_path` verifies both the registered DetectionSet → prompts → mask →
  DetectionSet conversion and an actually available non-Mock prompted-segmentation model. The
  current missing-weights SAM manifest is an unapplied setup alternative, not a Draft binding.
- `qwen_style_vlm_binds_structured_detection_but_not_native_detector` runs the complete controlled
  Agent sequence through validation and sandbox Dry Run. The resulting editable Draft has a Human
  Review boundary and no `capability.segment` node.
- Verification passed all 329 active Rust tests and doc tests (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt/diff checks, Web
  TypeScript, all 41 Web unit tests and the production Web build.

## M6 evidence

- `GeometryRefinementTrace` validates exact typed item references for the source detection, prompt,
  mask and refined detection. Mask-to-BBox writes the trace without replacing semantic score
  semantics or claiming calibration.
- `GeometryRefinementEvaluation` compares original/refined boxes with Project-configurable
  thresholds for IoU, center movement and area change. Its decision input is measured geometry;
  semantic confidence is never read.
- Runtime tests prove a stable tightening routes `accept`, while a stricter center-shift policy
  routes the same result to `review`. Empty/missing/malformed evaluation also fails closed.
- The public catalog exposes Geometry Evaluation and Geometry Decision, and both fresh execution
  and review/replay Published Runtime registration paths use the real Core runner.
- The Builder revision test proves an evidence-backed prompted segmenter is inserted only as
  Prompt→Mask→BBox→Evaluation→Decision and the old semantic Confidence Gate disappears.
- The full offline Application regression publishes and executes the chain, exposes node metadata
  and exact Artifact types, and suspends at its retained Human Review boundary. A real SAM Worker
  remains live-conditional and no Mock result is presented as model-quality evidence.
- Release verification passed all 332 active Rust tests and doc tests (one explicitly billable
  smoke ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt/diff checks, Web
  TypeScript, all 41 Web unit tests and the production build.

## M7 evidence

- `PipelineImprovementSession` durably binds the immutable baseline, target task/Label, diagnosis
  Runs, disjoint evaluation Runs, comparison Draft, candidate Draft, exact structured diff, static
  validation, recommendation and selected-change application state. SQLite migration 12 and the
  round-trip test preserve sessions and comparisons across restart.
- The Application classifies structured Run failures and human correction reasons before patching.
  Prompted segmentation is considered only for geometry errors with a candidate; Provider,
  no-candidate, semantic and domain failures receive distinct setup/remediation guidance and a
  conservative Review boundary.
- Baseline and candidate execute the same non-committing holdout images. The comparator derives
  semantic precision/recall, IoU and center-shift percentiles, manual-resize/loose/tight/
  no-candidate/review rates, cost, latency, failures and object-size buckets from human-accepted
  bbox references.
- Core tests prove four images cannot recommend even with many objects, five images remain
  provisional, sufficient improvement can recommend, and geometry gains cannot hide recall or
  cost regression.
- The four requested REST operations plus Project-scoped listing are registered and tested. The
  65th bounded Builder Tool, `compare_pipeline_geometry`, reads only a persisted comparison.
- The Application regression proves geometry evidence creates a scoped Patch Draft or explicit
  safe setup fallback and leaves the serialized Published Workflow unchanged. Apply-to-Draft
  requires a prior comparison and explicit selected diff IDs; no path auto-publishes.
- Release verification passed all 339 active Rust tests and doc tests (one explicitly billable
  smoke ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt/diff checks, Web
  TypeScript, all 41 Web unit tests and the production build.
