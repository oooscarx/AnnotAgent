# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-27 16:46 CST

## Current milestone

Milestone LP5 — complete: Project Label authoring, Shared Stage/per-Label Pipeline GUI, controlled
Node Catalog editing, Detect & Crop composition, bbox/crop preview, Inspector, Replay, and the full
Rust/Web/browser release gate.

## Product objective

The active release target is **AnnotAgent Label Pipeline Alpha**:

- Project Schema owns annotation semantics and Labels.
- Workflow owns how each Label is produced.
- multiple Label Pipelines may fan out from one shared upstream node;
- a shared node has one compiled identity and executes once per image/configuration;
- Advisor output is always a registry-bounded editable Draft;
- Dry Run, immutable publish, exact-version execution, Artifact inspection, and Replay are real
  Runtime capabilities rather than UI placeholders.

The full RoboCup Workflow is now a non-blocking extension example and Roadmap item. Its existing
implementation remains under regression tests, but it is not the primary Label Pipeline Alpha
acceptance path.

## Completed

- Workflow Alpha M0–M9 remains the tested foundation: immutable Workflow versions, typed flat DAG,
  cache/checkpoint/Replay traces, Review, batch recovery, Model Registry, controlled Advisor, and
  security boundaries.
- LP1 added `LabelPipeline`, `SharedWorkflowStage`, `PipelineSource`, `PipelineStep`, `ArtifactRef`,
  `DetectionSetArtifact`, `CropSetArtifact`, `ClassificationSetArtifact`,
  `AnnotationCandidateSet`, `ModelBinding`, and `SkillBinding`.
- LP1 compiles one Image Input plus all shared and per-Label steps into the existing flat Workflow
  graph; three Label Pipelines referencing one shared detector compile to one detector node with
  three outgoing edges.
- LP1 implements explicit Image + DetectionSet → CropSet fan-out and DetectionSet +
  ClassificationSet → AnnotationCandidateSet fan-in. Crop and Classification records retain exact
  parent/subject item references.
- LP1 static validation blocks unknown Labels/tasks/nodes/models/Skills, capability mismatches,
  broken shared-stage ownership, missing sources, and Artifact type mismatches.
- Published snapshot content hashing now includes the optional Label Pipeline authoring projection;
  existing Workflow/RoboCup snapshots remain compatible through a defaulted optional field.
- LP2 extends the existing DAG checkpoint, Trace, content-addressed cache, and Replay engine with
  typed Pipeline Artifacts; it does not introduce a parallel Runtime.
- LP2 adds executable Core Crop, Filter, Map Label, Attach Result, Attach Attribute, and Confidence
  Gate nodes. Image Input, Human Review, Commit, Artifact Cache, and Replay remain generic built-ins.
- LP2 adds separate Classification and YOLO Detection Skill crates. The Detection Skill accepts an
  Image and produces only `DetectionSetArtifact`; Crop exists only in Core.
- Classification supports mock, registry-bounded OpenAI-compatible VLM, and generic HTTP JSON
  bindings. Detection supports mock and generic HTTP JSON bindings over protocol v1.
- `replay_from` resets one node and its descendants while retaining completed upstream outputs.
  The crop-classification gate proves classifier Replay does not call the detector again.
- LP3 connects typed Pipeline execution to application-owned published Runs. The selected image is
  materialized as an `ImageArtifact`; node outputs are persisted in the Run checkpoint and committed
  Pipeline candidates become formal stored annotations.
- The application Model/Node Catalog now exposes real mock classifier and detector bindings plus the
  formal Skill and Core node descriptors used by publication validation.
- Three generic example Project Schemas cover whole-image classification, detection, and shared
  detector → Crop → Classification composition without enabling RoboCup.
- A 100-image synthetic Dataset gate executes the exact immutable published whole-image
  Classification Workflow and persists one committed annotation per child Run.
- LP4 constrains both mock and LLM Advisor paths to an exact Project task/Label pair. The LLM may
  only adjust registered bindings and review gates on a safe composition-backed Draft.
- Saving a Label Pipeline Draft recompiles its authoring projection into the one flat Runtime DAG;
  static Label type/Registry errors remain editable but block publish.
- Label Pipeline Dry Run calls the same typed DAG runners used by Published Runs, accepts at most 10
  selected images, and creates neither a durable Run nor a formal annotation.
- Run Inspector exposes each node's configuration, typed inputs/outputs, status, attempts, cache,
  usage, latency, and structured error directly from the persisted checkpoint.
- Replay starts at one exact node in a sandbox, keeps byte-for-byte-equal upstream checkpoint
  outputs, and never recovers credentials from Run history.
- LP5 adds validated Project Schema Label creation without coupling Label semantics to Runtime
  methods. Existing published versions remain immutable.
- The Workflow GUI renders Shared Stages separately from per-Label lanes, exposes typed sources,
  Model Binding, threshold, padding, class mapping, fallback, parameters, Save, Dry Run, and publish.
- The optional Detect & Crop template is visibly and internally detector → filter → Core Crop →
  Classification → Attach Result → Confidence Gate → Commit; Crop is never placed in the detector.
- The Node Artifact Inspector renders the original image, Detection bbox overlays, Crop previews,
  typed JSON inputs/outputs, full configuration, timing/error/usage, and real Replay results.

## LP5 verification

- In-app browser gate passes Project Label creation → target-Label Draft → human-visible Pipeline
  editor → real Dry Run → immutable publish → exact-version Run → Inspector → classifier Replay.
- Browser Replay reports `scene.day.classifier`, Gate, and Commit re-executed while
  `core.image_input` remains preserved. The inspected image and configuration render without layout
  overlap at the tested desktop viewport.
- `cargo test --workspace --all-features`: 126 Rust tests passed, 0 failed; doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Web typecheck/build passed; 8 files and 15 tests passed.
- `./scripts/acceptance.sh`: passed end to end, including domain/secret scans, doctor, and offline
  generic demo.
- Core domain scan for RoboCup/YOLO/domain Labels: clean.
- No conversation credential was read, restored, logged, or used.

## Release status

All 20 Label Pipeline Alpha Release Blocking gates have direct offline evidence. Live
OpenAI-compatible inference and configured external HTTP detector quality remain optional deployment
conditions, not blockers for the mock/offline Alpha contract. RoboCup remains regression-tested and
on the Roadmap; it is not the primary acceptance path.
