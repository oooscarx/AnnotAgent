# Real Prompted-Segmentation Delivery Alpha Status

Last updated: 2026-09-03 CST

## Current Milestone

M5 — Pipeline, Geometry Safety, Debug, Review, Replay and restart closure complete. M6 release
artifacts and complete regression validation are next.

## Current candidate

EfficientSAM-Ti split ONNX is a real, non-Fixture Supported model on the verified macOS ARM64 CPU
path. A fresh workspace completed the GUI Catalog → license → install → Contract → real Smoke Test
→ Ready path and preserved the selectable Model Instance across an AnnotAgent restart.

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
- Added the dedicated `org.annotagent.efficientsam-onnx@1.0.0` Rust Plugin. Its manifest and Recipe
  share the exact capability Contract hash
  `ad3f23abcadb04561dcced33bae9cbfccbce4c13910a715fc964f1281c8f56ee`.
- Implemented the audited EfficientSAM preprocessing and split graph protocol: dynamic RGB NCHW
  float input, original-pixel box/point prompts, `int64` original size, finite IoU candidate
  selection, zero-logit threshold, source-size restoration, COCO RLE and prompt lineage.
- Packaged and verified the current macOS ARM64 Plugin outside Git. Package SHA-256:
  `283a9486edaa7b25ae3cf111cd859ca90fa38de488cd3a8c9196d297d10099cd`.
- Installed the real Bundle as Model Instance `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`, ran the fixed
  official dog-image box prompt and reached persisted `Ready` / `workflow_ready` status.
- The cold smoke produced a valid MaskSet with prompt lineage, mask coverage `0.061009`, and Core
  tight bbox `xywh=[0.543843,0.384743,0.175373,0.592040]`. Encoder inference was 2,238 ms,
  decoder inference 51 ms and full process/conformance smoke 4,532 ms on this host.
- A warm repeat reused the persisted image embedding: encoder 0 ms, decoder 50 ms, full smoke
  578 ms. The cache identity includes exact encoder SHA-256 plus input image digest.
- Hardened both Plugin and Model Bundle Registries so a relative workspace argument is canonicalized
  and persisted model/process roots are reconstructed under the trusted Registry root. This fixed
  a real one-millisecond Plugin child launch failure after CLI installation.
- Extended smoke evidence with exact prompt lineage, observed coverage, non-degenerate mask-to-bbox
  geometry and internally consistent encoder/decoder timing.
- Inspected the active Plugin process tree: one Rust EfficientSAM executable and no child process;
  install, smoke and inference used no Python or conversion process.
- Added server-owned model installation operations with exact Catalog/Bundle/Plugin identity,
  byte progress, ten truthful lifecycle stages, Model Instance IDs, structured errors and concrete
  retry actions. The GUI resumes an active operation after a browser refresh.
- Expanded the Catalog entry and install review with model family, capability, plugin binding,
  target platforms, CPU provider, 38,577,735-byte download size and 41,834,247-byte installed size.
- Replaced the normal raw-file path with a six-step modal and a detailed real-install checklist;
  advanced local import remains a single verified `.annotmodel`, never separate ONNX files.
- Ran the real browser flow against a fresh workspace containing only the immutable Plugin package
  and trusted local Catalog. The UI copied and verified the real Bundle, inspected both graphs,
  executed the official dog-image bbox prompt, and persisted Model Instance
  `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72` as `Ready` in 4,817 ms.
- Reloaded the browser during sample inference and recovered `Run real sample inference` plus its
  live action from the server. Restarted AnnotAgent and confirmed one selectable Ready model.
- Project Workflow Catalog now projects each selectable Ready Model Instance as a concrete
  `model-instance:<uuid>` specialist model. Pipeline Builder exposes
  `EfficientSAM-Ti ONNX · Ready local model`, while Provider credential resolution is skipped for
  this immutable local Rust model.
- Published Workflow version `fc7d41b9-bba0-4214-b8f2-544b36e4d67f@v1` freezes the exact Plugin,
  Plugin package, Bundle, component files, capability Contract, Model Instance, Model Profile
  revision and CPU execution provider.
- Executed the real nine-node Workflow on the official 1072×603 dog image:
  Image → Existing Bounding Box → Box Prompt → EfficientSAM → Mask → Mask-to-BBox → Geometry
  Evaluation → Geometry Decision → Human Review → Commit. Run
  `b0dd0e50-3698-4cd5-a65d-28e6d881293b` reached `CompletedWithReview`, then completed after one
  explicit human acceptance without a duplicate annotation.
- The real Run persisted one non-empty COCO RLE MaskSet, score `0.916484`, refined bbox
  `[0.543843,0.384743,0.175373,0.592040]`, coarse/refined IoU `0.774835`, normalized center shift
  `0.035796`, and the exact Detection → Prompt → Mask → refined Detection lineage. Segment runtime
  was 1,444 ms on Rust ONNX CPU.
- Replay from `segment` preserved `image`, `coarse_bbox`, and `box_prompts`; only `segment` and its
  downstream geometry/review nodes re-executed. The replay returned a non-empty real mask in
  1,482 ms and retained the same immutable Model Instance identity.
- Run Debug now displays a Frozen Model identity panel and renders uncompressed COCO RLE masks as
  a bounded translucent overlay instead of exposing the mask only as plain JSON. Pipeline
  Artifact counts include typed checkpoint outputs.
- Fixed hidden model-image resizing: thumbnails no longer upscale small images, and Workflows with
  local Plugin/Model Instance bindings preserve original pixels so the Plugin request and Image
  Artifact dimensions cannot diverge.
- Restarted AnnotAgent after publication and Review. The Ready Model Instance, Published Workflow,
  completed Run, Frozen Model identity and Replay all remained available.
- Attempting to remove the referenced Bundle was rejected with the exact protecting Workflow
  reference. Active Replay process inspection showed only the Rust EfficientSAM executable and no
  child process or Python worker.

## In progress

- Build release metadata/assets and execute the complete Rust/Web/cross-platform truth matrix in
  M6.

## Next

Run the complete release test matrix, validate the current-platform release asset list and user
installation instructions, then record final Git and unsupported-platform truth.

## Latest verification

| Gate | Result |
| --- | --- |
| Rustfmt | PASS — `cargo fmt --all -- --check` |
| Initial Rust workspace tests | FAIL — 1 stale test fixture identity; 57 Application tests passed, 1 failed, 1 billable ignored before the workspace run stopped |
| Repeated Rust workspace tests | PASS — `cargo test --workspace --all-features` |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle verification | PASS — real non-Fixture EfficientSAM-Ti Bundle built and verified outside Git |
| Real graph inspection | PASS — encoder and decoder loaded with ORT CPU; descriptors recorded |
| Real inference | PASS — official dog image; box prompt → MaskSet → tight bbox through Rust ORT CPU |
| Web E2E | PASS — 2 focused Plugin/Bundle setup scenarios |
| Model Recipe | PASS — 3 downloads / 41,814,211 bytes and 8 static files verified |
| Real Bundle | PASS — 38,577,735 bytes / `3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c` |
| Local Catalog | PASS — persisted `trusted_local_catalog`, real entry searchable after refresh |
| Latest focused tests | PASS — 15 Catalog, 4 Plugin Registry and 5 EfficientSAM Plugin tests |
| Latest Rust workspace tests | PASS — all runnable tests; only explicitly external/billable tests ignored |
| Latest Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Real cold smoke | PASS — mask coverage 0.061009; bbox `[0.543843,0.384743,0.175373,0.592040]`; encoder 2238 ms; decoder 51 ms; total 4532 ms |
| Real warm smoke | PASS — cached encoder 0 ms; decoder 50 ms; total 578 ms |
| Rust-only process audit | PASS — Plugin executable had no child process and no Python descendant |
| Fresh-workspace GUI install | PASS — real Bundle → real smoke → Ready in 4,817 ms |
| Browser refresh recovery | PASS — active `running_sample_inference` operation restored |
| Server restart recovery | PASS — Ready instance and selectable profile restored |
| Pipeline Builder selection | PASS — Ready `model-instance:ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72` is selectable |
| Real Published Workflow | PASS — 9 nodes; real MaskSet → refined bbox → Geometry Safety → Review → Commit |
| Run Debug | PASS — mask overlay plus frozen Plugin/Bundle/file/Contract/Instance/Profile/provider identity |
| Replay | PASS — upstream image/coarse bbox/box prompt preserved; real segment and downstream nodes re-executed |
| Referenced Bundle removal | PASS — rejected with exact `fc7d41b9-bba0-4214-b8f2-544b36e4d67f@v1` reference |
| Active Replay process audit | PASS — Rust EfficientSAM process, no child and no Python worker |
| Latest focused Web checks | PASS — typecheck, 45 unit tests and production build |
| Latest focused Rust checks | PASS — Image Tools, exact prior Project/Run annotation source and Server/Application checks |
| Latest commit | `aad5066` — M4; M5 commit pending |

## Release-blocking remainder

The real-model execution, normal-user GUI installation and Published Workflow/Review/Replay gates
are closed on macOS ARM64 CPU. Release packaging and the final cross-platform/regression matrix
remain blocking.

## Real blocker

There is no external blocker recorded yet. A missing implementation or an unperformed audit is not
classified as an external blocker.
