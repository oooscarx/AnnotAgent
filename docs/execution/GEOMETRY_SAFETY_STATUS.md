# Geometry Safety Status

Last updated: 2026-09-02

## Current milestone

Milestone 2 — static geometry safety (implementation and verification complete; commit in
progress).

## Completed

- Read the master prompt to EOF and saved it in the repository.
- Verified `main` is eight commits ahead of `origin/main` before geometry-safety work began.
- Confirmed the active RoboCup Ball Workflow is `RoboCup Ball · VLM bootstrap@v1`.
- Confirmed its executable path is VLM Detection → label filtering → static domain validation →
  generic Confidence Gate → Commit, with Review only on the low-score route.
- Added a deterministic Core fixture containing semantic score `0.99`, coarse geometry, a loose
  predicted box and a tighter human reference.
- Reproduced the legacy behavior: the existing static validator accepts that unsafe graph because
  any upstream Validator currently satisfies `validate_commit_safety`.
- Added explicit semantic-confidence and detection-confidence score semantics.
- Added separate geometry calibration state, validation state, quality-score source and geometry
  quality reference without inventing localization scores.
- Added operation-scoped `ModelCapabilityQualityContract` with conservative VLM, specialist and
  prompted-segmentation defaults.
- Frozen Model Profile snapshots now include effective quality contracts; legacy profile JSON
  without the field migrates conservatively.
- New OpenAI-compatible VLM Detection artifacts now preserve `VisionLanguage`,
  `SemanticConfidence` and `CoarseHypothesis` instead of masquerading as specialist detections.
- Added the read-only Model Profile quality-contract API and user-declared override input; server
  binds overrides to the actual Model Profile revision and marks their source truthfully.
- Added conservative Project Geometry Policy defaults: bounding-box annotation requires training
  quality and uses Refiner-or-Review acceptance.
- Geometry-aware Core validation now follows every candidate-source-to-Commit path and rejects
  score-only acceptance of coarse or uncalibrated predicted geometry.
- Mandatory Human Review and an available prompted-refinement plus mask-to-bbox chain are legal;
  missing and stale calibration have distinct blocking codes.
- `allow_unvalidated_commit` cannot bypass geometry policy.
- New bbox suggestions and RoboCup Ball templates route uncalibrated geometry through mandatory
  Review; classification behavior remains unchanged.
- Published snapshots record safety compatibility. Legacy snapshots remain immutable, are
  re-assessed before a new formal Run, and unsafe versions fail with `unsafe_legacy_workflow`.
- Added `Create geometry-safe Draft`, which clones a historical version and inserts mandatory
  Review boundaries without editing the original version.

## In progress

- Create the independent M2 local commit, then begin structured Review geometry evidence.

## Next

- Milestone 3: structured Review geometry feedback, reports and correction lineage.

## Recent verification

- Rust: `cargo test -p annotagent-core -p annotagent-skill-robocup -p annotagent-server` — passed
  (83 Core, 17 RoboCup including integration tests, 17 Server tests).
- Application: all 54 active tests passed; the billable provider smoke test remains explicitly
  ignored.
- The 100-image pause/restart/resume regression passed after moving that control-plane fixture to
  a non-geometric classification Project.
- Workspace: full Rust tests, all-feature build, all-target/all-feature Clippy with warnings denied,
  formatting and diff checks passed.
- Web: TypeScript, 41 unit tests, and production build passed.
- Web: not run for M0; no Web behavior changed.
- E2E: not run for M0; the baseline is a Core static-validation fixture.
- Browser: 2026-09-02 read-only inspection of all four current RoboCup Ball Run Results confirmed
  correct semantic targets with imperfect box tightness and no Crop artifact.

## Recent local commit

- `93882e7 test(geometry): reproduce unsafe vlm bbox auto-acceptance` (M0).
- `291f20c feat(models): separate semantic confidence from geometry quality` (M1).

## Release-blocking remainder

- M3 through M8 remain open. M2 acceptance evidence is implemented and verified.

## Live-conditional items

- Real Qwen geometry comparison.
- Real SAM Worker inference.
- Specialist detector inference and weights.

## Real blockers

- No healthy prompted-segmentation Model Profile is currently registered.
- The four B-Human predictions do not yet have independent human ground-truth boxes stored as a
  calibration/evaluation set.
