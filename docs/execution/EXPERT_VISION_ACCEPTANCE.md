# Expert Vision Acceptance Evidence

Updated: 2026-09-01

Legend: `PASS`, `PENDING`, `LIVE-CONDITIONAL`, `MANUAL`.

## A — Extension architecture

- PASS — Expert model registration through a versioned Manifest.
- PASS — Unknown expert detector works without a Core enum/runtime branch; the Test Edge Detector
  fixture registers from a Manifest and the existing generic backend.
- PENDING — Python Worker SDK/scaffold and conformance tests.
- PARTIAL — Capability and input/output discovery exists, but model/contract endpoints and unified
  worker manifests are incomplete.
- PASS — Manifest validation requires health, protocol, contract, weights and sample-conversion
  evidence before `Available`; descriptor migration fails closed to `Unknown` for untested HTTP
  Workers.

## B — Expert models

- PARTIAL — Capabilities already represent prompted/semantic/instance segmentation and object/open
  vocabulary detection without model-branded Core node kinds.
- PENDING — SAM/YOLO/RF-DETR/LocateAnything/PIDNet/Grounding DINO manifests and truthful state
  projections.
- PASS — `MissingWeights` is a non-publishable Manifest state and conflicts with `weights_ready`;
  `Available` requires `weights_ready` plus the other registration evidence.

## C — SAM legal path

- PENDING — DetectionSet → BoxPromptSet conversion.
- PARTIAL — Generic prompted-segmentation HTTP adapter and a RoboCup-specific legacy refiner exist.
- PENDING — Worker MaskSet → explicit Core Mask-to-BBox → refined DetectionSet.
- PENDING — Original box, mask and refined box are all inspectable through the generic DAG.
- PENDING — Builder availability/failure/no-candidate policy tests.

## D — VLM geometry quality

- PARTIAL — Manifest migration maps Vision Language capability to `CoarseHypothesis`; propagation
  onto every produced Detection Artifact remains M4.
- PASS — Detection score semantics preserve missing/unknown values without fabrication.
- PENDING — Geometry report, human-adjustment and refiner Dry Run metrics.
- PENDING — Evidence-driven refiner selection.

## E — Advisor

- PASS — Builder already has constrained Draft mutation, static validation, Dry Run and
  human-approval boundaries.
- PARTIAL — Compatible Model Profile filtering exists for provider-backed profiles.
- PENDING — Worker health/contracts/label-space/score/geometry/conversion inspection tools.
- PENDING — Structured failure diagnosis and evidence-driven revision cases 1–8.

## F — Product

- PARTIAL — Models/Settings currently expose Detection Worker configuration and Test Worker.
- PENDING — Guided Add Expert Model flow with discovery, identity, explicit sample smoke and
  availability gate.
- PENDING — Advisor reasons for adding or withholding prompted segmentation.
- PENDING — Expert Artifact-chain inspection for prompts and masks.

## G — RoboCup

- PASS — Current Ball Skill template is capability-bound and contains tests rejecting concrete
  backend brands.
- PARTIAL — Hard-negative validation and crop classification paths exist.
- PENDING — Generic conditional geometry refinement and specialist-first Advisor evidence.
- PASS — Generic Projects do not load RoboCup in the existing integration test.

## M0 evidence

- `git status --short --branch`: clean at task start; `main...origin/main [ahead 21]`.
- `git log --oneline -20`: captured in task execution; head `8a1cbb1`.
- `cargo test --workspace --all-features`: PASS, no failures, one opt-in billable smoke ignored.
- `npm --prefix web run typecheck`: PASS.
- `npm --prefix web test -- --run`: PASS, 12 files and 40 tests.
- `npm --prefix web run build`: PASS.
- Python parse baseline: PASS for four tracked reference workers; no weights or dependencies were
  loaded.
- `cmp` against the attached prompt: exact repository copy retained.
- Source audit confirms a real Rust SAM HTTP adapter exists but is a domain-specific refiner and is
  not the public typed conversion chain requested by this Alpha.

## M1 evidence

- Core focused suite: 67 passed, including strict availability, provider connection projection,
  legacy HTTP migration and arbitrary Worker manifest registration.
- Full Rust workspace suite: PASS with zero failures and one explicitly billable smoke ignored.
- Full workspace strict Clippy and Rustfmt checks: PASS.
- Production implementation contains no model-brand comparison or model-branded Core Node kind.
