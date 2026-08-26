# Annotation Schema

## IDs

Project, image, annotation, revision, run, step, task, label, and tool-call identifiers are newtypes with serialization, display, and parsing support. UUID-backed IDs cannot be accidentally exchanged with task/label strings.

## Geometry

`NormalizedPoint` and `NormalizedRect` hide their fields. Constructors reject NaN, infinity, values outside `[0,1]`, zero/negative extents, and rectangles extending past the image. Pixel conversion is based on `ImageMetadata { width, height, mime_type, sha256 }`.

`AnnotationValue` is a tagged enum rather than a bag of optional fields:

- classification labels;
- `[x,y,width,height]` bounding box;
- named visible/invisible keypoints;
- polyline;
- polygon rings;
- instance mask as polygon rings or COCO RLE;
- relation with source, predicate, and target annotation IDs.

Attribute tasks attach typed `AttributeValue` entries to a target geometry. Each annotation also records source (`model`, deterministic tool, combined, human, imported), review status, confidence, provenance, and UTC creation time.

## Project YAML

`ProjectSchema::from_yaml` uses `serde_path_to_error` and denies unknown fields. Validation checks schema version, relative dataset root without `..`, duplicate task/label IDs, task-kind constraints, attributes, missing dependencies, cycles, target tasks, and registered validator/refiner names.

```bash
cargo run -p annotagent -- project validate examples/robocup/project.yaml
```

## Revision model

A human edit never silently replaces history. `AnnotationRevision` records a new revision ID, annotation ID, parent revision, before/after snapshots, actor, reason, and UTC time. Runtime refinements and GUI edits both append revisions. Accept/reject/delete operations update the review state through the same transactional path and may add a correction record.

## Export compatibility

Native JSON represents every internal value. COCO supports bbox, keypoints, polygon and instance mask; YOLO Detection supports bbox; YOLO Segmentation supports polygon/mask; LabelMe supports bbox, keypoint, polyline and polygon. Exporters report every skip and unsupported task kind instead of silently dropping data.
