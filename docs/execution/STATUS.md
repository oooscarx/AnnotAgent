# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-27 14:59 CST

## Current milestone

Milestone LP2 — complete: executable Pipeline Artifacts, generic Core nodes, formal Classification
and YOLO Detection Skills, mock/OpenAI-compatible/HTTP JSON bindings, cache, Commit, and node Replay.

Milestone LP3 — next: application-owned example Projects, persisted Pipeline Run integration,
100-image Label Pipeline batch, lifecycle/recovery, and CLI demo gates.

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

## LP2 verification

- Three executable Label Pipeline integration tests passed: whole-image classification, typed
  detection, and crop classification with Replay.
- Two provider protocol tests passed: generic HTTP detector/classifier and bounded
  OpenAI-compatible classifier.
- `cargo test --workspace --all-features`: 122 Rust tests passed, 0 failed; doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Core domain scan for RoboCup/YOLO/domain Labels: clean.
- No conversation credential was read, restored, logged, or used.

## Remaining release blockers

LP3–LP5 must still connect this Runtime to persisted application Runs, ship three example Projects
and the Label-specific 100-image/lifecycle evidence, constrain the target-Label Advisor, and finish
the product GUI/Inspector plus full Rust/Web/browser acceptance. Until those gates pass, Label
Pipeline Alpha is not release-complete.
