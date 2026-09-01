# Geometry Safety Status

Last updated: 2026-09-02

## Current milestone

Milestone 0 — reproduction and baseline.

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

## In progress

- Create the independent M0 baseline commit.

## Next

- Milestone 1: operation-scoped model quality contracts and conservative compatibility migration.

## Recent verification

- Rust: `cargo test -p annotagent-core` — passed (74 tests); `cargo fmt --all --check` and
  `git diff --check` passed.
- Web: not run for M0; no Web behavior changed.
- E2E: not run for M0; the baseline is a Core static-validation fixture.
- Browser: 2026-09-02 read-only inspection of all four current RoboCup Ball Run Results confirmed
  correct semantic targets with imperfect box tightness and no Crop artifact.

## Recent local commit

- Pending M0 commit.

## Release-blocking remainder

- M1 through M8 and all acceptance items remain open.

## Live-conditional items

- Real Qwen geometry comparison.
- Real SAM Worker inference.
- Specialist detector inference and weights.

## Real blockers

- No healthy prompted-segmentation Model Profile is currently registered.
- The four B-Human predictions do not yet have independent human ground-truth boxes stored as a
  calibration/evaluation set.
