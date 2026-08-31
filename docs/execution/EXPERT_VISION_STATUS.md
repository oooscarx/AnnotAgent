# Expert Vision Status

Updated: 2026-09-01

## Current milestone

M1 — Expert Model Manifest (complete)

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

## In progress

- None. M1 is ready for its independent local commit.

## Next

- M2: extend the existing HTTP Worker protocol and add the Python Worker SDK, scaffold CLI and
  conformance tests.

## Latest Rust tests

- `cargo test --workspace --all-features`: PASS; zero failures; one opt-in billable smoke ignored.
- M1 `cargo fmt --all --check`: PASS.
- M1 `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- M1 `cargo test --workspace --all-features --quiet`: PASS; zero failures; one opt-in billable
  smoke ignored.
- M1 focused Core suite: 67 passed.

## Latest Web tests

- `npm --prefix web run typecheck`: PASS.
- `npm --prefix web test -- --run`: PASS — 12 files, 40 tests.
- `npm --prefix web run build`: PASS.

## Latest Worker contract tests

- Existing Rust provider contract tests passed inside the workspace suite.
- `python3 -m py_compile` for the HTTP, SAM, RF-DETR and LocateAnything reference workers: PASS.
- Python SDK/conformance suite does not exist yet and is an M2 deliverable.

## Latest browser verification

- Not run for M0; no product UI changed yet.

## Latest local commit

- Pre-task head: `8a1cbb1 fix(agent): harden pipeline builder tool protocol`.
- M0: `127d0de docs: establish expert vision integration baseline`.
- M1 commit pending: `feat(models): add capability-driven expert model manifests`.

## Release-blocking remainder

- M1–M8 and every unchecked item in `EXPERT_VISION_ACCEPTANCE.md`.

## Live-conditional

- Real SAM, RF-DETR, LocateAnything, YOLO, PIDNet and Grounding DINO inference where legal local
  weights, dependencies and suitable hardware are not configured.
- External provider calls requiring user-owned credentials.

## Real blockers

- None for offline architecture, protocol, Mock/conformance, Runtime, UI or Agent work.

## Audited capability-state matrix

| Backend | Adapter implemented | Worker implemented | Process/weights configured | Health/smoke | Registered execution path | Builder selectable |
| --- | --- | --- | --- | --- | --- | --- |
| SAM 2 | Yes: generic HTTP adapter plus legacy RoboCup refiner | Yes: reference worker | No evidence | Not run; Labs/unavailable | Legacy refiner only; public Prompt→Mask→BBox path missing | No |
| YOLO | Yes: generic HTTP/Pipeline detection adapters | Reference HTTP worker supports explicit local Ultralytics weights | No evidence | Not run | Generic Object Detection and legacy YOLO Skill paths | Only configured/healthy generic profiles; preset is not available |
| RF-DETR | Yes: detection Worker adapter | Yes: reference worker | No evidence | Not run; disabled/unavailable | Generic Object Detection | No until configured, healthy and tested |
| LocateAnything | Yes: detection Worker adapter | Yes: reference worker | No evidence | Not run; disabled/unavailable | Generic Open Vocabulary/Phrase Grounding | No until configured, healthy and tested |
| PIDNet | Generic semantic-segmentation adapter contract only | No tracked concrete worker | No | None | Generic segment catalog only | No |
| Grounding DINO | Generic open-vocabulary adapter contract only | No tracked concrete worker | No | None | Generic detection contract only | No |

The baseline deliberately does not convert “file exists” into “supported”. `Available` and Builder
selection will be closed behind the complete M1–M3 manifest/contract and M7 sample-test gates.
