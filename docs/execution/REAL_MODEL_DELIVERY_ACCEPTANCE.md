# Real Prompted-Segmentation Delivery Alpha Acceptance

Last updated: 2026-09-03 CST

## Milestone 0 evidence

- [x] Governing prompt preserved byte-for-byte.
- [x] Initial branch/history, remote, OS and architecture recorded.
- [x] Installed Plugin version and required roles inspected.
- [x] Current Catalog and Fixture Manifest inspected.
- [x] Fixture-only Model Instance shown to remain non-selectable.
- [x] Normal GUI has no production compatible model.
- [x] Existing SAM 2.1 `.pt` incompatibility with the SAM 1 ONNX Plugin recorded.
- [x] Initial full Rust baseline executed and its one stale test failure recorded.
- [x] Corrected full Rust tests and build pass.
- [x] Missing-real-model regression is automated independently from mutable workspace state.
- [x] M0 committed locally as `ccbd2f6`.

## Milestone 1 evidence

- [x] Four official candidates audited; at least three were required.
- [x] Exact official repository revisions, model source URLs and license source recorded.
- [x] Redistribution and commercial-use implications recorded without overriding license terms.
- [x] EfficientSAM-Ti asset byte sizes and SHA-256 values independently reproduced.
- [x] Both real EfficientSAM-Ti ONNX graphs load in the current Rust Runtime on macOS ARM64 CPU.
- [x] Real tensor descriptors, preprocessing, prompt labels and postprocessing contract recorded.
- [x] Existing SAM 1 Plugin incompatibility established; dedicated EfficientSAM Plugin selected.
- [x] MobileSAM and SAM 1 rejected for the normal-user path because official assets require export.
- [x] SAM 2.1 retained as Labs because its official path requires Python/PyTorch.
- [ ] M1 committed locally.

## Release matrix

### A. Real model

- [ ] A non-Fixture prompted-segmentation Bundle exists outside Git.
- [x] Official/audited source and exact licenses are recorded.
- [x] Rust Runtime loads the real graph on macOS ARM64.
- [ ] Real box-prompt inference produces a non-empty finite mask.
- [ ] Mask-to-BBox produces valid refined geometry.
- [ ] Report records all identities, digests, platform, provider, prompt, mask and duration.

### B. User installation

- [ ] User needs no Python, conversion, raw ONNX search, or multi-file upload.
- [ ] `Install compatible model` lists a clearly non-Fixture model.
- [ ] Source, license, size, digest, platform and provider are visible before mutation.
- [ ] Install verifies Bundle/files/graph, runs real smoke, and persists `Ready`.
- [ ] Restart preserves the Ready Model Instance.

### C. Pipeline

- [ ] Pipeline Builder initially blocks safely and can retry the same saved Draft after install.
- [ ] Runtime executes Prompted Segmentation → Mask-to-BBox → Geometry Safety.
- [ ] Debug, Review and Replay preserve complete real-model lineage.
- [ ] Published Workflow freezes exact Plugin/Bundle/file/Contract/instance/provider identity.

### D. Truthfulness and Rust-only path

- [x] Fixture and publishable status are separate in current schemas and selector logic.
- [x] Accepted model and Plugin naming accurately describe the model family.
- [x] SAM 2 remains Labs unless a verified Rust Bundle exists.
- [ ] Active install, smoke, run and replay process trees contain no Python.

### E. Regression

- [ ] Full Rustfmt, Clippy, workspace tests and build pass.
- [ ] Full Web typecheck, unit, E2E and build pass.
- [ ] Provider, Workflow validator, Geometry, batch, lifecycle, Review, Replay and Export pass.
