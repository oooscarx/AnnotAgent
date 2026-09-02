# VLM Geometry Safety

A vision-language model can identify the right object while returning a loose, shifted or otherwise
training-inaccurate box. AnnotAgent therefore treats a VLM detection score as semantic evidence and
its box as an uncalibrated coarse hypothesis by default.

This rule is enforced at four independent boundaries:

1. The Model Profile revision freezes an operation-scoped quality contract.
2. The Project declares the geometry quality required by each task kind.
3. Rust static validation follows every candidate-to-Commit path and blocks score-only geometry
   acceptance.
4. Runtime evidence, calibration or Human Review supplies the missing geometry verification.

`SemanticConfidence` is never interpreted as IoU, tightness or center accuracy. Grid overlays,
resizing and coordinate instructions may improve a proposal, but do not change its calibration
state.

For training-quality bounding boxes, a VLM-only Pipeline must route through mandatory Human Review.
If a healthy prompted-segmentation backend is registered, AnnotAgent may propose Detection → Box
Prompt → Mask → BBox → Geometry Evaluation → Geometry Decision, with Review retained for conflicts.
If the Provider failed, no candidate exists, or the object is semantically wrong, segmentation is
not the primary repair.

See [Geometry Quality Model](GEOMETRY_QUALITY_MODEL.md), [Safe VLM Detection Pipelines](SAFE_VLM_DETECTION_PIPELINES.md), and [Geometry Calibration](GEOMETRY_CALIBRATION.md).
## Plugin-backed refinement

A Ready PromptedSegmentation plugin may provide MaskSet evidence after coarse VLM geometry, but its
semantic score is not box-quality proof. The path must still convert prompts and masks with lineage,
run Geometry Evaluation/Decision, and retain Review when calibration or stability evidence is
insufficient. Missing weights or smoke evidence blocks the Draft rather than falling back to an
external worker.
