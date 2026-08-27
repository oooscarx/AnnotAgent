# Artifact Model

`VisionArtifact` is the immutable data plane between VLMs, specialist models, deterministic tools, Validators, Refiners, Review, and Commit. Supported values are classification, bounding boxes, keypoints, polyline, polygon, semantic mask, instance masks, attributes, and relations.

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

SQLite history stores Artifacts and lineage beside node trace and annotations. Native export/import preserves Artifact-derived provenance and annotation revisions. Lossier formats return compatibility warnings instead of silently claiming full fidelity.

