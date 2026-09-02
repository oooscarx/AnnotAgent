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
- [x] M1 committed locally as `fb22f8d`.

## Milestone 2 evidence

- [x] Controlled schema-1 Model Supply Recipe added for EfficientSAM-Ti.
- [x] Recipe schema rejects commands, unknown fields, unsafe paths and non-HTTPS sources.
- [x] Rust audit/fetch/build/verify commands implemented.
- [x] Redirects are disabled by default and bounded to explicit HTTPS host allowlists with public
  DNS revalidation.
- [x] Every download and static payload is checked against fixed size and SHA-256.
- [x] Download failure/cancellation removes its partial file; cached mismatches fail closed.
- [x] Recipe cannot execute shell, Python, package managers, Git or downloaded programs.
- [x] Real non-Fixture `.annotmodel` built outside Git and independently verified.
- [x] Deterministic rebuild reproduced the same Bundle SHA-256.
- [x] Trusted local Catalog add/list/refresh and restart persistence implemented.
- [x] Local Catalog validates every Bundle and confines lookup to its explicit `bundles/` root.
- [x] Tampered local Bundle causes refresh to fail without replacing stored metadata.
- [x] Generated Catalog search exposes the real publishable model separately from Fixture.
- [ ] M2 committed locally.

## Release matrix

### A. Real model

- [x] A non-Fixture prompted-segmentation Bundle exists outside Git.
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
