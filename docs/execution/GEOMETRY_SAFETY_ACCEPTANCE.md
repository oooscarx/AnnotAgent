# Geometry Safety Acceptance

Status values: `open`, `implemented`, `verified`, `live-conditional`.

| Case | Status | Current evidence |
|---|---|---|
| 1. High semantic score cannot pass geometry safety | verified | Core regression now rejects the exact M0 graph with semantic-score and uncalibrated-geometry blockers. |
| 2. VLM plus mandatory Review is legal | verified | Core dominance-path test and published Runtime regressions pass. |
| 3. Healthy SAM refinement path | live-conditional | Typed available-refiner path and offline prompted-segmentation Runtime pass; a real SAM Worker is not configured. |
| 4. Unavailable SAM becomes setup alternative | open | Builder has availability concepts; exact safe fallback test pending. |
| 5. Provider failure does not suggest SAM | open | Failure classifier exists; Agent behavior test pending. |
| 6. No candidate does not suggest SAM | open | Failure classifier exists; Agent behavior test pending. |
| 7. Wrong object does not use SAM as primary fix | open | Domain guidance exists; Agent behavior test pending. |
| 8. Bbox edit creates geometry evidence | verified | HTTP Review regression persists typed report/evidence records with original/corrected geometry, five requested metrics, reason and lineage, then reads them through Project and Run APIs. |
| 9. Calibration can pass with sufficient evidence | verified | Core aggregation and an Application end-to-end fixture build a `Passed` report from exact reviewed Run/model/node evidence under Project thresholds. |
| 10. Configuration changes stale calibration | verified | Core exhaustively classifies staleness dimensions; the Application fixture changes grid/node configuration and observes effective `Stale`. |
| 11. Qwen-only first Draft requires Review | implemented | Deterministic bbox drafts and the VLM bootstrap template now make Review mandatory; LLM Builder policy is M5. |
| 12. Registered SAM enables improvement Draft | open | Not implemented end to end. |
| 13. No measured improvement means no recommendation | open | Comparison tools exist in partial form; holdout rule pending. |
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
- Separate tests prove mandatory Review and an available
  Detection → Box Prompt → Prompted Segmentation → Mask to BBox path are legal.
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
