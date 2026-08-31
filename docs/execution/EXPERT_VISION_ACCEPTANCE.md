# Expert Vision Acceptance Evidence

Updated: 2026-09-01

Legend: `PASS`, `PENDING`, `LIVE-CONDITIONAL`, `MANUAL`.

## A — Extension architecture

- PASS — Expert model registration through a versioned Manifest.
- PASS — Unknown expert detector works without a Core enum/runtime branch; the Test Edge Detector
  fixture registers from a Manifest and the existing generic backend.
- PASS — Python Worker SDK/scaffold and 13 deterministic conformance/helper tests.
- PASS — Capability, model and complete contract discovery are versioned and Rust-validated.
- PASS — Manifest validation requires health, protocol, contract, weights and sample-conversion
  evidence before `Available`; descriptor migration fails closed to `Unknown` for untested HTTP
  Workers.

## B — Expert models

- PASS — Capabilities represent prompted/semantic/instance segmentation and object/open-vocabulary
  detection without model-branded Core node kinds.
- PASS — SAM, YOLO, RF-DETR and LocateAnything workspace profiles are registered from
  `ExpertModelManifest`; PIDNet and Grounding DINO are representable through the same SDK preset
  manifest boundary without Core changes.
- PASS — RF-DETR and YOLO share Object Detection plus DetectionSet contracts; SAM uses Prompted
  Segmentation plus MaskSet; LocateAnything preserves Open Vocabulary, Phrase Grounding and
  `NotProvided` scores.
- PASS — `MissingWeights` is a non-publishable Manifest state and conflicts with `weights_ready`;
  `Available` requires `weights_ready` plus the other registration evidence.

## C — SAM legal path

- PASS — DetectionSet → BoxPromptSet conversion with exact Detection item subjects.
- PASS — Generic Image+Box/Point PromptSet → MaskSet prompted-segmentation runner supports mock and
  protocol-v1 Worker backends.
- PASS — Explicit Core Mask-to-BBox produces a refined DetectionSet outside the Worker.
- PASS — Generic DAG Artifacts retain original box, prompt, mask, tight box and both model evidence
  sources; focused Runtime test verifies the full lineage.
- PASS — Existing editable `sam_prompted_refiner` Drafts migrate idempotently to the explicit
  public chain while preserving the original final node id and downstream edges.
- PENDING — Builder availability/failure/no-candidate policy tests.

## D — VLM geometry quality

- PASS — Every Detection carries geometry semantics; Vision Language maps to `CoarseHypothesis`,
  detector output to `PredictedGeometry`, and prompted segmentation to `MaskRefinedGeometry`.
- PASS — Detection score semantics preserve missing/unknown values without fabrication.
- PASS — Geometry reports and Dry Run summaries expose geometry, human-adjustment and refiner
  metrics independently from score confidence.
- PENDING — Evidence-driven refiner selection.

## E — Advisor

- PASS — Builder already has constrained Draft mutation, static validation, Dry Run and
  human-approval boundaries.
- PARTIAL — Compatible Model Profile filtering exists for provider-backed profiles.
- PARTIAL — Builder now has `find_artifact_conversion_path`; Worker health/contracts/label-space/
  score/geometry selection evidence remains M5–M7.
- PENDING — Structured failure diagnosis and evidence-driven revision cases 1–8.

## F — Product

- PARTIAL — Models/Settings currently expose Detection Worker configuration and Test Worker.
- PENDING — Guided Add Expert Model flow with discovery, identity, explicit sample smoke and
  availability gate.
- PENDING — Advisor reasons for adding or withholding prompted segmentation.
- PARTIAL — Node Inspector accepts prompt/mask/polygon Artifacts and renders their bounds; dedicated
  quality presentation remains M7.

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

## M2 evidence

- Rust protocol fixture completes health, capability, model, contract, warmup and inference calls;
  invalid duplicate model and spoofed Worker contract identities fail closed.
- `uv run --project sdk/python --extra test python -m pytest sdk/python/tests`: 13 passed.
- Native `annotagent worker scaffold --preset sam2` generated a template whose SDK-parsed Manifest
  remained `missing_weights`.
- All six presets plus generic capability scaffolding are adapter templates only; no weight,
  Provider credential or external inference was used.
- Full Rust workspace tests, strict Clippy and Rustfmt checks passed.

## M3 evidence

- `ArtifactConversionRegistry` finds the three-node DetectionSet refinement cycle and returns no
  path when `capability.segment` is absent.
- Runtime test `sam_artifact_chain_preserves_original_prompt_mask_and_refined_box` verifies exact
  Detection subject, prompt item, Mask item, original evidence, SAM evidence and refined box.
- Application test `published_prompted_segmentation_pipeline_runs_end_to_end_offline` publishes and
  executes the complete chain, then exposes its refined box in the real review queue.
- Python SDK 14/14 tests pass, including the wire-compatible MaskSet helper and exact prompt item
  reference.
- Public Node Catalog exposes only generic conversion/capability nodes; no SAM brand branch was
  added to Core or Runtime.
- Full Rust gate passes 291 tests with zero failures and one explicit billable smoke ignored;
  strict workspace Clippy and Rustfmt also pass.

## M4 evidence

- `AnnotationFailureClass` deterministically distinguishes all nine required failure classes;
  Provider failure and no-candidate are not geometry evidence.
- `GeometryQualityReport` records geometry semantics, bounded deterministic checks, explicit unknown
  measurements, refiner comparison and historical correction rate.
- The Generic Object Detection Dry Run test exposes predicted geometry and aggregate quality through
  the real Application path, then separately proves Provider failure and terminal no-candidate
  results remain distinct.
- The Server review regression drags a bbox, observes API geometry metrics, resolves review, and
  verifies center-shift/area-change metrics persisted in Correction Memory.
- Full Rust workspace: 294 passed, zero failed, one explicitly billable smoke ignored. Strict Clippy,
  Rustfmt, Web typecheck and all 40 Web tests pass.

## M5 evidence

- Application catalog construction registers every configured HTTP Worker backend first, then
  registers its versioned capability Manifest; there is no separate SAM/YOLO Server card path.
- Default LocateAnything, RF-DETR, SAM and YOLO profiles all remain non-publishable
  `MissingWeights`; settings metadata alone never claims health or smoke-test success.
- `legacy_sam_refiner_migrates_to_an_auditable_capability_chain` proves idempotent compatibility
  expansion and downstream edge preservation.
- Model migration tests prove additive loading for older Settings, common YOLO/RF-DETR DetectionSet
  contracts, SAM prompt/mask contracts and LocateAnything missing-score semantics.
- Full Rust workspace: 295 passed, zero failed, one explicitly billable smoke ignored. Strict
  workspace Clippy and Rustfmt pass.
