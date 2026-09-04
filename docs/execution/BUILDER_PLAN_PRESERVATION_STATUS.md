# Builder Plan Preservation Alpha Status

## Milestone 0 — regression fixture (2026-09-05)

Status: complete.

- Saved the governing task as `BUILDER_PLAN_PRESERVATION_MASTER_PROMPT.md`.
- Confirmed the live failure with the configured GLM Builder: the persisted
  `find_geometry_refinement_path` observation reported a runnable prompted-segmentation path, but
  the final Draft remained the VLM bootstrap path.
- Added the deterministic scripted regression
  `discovered_prompted_segmentation_path_is_materialized_at_discovery_limit`.
- Before the fix, its explicit ignored run fails after proving `runnable=true`; the recovered Draft
  contains only `core.image_input`, `vlm_detection.detect`, `core.filter`, `static_validator`,
  `core.confidence_gate`, `review_gate`, and `commit`.
- The loss occurs in the discovery-deadline branch of `run_workflow_advisor_loop`: it calls
  `materialize_feasibility_draft` with the precomputed safe suggestion and does not consume the
  successful conversion-path observation.
- The regression is ignored only while Milestones 1–4 introduce durable candidates and salvage. It
  must be enabled and pass before release.

### Baseline

- `cargo fmt --all --check`: pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass.
- `cargo test --workspace --all-features`: pass; five external/billable tests remain explicitly
  ignored.
- `cargo build --workspace --all-features`: pass.
- `npm --prefix web run typecheck`: pass.
- `npm --prefix web test`: pass, 61 tests.
- `npm --prefix web run build`: pass with the pre-existing bundle-size warning.
- `npm --prefix web run test:e2e`: pass, 43 Chromium scenarios.

No remote, credential, model asset, Published Workflow, historical Run, or formal annotation was
modified by this milestone.

## Remaining milestones

- Milestone 1: durable Build Mode, Working Draft, Plan Candidate, Fragment, Working Memory and API.
- Milestone 2: conversion-path blueprint and materialization.
- Milestone 3: registry-driven synthesis and deterministic ranking.
- Milestone 4: reserved-budget DraftSalvage and structured outcomes.
- Milestone 5: prompt, API, UI and build-mode product flow.
- Milestone 6: current-project real Builder validation and release regression.
