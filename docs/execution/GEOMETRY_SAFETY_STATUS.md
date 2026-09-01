# Geometry Safety Status

Last updated: 2026-09-02

## Current milestone

Milestone 1 — quality semantics and Model Capability Quality Contracts (implementation and
verification complete; commit in progress).

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

## In progress

- Create the independent M1 commit, then begin M2 policy enforcement.

## Next

- Milestone 2: Project Geometry Policy, static geometry-safe publication and legacy run guards.

## Recent verification

- Rust: `cargo test -p annotagent-core -p annotagent-provider -p annotagent-server` — passed
  (79 Core, 44 Provider, 17 Server tests).
- Workspace: `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- Formatting: `cargo fmt --all --check` and `git diff --check` — passed.
- Web: not run for M0; no Web behavior changed.
- E2E: not run for M0; the baseline is a Core static-validation fixture.
- Browser: 2026-09-02 read-only inspection of all four current RoboCup Ball Run Results confirmed
  correct semantic targets with imperfect box tightness and no Crop artifact.

## Recent local commit

- `93882e7 test(geometry): reproduce unsafe vlm bbox auto-acceptance` (M0).

## Release-blocking remainder

- M2 through M8 remain open. M1 acceptance evidence is implemented and verified.

## Live-conditional items

- Real Qwen geometry comparison.
- Real SAM Worker inference.
- Specialist detector inference and weights.

## Real blockers

- No healthy prompted-segmentation Model Profile is currently registered.
- The four B-Human predictions do not yet have independent human ground-truth boxes stored as a
  calibration/evaluation set.
