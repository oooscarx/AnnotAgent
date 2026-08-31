# Artifact Conversion Registry

AnnotAgent composes model capabilities through typed Artifacts. A conversion is available only
when its executable node is registered; a model name never creates an implicit Core branch.

## Registered conversions

| From | Node | Additional input | To |
| --- | --- | --- | --- |
| `DetectionSet` | `core.detections_to_box_prompts` | — | `BoxPromptSet` |
| `BoxPromptSet` | `capability.segment` | `Image` | `MaskSet` |
| `PointPromptSet` | `capability.segment` | `Image` | `MaskSet` |
| `MaskSet` | `core.mask_to_bbox` | `BoxPromptSet` at execution time | `DetectionSet` |
| `MaskSet` | `core.mask_to_polygon` | — | `PolygonSet` |
| `DetectionSet` | `core.crop` | `Image` | `CropSet` |
| `CropSet` | `capability.classify` | — | `ClassificationSet` |
| `ClassificationSet` | `core.attach_result` | `DetectionSet` | `AnnotationCandidateSet` |

`ArtifactConversionRegistry::find_conversion_path` returns every shortest legal path using the
provided `NodeRegistry`. A same-type request is allowed to return a non-empty refinement cycle. For
example, `DetectionSet → DetectionSet` returns the prompted-segmentation chain only when prompt
conversion, segmentation, and mask-to-box are all executable.

The Pipeline Builder exposes this through `find_artifact_conversion_path`. The result is advisory
evidence, not permission to invent a model binding: static validation and Model availability still
apply.

## Lineage rules

- Every Box Prompt references one exact Detection item.
- Every Mask references one exact Box or Point Prompt item.
- Every refined Detection records the original geometry, prompt reference, Mask reference, refined
  geometry, prompted-segmentation model, and both original/refinement evidence.
- Fan-out/fan-in joins use Artifact IDs and item IDs, never array order.
- Polygon and uncompressed COCO RLE masks support tight-box conversion. Compressed RLE must be
  decoded by the Worker before it crosses the protocol boundary.

These rules make Node Inspector and Replay able to distinguish source evidence from derived
geometry.
