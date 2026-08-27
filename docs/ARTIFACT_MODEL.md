# Artifact Model

`ArtifactEnvelope` is the versioned immutable data plane between VLMs, specialist models,
deterministic tools, Validators, Refiners, Review, and Commit. Every envelope owns Project, Run,
image and producing-node scope, one typed vision or pipeline payload, parent/item references,
provenance, creation time and an optional deterministic cache key. Envelope validation rejects
scope/payload mismatches, self-parenting, duplicate lineage and invalid typed payloads.

`VisionArtifact` remains the annotation-shaped payload. Supported values are classification,
bounding boxes, keypoints, polyline, polygon, semantic mask, instance masks, attributes, and
relations. `PipelineArtifact` carries Image, DetectionSet, CropSet, ClassificationSet and
AnnotationCandidateSet payloads with item-level subject/parent references.

Every Artifact has a stable `ArtifactId`, image/task scope, source node, role, confidence, provenance, metadata, validation state, and revision/replacement lineage. Geometry remains normalized and checked. Model-facing tool results contain structured data or stable Artifact references; a short UI summary is stored separately and is never substituted for geometry.

The normal flow is:

```text
candidate Artifact
→ optional refined replacement Artifact
→ Validator evidence and validation state
→ accept / reject / request refinement
→ safe Commit or Human Review
```

A Refiner returns a new Artifact revision rather than asking a VLM to copy points. The field-line test, for example, preserves the coarse Polyline and its refined replacement, validates the replacement directly, and commits without a second model geometry response.

Published DAG traces now materialize input and output envelopes, including cache identity, beside
their compatibility payload arrays. SQLite history stores Artifacts and lineage beside node trace
and annotations. Native export/import preserves Artifact-derived provenance and annotation
revisions. Lossier formats return compatibility warnings instead of silently claiming full
fidelity.
