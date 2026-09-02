# Real Prompted-Segmentation Delivery Alpha Status

Last updated: 2026-09-03 CST

## Current Milestone

M0 — baseline and missing-real-model regression complete.

## Current candidate

EfficientSAM-Ti split ONNX, pending graph inspection and candidate decision in M1.

## Candidate eliminations

- None yet. Existing notes are prior research and do not count as this task's completed audit.

## Completed

- Saved the governing prompt byte-for-byte as `REAL_MODEL_DELIVERY_MASTER_PROMPT.md`.
- Recorded the initial Git state: `main`, clean before task edits, 40 commits ahead of `origin/main`.
- Recorded the host: macOS 26.3 build 25D125, Darwin ARM64 (`arm64`).
- Reproduced the product gap: the only Catalog entry is
  `org.annotagent.models.fixture-prompted-segmentation@1.1.0`, marked `fixture=true` and
  `publishable=false`.
- Confirmed installed `org.annotagent.sam-onnx@1.1.0` requires `image_encoder` and `mask_decoder`.
- Confirmed the installed Fixture Model Instance is `Ready` but its Model Profile is
  `selectable=false`; no production prompted-segmentation model exists.
- Confirmed the existing `sam2.1_hiera_tiny.pt` is not the two-file SAM 1 ONNX Contract exposed by
  the installed Rust Plugin.
- Ran the initial full Rust baseline. It found one stale Fixture-version assertion in Application
  tests (`1.0.0` after the Catalog moved to `1.1.0`); the production path was not falsely promoted.
- Corrected that baseline test to request the immutable Fixture `1.1.0` identity.
- Added a state-independent regression proving the built-in Catalog cannot satisfy real-model
  delivery: every prompted-segmentation entry is currently a non-publishable Fixture.
- Repeated the complete Rust baseline successfully. All runnable workspace tests passed; only the
  pre-existing tests that explicitly require separately supplied legal model weights remained
  ignored. The all-features workspace build also passed.
- Downloaded the two EfficientSAM-Ti candidate ONNX files to a temporary audit directory only and
  reproduced their audited sizes and SHA-256 values. No weight entered Git or the workspace.

## In progress

- Complete the M1 official-source audit and inspect the EfficientSAM-Ti graphs through the Rust
  ONNX Runtime.

## Next

Complete the three-candidate official-source audit, inspect the EfficientSAM graphs with the Rust
ONNX Runtime, and accept or reject EfficientSAM-Ti with concrete evidence.

## Latest verification

| Gate | Result |
| --- | --- |
| Rustfmt | PASS — `cargo fmt --all -- --check` |
| Initial Rust workspace tests | FAIL — 1 stale test fixture identity; 57 Application tests passed, 1 failed, 1 billable ignored before the workspace run stopped |
| Repeated Rust workspace tests | PASS — `cargo test --workspace --all-features` |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle verification | PASS for installed Fixture only; real Bundle absent |
| Real graph inspection | PENDING |
| Real inference | NOT RUN |
| Web E2E | PENDING for this task |
| Latest commit | M0 commit pending at the time of this ledger update |

## Release-blocking remainder

Every real-model acceptance item remains blocking until a non-Fixture Bundle is built, installed,
smoke-tested, selectable, exercised by a real Workflow, and recovered across restart.

## Real blocker

There is no external blocker recorded yet. A missing implementation or an unperformed audit is not
classified as an external blocker.
