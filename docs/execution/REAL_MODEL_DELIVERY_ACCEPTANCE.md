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
- [x] M2 committed locally as `2984dce`.

## Milestone 3 evidence

- [x] Dedicated `org.annotagent.efficientsam-onnx@1.0.0` Rust Plugin implemented.
- [x] Plugin and Recipe model Contracts are byte-serialization compatible at SHA-256
  `ad3f23abcadb04561dcced33bae9cbfccbce4c13910a715fc964f1281c8f56ee`.
- [x] Real encoder and decoder descriptors are checked exactly at Plugin setup.
- [x] Real official dog image and fixed box prompt execute through the installed Plugin process.
- [x] Real MaskSet is non-empty, finite, Core-valid and references the exact input prompt item.
- [x] Real mask coverage is `0.061009`.
- [x] Core mask-to-bbox succeeds with normalized
  `xywh=[0.543843,0.384743,0.175373,0.592040]`.
- [x] Cold timing recorded: encoder 2,238 ms, decoder 51 ms, full smoke 4,532 ms.
- [x] Warm cache reuse recorded: encoder 0 ms, decoder 50 ms, full smoke 578 ms.
- [x] Exact Plugin package, Bundle, component, Contract, Model Instance and provider identities are
  present in the persisted smoke/doctor report.
- [x] Model Instance `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72` is persisted `Ready` and diagnosed
  `workflow_ready`.
- [x] Relative workspace Registry roots are canonicalized and covered by regression tests.
- [x] Active EfficientSAM Plugin process has no child process or Python descendant.
- [x] Focused Catalog, Plugin Registry and EfficientSAM Plugin tests pass.
- [x] M3 committed locally as `3a0e50b`.

## Milestone 4 evidence

- [x] `Install compatible model` lists the real non-Fixture EfficientSAM-Ti entry.
- [x] Model family, capability, source, publisher trust, license/digest, Bundle digest, download and
  installed sizes, immutable Plugin/model binding, platforms and CPU provider are shown first.
- [x] License acceptance remains an explicit local-user action.
- [x] A typed server operation exposes resolving, download, Bundle digest, model files, ONNX
  Contract, Rust Plugin start, model load, real inference, profile registration and Ready stages.
- [x] Structured failures retain the stopped stage and a concrete remediation action.
- [x] Browser refresh restores an active operation and exposes `View installation`.
- [x] Server restart derives Ready from durable Registry evidence rather than transient UI state.
- [x] Normal setup does not expose separate encoder/decoder upload controls.
- [x] Advanced import accepts one already verified `.annotmodel` only.
- [x] Real fresh-workspace browser install produced Ready Model Instance
  `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`; Smoke Test passed in 4,817 ms.
- [x] Focused Playwright E2E covers responsive setup, source/license review, live stage recovery,
  raw-ONNX absence and an actionable compatibility failure.
- [x] M4 committed locally as `aad5066`.

## Milestone 5 evidence

- [x] Pipeline Builder lists the selectable Ready Model Instance, not a Fixture or MissingWeights
  placeholder, for Prompted Segmentation.
- [x] Saved Draft validation and Dry Run resolve the refreshed Registry and execute the real local
  model without requiring a Provider credential.
- [x] Published Workflow `fc7d41b9-bba0-4214-b8f2-544b36e4d67f@v1` freezes exact Plugin,
  package, Bundle, component-file, Contract, Model Instance, Model Profile revision and CPU
  provider identities.
- [x] Real Run `b0dd0e50-3698-4cd5-a65d-28e6d881293b` executed Image → prior persisted bbox →
  BoxPromptSet → real EfficientSAM MaskSet → refined DetectionSet → Geometry Evaluation/Decision
  → Review → Commit.
- [x] The real Segment node completed in 1,444 ms with mask score `0.916484`; Mask-to-BBox emitted
  `[0.543843,0.384743,0.175373,0.592040]`, IoU `0.774835` and center shift `0.035796`.
- [x] Human acceptance resumed only Review/Commit and left one accepted annotation—no duplicate
  Commit.
- [x] Replay from `segment` preserved image, coarse detection and box prompt, re-executed the real
  model plus downstream nodes, and emitted one non-empty MaskSet in 1,482 ms.
- [x] Run Debug shows original bbox, BoxPromptSet, real translucent Mask overlay, refined bbox,
  Geometry evidence and a Frozen Model identity panel.
- [x] Server restart preserves the Ready instance, Published Workflow, completed Run and working
  Replay.
- [x] Referenced Bundle removal is rejected with the exact protecting Workflow reference.
- [x] Active Replay process audit shows the Rust EfficientSAM executable with no child process or
  Python worker.
- [x] Focused Rust checks and storage/image regressions pass; Web typecheck, 45 unit tests and
  production build pass.
- [x] M5 committed locally as `a322159`.

## Milestone 6 evidence

- [x] macOS ARM64 Plugin release candidate is immutable, package-verified and pinned at SHA-256
  `283a9486edaa7b25ae3cf111cd859ca90fa38de488cd3a8c9196d297d10099cd`.
- [x] Real Model Bundle, Catalog and verification report are staged with a verified
  `SHA256SUMS`; model bytes remain outside Git.
- [x] Exact GitHub Release asset names, sizes, hashes and the not-yet-published boundary are
  documented.
- [x] GUI and CLI installation instructions identify the exact workspace Plugin Registry and local
  Catalog paths.
- [x] CLI `models install` performs the real declared Smoke Test automatically and reports Ready,
  Passed and non-Fixture from a fresh isolated workspace.
- [x] Direct `.annotmodel` import performs the same real Smoke Test and reached
  Ready/Passed/non-Fixture in a separate isolated workspace.
- [x] Explicit CLI `catalog list`, `search`, `test` and `doctor` commands passed against that second
  clean installation; doctor returned `workflow_ready`.
- [x] Full Rustfmt and strict all-target/all-feature Clippy passed.
- [x] Full all-feature Rust workspace tests passed: 419 runnable tests, 0 failed; 5 explicitly
  external/billable tests ignored.
- [x] Full all-feature Rust workspace build passed.
- [x] Web typecheck, 45 unit tests, production build and all 38 Chromium E2E journeys passed.
- [x] Boundary checks, doctor and the four offline product demos passed.
- [x] In-app Run Debug verification still shows the real Mask overlay, eight typed Artifacts and
  exact frozen model identity after restart.
- [x] macOS ARM64 is marked Supported. Linux x86_64 is marked build-compatible only, with no claim
  of unexecuted real inference.
- [x] M6 is contained in the release-closure commit that records this evidence.

## Release matrix

### A. Real model

- [x] A non-Fixture prompted-segmentation Bundle exists outside Git.
- [x] Official/audited source and exact licenses are recorded.
- [x] Rust Runtime loads the real graph on macOS ARM64.
- [x] Real box-prompt inference produces a non-empty finite mask.
- [x] Mask-to-BBox produces valid refined geometry.
- [x] Report records all identities, digests, platform, provider, prompt, mask and duration.

### B. User installation

- [x] User needs no Python, conversion, raw ONNX search, or multi-file upload.
- [x] `Install compatible model` lists a clearly non-Fixture model.
- [x] Source, license, size, digest, platform and provider are visible before mutation.
- [x] Install verifies Bundle/files/graph, runs real smoke, and persists `Ready`.
- [x] Restart preserves the Ready Model Instance.

### C. Pipeline

- [x] Pipeline Builder initially blocks safely and can retry the same saved Draft after install.
- [x] Runtime executes Prompted Segmentation → Mask-to-BBox → Geometry Safety.
- [x] Debug, Review and Replay preserve complete real-model lineage.
- [x] Published Workflow freezes exact Plugin/Bundle/file/Contract/instance/provider identity.

### D. Truthfulness and Rust-only path

- [x] Fixture and publishable status are separate in current schemas and selector logic.
- [x] Accepted model and Plugin naming accurately describe the model family.
- [x] SAM 2 remains Labs unless a verified Rust Bundle exists.
- [x] Active install, smoke, run and replay process trees contain no Python.

### E. Regression

- [x] Full Rustfmt, Clippy, workspace tests and build pass.
- [x] Full Web typecheck, unit, E2E and build pass.
- [x] Provider, Workflow validator, Geometry, batch, lifecycle, Review, Replay and Export pass.
