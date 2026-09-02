# Rust Expert Model Plugin Alpha — Acceptance Evidence

Only commands actually executed are recorded here.

## M6 SAM and PIDNet Rust plugins — 2026-09-02

- Added `org.annotagent.sam-onnx` with strict two-session SAM ViT-B contracts, exact box/point
  prompt tensors, bounded multi-mask selection, relative mask scores, original-coordinate
  uncompressed RLE, typed prompt lineage and encoder embedding cache keyed by encoder plus image
  digest.
- Added named multi-file weight components to Manifest/Registry/CLI. The SAM registry test proves
  one component stays `NeedsWeights`, both copied/hash-verified components move only to `Installed`,
  and the ordered aggregate checkpoint identity persists across reopen. No smoke report is forged.
- Added `org.annotagent.pidnet-onnx` with bounded fixed/dynamic NCHW input, ImageNet normalization,
  strict NCHW logit decode, channel argmax, source-size restoration, explicit class mapping and a
  new dense lineage-preserving `SemanticMaskArtifact`.
- Added generic Prompted and Semantic Segmentation Skill runners and corrected capability binding:
  `capability.segment` requires PromptedSegmentation, while `capability.semantic_segment` requires
  SemanticSegmentation. Core contains no plugin-brand branch.
- Offline plugin tests: PASS — 12 active focused cases across manifests, preprocessing, prompts,
  masks, cache, semantic decode, Registry projections and component identities; 2 real-process
  tests are explicitly ignored until legal SAM/PIDNet weights and sample images are supplied.
- `cargo test --workspace --all-features`: PASS — 362 passed, 4 ignored (one billable Provider,
  three external real-model smokes). Existing Core
  `sam_artifact_chain_preserves_original_prompt_mask_and_refined_box` still proves
  MaskSet → Mask-to-BBox → Geometry Evaluation/Decision lineage.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, Rustfmt, diff check and
  Rust-only boundary scan: PASS.
- SAM and PIDNet real-weight tests were not run. Both product model projections remain
  `NeedsWeights`; no production checkpoint, credential, remote mutation or push was used.

## M0 baseline — 2026-09-02

- `git status --short --branch`: clean `main`, 17 commits ahead of `origin/main`.
- `git log --oneline -20`: latest pre-task commit `cb17d7b`.
- `cargo test --workspace --all-features`: PASS — 339 passed, 1 billable provider smoke ignored.
- Active boundary inventory found no Rust Plugin API, SDK, Host, Registry, package lifecycle or
  official Rust plugin process before this task.
- Existing protocol: versioned loopback HTTP Vision client supports health, capability, infer and
  cancel; the SAM reference additionally exposes model and contract discovery.
- Existing model selection: `ModelProfile`, capability matching and immutable
  `ModelProfileSnapshot`; no plugin package/checkpoint identity is frozen yet.
- Existing geometry safety and typed Artifact lineage tests pass.

Milestone evidence will be appended after the corresponding tests pass.

## M1 Plugin API — 2026-09-02

- Added `annotagent-plugin-api` with validated IDs, semantic versions, SHA-256 identities,
  capability-oriented model contracts, least-privilege permissions, weights/licenses, lifecycle
  states and authenticated process protocol DTOs.
- `cargo test -p annotagent-plugin-api`: PASS — 4 tests.
- `bash scripts/check-rust-plugin-boundary.sh`: PASS.
- Manifest TOML round trip and semantic digest stability, unsafe entrypoints, forbidden permissions,
  invalid contracts and path traversal are covered.

## M2 Rust SDK and dummy process — 2026-09-02

- Added an async Rust Plugin Server with one-time standard-input startup, dynamic loopback binding,
  nonce handshake and session-token authentication on every endpoint.
- Added health, capability, model, contract, warmup, infer, cancel and shutdown routes; bounded body
  and response sizes; typed request/Artifact validation; panic mapping; cancellation tracking; and
  bounded PNG/JPEG decoding.
- Added shared conformance runner and `org.annotagent.dummy-detector` as a standalone Rust binary.
- `cargo test -p annotagent-plugin-sdk -p annotagent-plugin-dummy-detector`: PASS — 2 SDK tests.
- Strict Clippy for both packages: PASS.
- Standalone executable smoke: PASS — process handshake, authenticated health and graceful shutdown.
- Rust-only boundary scan: PASS.

## M3 Host, Registry and lifecycle — 2026-09-02

- Added deterministic `.annotplugin` pack/verify/extract with exact SHA-256 file list, expansion and
  file-count limits, target executable checks, duplicate/link/path-traversal rejection and atomic
  side-by-side installation.
- Added a least-privilege Host with cleared environment, private directories, one-use token/nonce,
  bounded handshake/logs/responses, authenticated calls, graceful/forced stop and crash detection.
- Added a durable registry for versions, install state, explicit license approval, copied checkpoint
  identities, tests, events, model projections and reference-protected uninstall.
- Published Workflow snapshots now include exact generic plugin/package/protocol/model/checkpoint/
  contract identity in semantic content hashing.
- Added migration 13 with all plugin lifecycle, permission, model, weight, health, test, reference,
  license and event tables; no token, credential or image-byte column exists.
- Added `annotagent plugin` pack, inspect, verify, install/update, list/show/versions, provision,
  test/doctor, foreground start/restart, enable/disable, references and uninstall surfaces.
- Package/registry tests: PASS — 4 tests. Dummy process Host E2E: PASS — handshake, health, typed
  inference, conformance, forced crash and Core survival.
- Core/storage/plugin/app strict Clippy: PASS. Rust-only boundary scan: PASS.

## M4 Rust ONNX Runtime — 2026-09-02

- Added a model-neutral Common crate for deterministic resize/letterbox, normalization,
  NCHW/NHWC conversion, bbox transforms, NMS, masks, connected components, contours, polygon
  simplification and numeric validation.
- Added a native ONNX Runtime crate with explicit CPU/CUDA/TensorRT selection, strict accelerator
  registration, graph input/output shape and dtype discovery, typed tensors, thread configuration,
  warmup, cancellation boundaries, model SHA-256 and exact session caching.
- The real CPU runtime loaded a generated legal ONNX Identity graph and returned the actual
  [2.5, -4.0] tensor. Shape/type discovery, cache reuse/eviction and cancellation also pass.
- Focused tests for both new packages: PASS — 7 tests.
- Full workspace/all-feature tests: PASS — 346 tests, 1 explicitly billable Provider test ignored.
- Strict all-target/all-feature Clippy for both packages: PASS.
- The fixture contains no learned weight and is not claimed as an expert model. The first real
  expert-model release gate remains M5.

## M5 YOLO Rust Plugin — 2026-09-02

- Added org.annotagent.yolo-onnx as a standalone Rust process with an exact YOLOX Nano 416×416
  COCO-80 contract, CPU ONNX session, BGR NCHW preprocessing, grid/stride decode, true composed
  detection score, original-image projection, class-aware NMS, mapping and typed DetectionSet.
- Added a Core-facing PluginPipelineBackend. Cancellation is forwarded to the authenticated plugin
  cancel endpoint and Provider failures remain structured.
- The deterministic package verifies. Installation without explicit terms approval is rejected.
  An approved temporary test installation projects an exact MissingWeights Model Profile and
  NeedsWeights status with plugin/package/capability identity.
- Fixed recipe identity: official YOLOX Nano release asset, 3,659,407 bytes, SHA-256
  c789161ed43c8269fcd4e67c67eeeb4e80c622da2eb296a20bc6007bd18a0b7d. The file is not committed.
- Explicit real-model test: PASS — isolated Host process, token/handshake/discovery/conformance,
  native ONNX image inference, Object Detection Skill and Core Filter in 1.41 seconds.
- Regular plugin/SDK/Host tests and strict all-target/all-feature Clippy: PASS. The real test stays
  opt-in because it requires explicitly supplied external weights and image paths.
- Full workspace/all-feature tests: PASS — 351 tests; the billable Provider smoke and externally
  supplied real-model test are the 2 explicit ignored cases in the ordinary run.
- The package produces only DetectionSet and has no Crop or Commit behavior.
