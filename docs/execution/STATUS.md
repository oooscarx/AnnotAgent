# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-27 15:37 CST

## Current milestone

Milestone LP3 — complete: persisted application execution, three generic offline examples,
exact-version 100-image Label Pipeline batch, and reuse of the durable lifecycle/recovery path.

Milestone LP4 — next: target-Label Advisor contracts, application/server APIs for Label Pipeline
Drafts, bounded Dry Run, typed Artifact inspection, and exact-node Replay.

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

## LP3 verification

- Four Label Pipeline integration tests passed: all three executable offline flows plus validation
  of the three generic example Project Schemas.
- The application integration gate passed exact-version execution, typed checkpoint persistence,
  formal annotation storage, and a 100-image Label Pipeline Dataset batch.
- Existing pause/resume/cancel, startup reconciliation, active-Run exclusion, and persistent
  100-image restart recovery tests pass through the same application coordinator used by the new
  Pipeline path.
- `cargo test --workspace --all-features`: 124 Rust tests passed, 0 failed; doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Core domain scan for RoboCup/YOLO/domain Labels: clean.
- No conversation credential was read, restored, logged, or used.

## Remaining release blockers

LP4–LP5 must still constrain the target-Label Advisor, expose authoring/inspection/Replay APIs, and
finish the product GUI/Inspector plus full Rust/Web/browser acceptance. Until those gates pass,
Label Pipeline Alpha is not release-complete.
