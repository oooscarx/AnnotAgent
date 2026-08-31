# Expert Vision Status

Updated: 2026-09-01

## Current milestone

M4 — Geometry quality and failure classification (complete)

## Completed

- Read the complete 2,264-line master request and saved the exact repository copy.
- Verified `main` started clean and 21 commits ahead of `origin/main`.
- Verified the pre-change Rust workspace suite passes with one explicitly billable Provider smoke
  ignored.
- Audited the current Provider Model Profiles, generic Vision Model Registry, HTTP Worker clients,
  detection worker protocol, SAM adapter, segmentation capability, public Node Catalog, Pipeline
  Artifacts, Builder tools and Dry Run summary.
- Confirmed existing detection work already provides typed Detection evidence, score semantics,
  health-aware descriptors, object/open-vocabulary workers, Resize/Tile/Project Coordinates and
  bounded Agent editing.
- Verified the Web typecheck, 40-unit-test suite and production build pass.
- Verified all four tracked Python reference workers parse without importing dependencies or
  loading/downloading weights.
- Published the audited capability-state matrix below.
- Added a versioned, credential-free `ExpertModelManifest` with Provider/Worker/Mock connections,
  typed input/output and prompt contracts, model/version/checkpoint/runtime/license facts, score
  semantics and geometry semantics.
- Added the full availability lifecycle and a strict evidence gate: an Expert Model cannot validate
  as `Available` without health, protocol, contract, weights and sample-conversion evidence.
- Added `CoarseHypothesis`, `PredictedGeometry`, `MaskRefinedGeometry`, calibrated and
  human-verified geometry semantics independently from score confidence.
- Extended the existing Model Registry to ingest arbitrary Worker-backed manifests, project them
  into the legacy descriptor boundary and expose the original manifest without adding a Core model
  enum or runtime branch.
- Added a safe descriptor migration: untested HTTP Workers are never promoted to `Available`,
  while healthy in-process Mock/deterministic fixtures retain truthful offline availability.
- Added an extensibility test registering a fictional `Test Edge Detector` solely from capability
  and contract metadata.
- Extended the existing protocol v1 compatibly with model and contract discovery, multi-model
  capability summaries and optional warmup while retaining health, inference and cancellation.
- Added strict Rust discovery validation for protocol/model/Worker identity, duplicate IDs,
  checkpoint digests, complete Manifest contracts and warmup response scope.
- Added `sdk/python/annotagent_vision_worker` with Pydantic schemas, FastAPI helpers, bounded image
  decode, coordinate normalization, Artifact serialization, cancellation, error mapping,
  conformance tests and manifest loading.
- Added native `annotagent worker scaffold` support and matching Python scaffolding for generic
  capabilities plus SAM 2, YOLO, RF-DETR, LocateAnything, PIDNet and Grounding DINO presets.
- Verified every preset produces a valid, explicitly unavailable template and never downloads or
  claims model weights.
- Added typed `BoxPromptSet`, `PointPromptSet`, `MaskSet`, and `PolygonSet` Pipeline Artifacts with
  strict set/item references and validation.
- Added a capability-neutral Artifact Conversion Registry and the Builder's bounded
  `find_artifact_conversion_path` tool. The SAM refinement cycle is returned only when every
  executable node exists.
- Replaced the public implicit Detection→instance-mask segment contract with explicit
  Image+PromptSet→MaskSet `PromptedSegmentation` contracts in Rust and the Python Worker SDK.
- Added executable `core.detections_to_box_prompts`, `core.mask_to_bbox`, and polygon-mask
  conversion nodes plus the generic `capability.segment` runner for mock or protocol-v1 Workers.
- Added an offline full lineage test proving original Detection → Prompt → Mask → refined Detection
  retains source evidence and auditable geometry.
- Added a Published Runtime end-to-end test that executes the complete offline prompt-segmentation
  chain and persists the refined bounding box as an explicit human-review candidate.
- Extended Node Inspector geometry extraction and authoring types for prompt/mask/polygon Artifacts.
- Added stable failure classes that keep Provider/Worker failure, empty candidates, semantic error,
  geometry error, missing score, domain risk, invalid Artifact and budget exhaustion distinct.
- Added per-candidate `GeometryQualityReport` and aggregate `GeometryQualitySummary`; VLM boxes now
  default to coarse hypotheses while prompted-segmentation boxes are mask-refined geometry.
- Extended Dry Run and the bounded Agent summary with failure, review, manual-adjustment and refiner
  metrics. Existing sample-test records remain backward compatible.
- Bounding-box edits now return normalized center-shift/area-change/IoU through the API and persist
  those metrics into Correction Memory when review is resolved.
- Added deterministic Dry Run cases proving a Provider failure is classified without becoming a
  no-candidate/geometry claim, while a successful terminal empty result is `NoCandidate`.

## In progress

- None. M4 is ready for its independent local commit.

## Next

- M5: make Pipeline Builder decisions and revision policy consume availability, conversion paths,
  failure classes and geometry-quality evidence.

## Latest Rust tests

- `cargo test --workspace --all-features`: PASS — 294 tests; zero failures; one opt-in billable
  smoke ignored.
- M1 `cargo fmt --all --check`: PASS.
- M1 `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- M1 `cargo test --workspace --all-features --quiet`: PASS; zero failures; one opt-in billable
  smoke ignored.
- M1 focused Core suite: 67 passed.
- M2 full workspace Rust tests: PASS; zero failures; one opt-in billable smoke ignored.
- M2 full workspace strict Clippy and Rustfmt: PASS.

## Latest Web tests

- `npm --prefix web run typecheck`: PASS.
- `npm --prefix web test -- --run`: PASS — 12 files, 40 tests.
- `npm --prefix web run build`: PASS.

## Latest Worker contract tests

- Existing Rust provider contract tests passed inside the workspace suite.
- `python3 -m py_compile` for the HTTP, SAM, RF-DETR and LocateAnything reference workers: PASS.
- `uv run --project sdk/python --extra test python -m pytest sdk/python/tests`: PASS — 14 tests.
- Rust generic protocol fixture covers health/capabilities/models/contracts/warmup/infer and rejects
  duplicate/spoofed discovery identities.
- Native SAM scaffold command plus SDK manifest parse smoke: PASS.

## Latest browser verification

- Not run for M0; no product UI changed yet.

## Latest local commit

- Pre-task head: `8a1cbb1 fix(agent): harden pipeline builder tool protocol`.
- M0: `127d0de docs: establish expert vision integration baseline`.
- M1: `4faed86 feat(models): add capability-driven expert model manifests`.
- M2: `561de50 feat(workers): add an extensible expert vision worker sdk`.
- M3: `b98ceaa feat(workflow): compose expert models through typed artifact conversions`.
- M4 commit pending: `feat(evaluation): distinguish semantic, geometry and provider failures`.

## Release-blocking remainder

- M5–M8 and every unchecked item in `EXPERT_VISION_ACCEPTANCE.md`.

## Live-conditional

- Real SAM, RF-DETR, LocateAnything, YOLO, PIDNet and Grounding DINO inference where legal local
  weights, dependencies and suitable hardware are not configured.
- External provider calls requiring user-owned credentials.

## Real blockers

- None for offline architecture, protocol, Mock/conformance, Runtime, UI or Agent work.

## Audited capability-state matrix

| Backend | Adapter implemented | Worker implemented | Process/weights configured | Health/smoke | Registered execution path | Builder selectable |
| --- | --- | --- | --- | --- | --- | --- |
| SAM 2 | Yes: generic prompted-segmentation Pipeline adapter plus legacy refiner | SDK scaffold/reference adapter | No evidence | Not run; missing weights | Public Prompt→Mask→BBox path works with mock and any conforming healthy Worker | No until configured, healthy and sample-tested |
| YOLO | Yes: generic HTTP/Pipeline detection adapters | Reference HTTP worker supports explicit local Ultralytics weights | No evidence | Not run | Generic Object Detection and legacy YOLO Skill paths | Only configured/healthy generic profiles; preset is not available |
| RF-DETR | Yes: detection Worker adapter | Yes: reference worker | No evidence | Not run; disabled/unavailable | Generic Object Detection | No until configured, healthy and tested |
| LocateAnything | Yes: detection Worker adapter | Yes: reference worker | No evidence | Not run; disabled/unavailable | Generic Open Vocabulary/Phrase Grounding | No until configured, healthy and tested |
| PIDNet | Generic semantic-segmentation adapter contract only | No tracked concrete worker | No | None | Generic segment catalog only | No |
| Grounding DINO | Generic open-vocabulary adapter contract only | No tracked concrete worker | No | None | Generic detection contract only | No |

The baseline deliberately does not convert “file exists” into “supported”. `Available` and Builder
selection will be closed behind the complete M1–M3 manifest/contract and M7 sample-test gates.
