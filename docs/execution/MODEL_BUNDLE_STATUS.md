# Model Bundle Provisioning Alpha Status

Last updated: 2026-09-02 CST

## Current Milestone

M0 — baseline and current SAM provisioning gap.

## Completed

- Preserved the governing prompt and established the milestone/evidence ledgers.
- Re-ran the all-feature workspace Rust test and build baselines successfully.
- Confirmed `.annotplugin` is a deterministic safe ZIP, while no `.annotmodel` entity exists.
- Confirmed the SAM package declares two components, enters `NeedsWeights`, exposes a
  `MissingWeights` model and cannot be selected or smoke-tested before both files are uploaded.
- Confirmed the primary GUI path is Settings → Expert Model Plugins → installed SAM → Model Setup,
  where two raw file controls currently expose `image_encoder` and `mask_decoder` provisioning.
- Confirmed plugin operational state lives in `.annotagent/plugins/plugin-registry.json`; migration
  13 creates plugin audit tables in SQLite, while normal Model Profiles and Published Workflow
  snapshots use the existing SQLite model/workflow schema.
- Added an explicit regression assertion for both required SAM components and the unprovisioned
  state. Root `dist/` remains on disk but is ignored as generated package output.

## In progress

- None. M0 is ready for its focused verification and commit.

## Next

M1 — introduce the model-bundle API and manifest without changing Plugin Host or Provider Registry.

## Latest verification

| Gate | Result |
| --- | --- |
| Rust workspace tests | PASS — `cargo test --workspace --all-features`; 385 active, 5 explicit external/billable ignores in the established baseline |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle tests | Not applicable in M0; no Bundle API existed |
| Plugin conformance | Existing SAM registry component regression passes after M0 assertions |
| Real model smoke | Not run; no legally verified SAM assets are installed |
| Web tests | Last established release baseline: 44 unit, 37 Chromium E2E |
| E2E | M0 records the current route; installation UX changes begin in M6 |
| Local commit | Pending M0 commit |

## Release-blocking remainder

M1–M8 and every unchecked item in `MODEL_BUNDLE_ACCEPTANCE.md`.

## Live-conditional

- A legally redistributable, officially sourced prompted-segmentation ONNX bundle.
- External HTTPS curated-catalog hosting and signatures.
- GPU execution-provider coverage and non-macOS real-model smoke.
- SAM 2 remains Labs until a complete verified Rust ONNX package exists.

## Real blockers

No external model asset may be published until code license, weight license, redistribution terms,
official asset identity, ONNX tensor contract and a real Rust smoke test are all independently
verified.
