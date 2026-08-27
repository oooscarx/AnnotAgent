# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-27 14:59 CST

## Current milestone

Milestone LP1 — complete: domain-neutral Label Pipeline contracts, typed intermediate Artifacts,
parent/subject lineage, static validation, and compilation to one flat shared-stage DAG.

Milestone LP2 — next: execute the typed Pipeline Artifacts through Core nodes and the two formal
Skills, including versioned HTTP Vision Protocol adapters, cache, Commit, and node Replay.

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

## LP1 verification

- `cargo test -p annotagent-core`: 26 passed, 0 failed.
- `cargo test --workspace --all-features`: 117 Rust tests passed, 0 failed; doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Core domain scan for RoboCup/YOLO/domain Labels: clean.
- No conversation credential was read, restored, logged, or used.

## Remaining release blockers

LP2–LP5 must still complete the executable Core nodes and Skill backends, three offline demos,
100-image Label Pipeline batch/lifecycle evidence, bounded Label Advisor, product GUI/Inspector,
Replay boundary, and full Rust/Web/browser acceptance. Until those gates pass, Label Pipeline Alpha
is not release-complete.
