# Real Prompted-Segmentation Delivery Alpha Status

Last updated: 2026-09-03 CST

## Current Milestone

M2 — controlled Model Supply Recipe and trusted local Catalog complete.

## Current candidate

EfficientSAM-Ti split ONNX, accepted for delivery but not yet Supported.

## Candidate eliminations

- MobileSAM ViT-T: official normal-user path requires a PyTorch checkpoint and Python ONNX export;
  no immutable complete split ONNX package was established.
- Meta SAM 1 ViT-B: official ONNX export covers the prompt/mask decoder but does not distribute the
  complete encoder/decoder pair needed for Rust-only installation.
- Meta SAM 2.1 Tiny: official runtime requires Python/PyTorch and no fixed complete ONNX release was
  established; retained as Labs.

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
- Audited four official model candidates, their exact sources, licenses, formats, contracts,
  redistribution considerations and platform feasibility.
- Loaded both real EfficientSAM-Ti graphs through the repository's Rust ONNX Runtime on macOS
  ARM64 CPU and recorded their actual tensor descriptors.
- Selected a dedicated `org.annotagent.efficientsam-onnx` Plugin instead of adding incompatible
  model-family branches to `org.annotagent.sam-onnx`.
- Implemented Rust-only `models recipe audit/fetch/build/verify` with deny-unknown-fields parsing,
  safe relative paths, public HTTPS validation, explicit redirect hosts, bounded downloads,
  atomic partial cleanup, exact size/digest checks and deterministic packaging.
- Added the versioned EfficientSAM-Ti Recipe, real graph Contract, preprocessing/postprocessing
  metadata, exact Apache-2.0 license, source notice, box-prompt smoke request and author-repository
  dog test image declaration.
- Built and verified the real non-Fixture Bundle at
  `dist/model-catalog/bundles/efficientsam-ti-onnx-1.0.0.annotmodel` without adding model bytes to
  Git. Size: 38,577,735 bytes; SHA-256:
  `3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c`.
- Rebuilt the Bundle and reproduced the same digest.
- Implemented and tested explicit trusted-local Catalog registration, persistence, local Bundle
  confinement, transactional refresh, and tamper rejection.
- Generated `dist/model-catalog/catalog.json`, Catalog entry and verification report, then added
  `org.annotagent.catalog.local-models` to the current workspace. Search now returns the real
  `fixture=false`, `publishable=true` model as well as the clearly separate Fixture.

## In progress

- Implement the dedicated EfficientSAM Rust Plugin and real box-prompt smoke inference in M3.

## Next

Package and install `org.annotagent.efficientsam-onnx@1.0.0`, bind the real Bundle, inspect its
graphs, run the Bundle smoke request, require a non-empty finite mask, derive a valid tight bbox,
and record execution identity and timing.

## Latest verification

| Gate | Result |
| --- | --- |
| Rustfmt | PASS — `cargo fmt --all -- --check` |
| Initial Rust workspace tests | FAIL — 1 stale test fixture identity; 57 Application tests passed, 1 failed, 1 billable ignored before the workspace run stopped |
| Repeated Rust workspace tests | PASS — `cargo test --workspace --all-features` |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle verification | PASS — real non-Fixture EfficientSAM-Ti Bundle built and verified outside Git |
| Real graph inspection | PASS — encoder and decoder loaded with ORT CPU; descriptors recorded |
| Real inference | NOT RUN |
| Web E2E | PENDING for this task |
| Model Recipe | PASS — 3 downloads / 41,814,211 bytes and 8 static files verified |
| Real Bundle | PASS — 38,577,735 bytes / `3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c` |
| Local Catalog | PASS — persisted `trusted_local_catalog`, real entry searchable after refresh |
| Latest focused tests | PASS — 14 model-catalog tests and strict Clippy for Catalog/CLI |
| Latest Rust workspace tests | PASS — all runnable tests; only explicitly external/billable tests ignored |
| Latest Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Latest commit | `fb22f8d` — M1; M2 commit pending |

## Release-blocking remainder

Every real-model acceptance item remains blocking until a non-Fixture Bundle is built, installed,
smoke-tested, selectable, exercised by a real Workflow, and recovered across restart.

## Real blocker

There is no external blocker recorded yet. A missing implementation or an unperformed audit is not
classified as an external blocker.
