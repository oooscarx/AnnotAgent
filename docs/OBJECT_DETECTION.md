# Object Detection Capability Skill

`annotagent.object_detection` is the backend-neutral Capability Skill for trained detectors. A
Workflow binds `object_detection.detect` to any Model Registry entry that provides
`ObjectDetection`; model product names never become Core node kinds.

The node accepts one Image and these bounded parameters:

- `target_labels`: Project Label IDs to produce;
- `class_mapping`: model-native class name → Project Label ID;
- optional `confidence_threshold`, `iou_threshold`, and `max_detections`.

It emits exactly one typed `DetectionSetArtifact`, including for a valid no-object result. Every
candidate retains the source model, model-native label, mapped Project Label, normalized geometry,
real score and score semantics. Missing or incomparable scores are not dropped merely because a
threshold exists; they remain available for Evidence Gate or Human Review.

The bundled Mock backend is `mock-object-detector`. The
`object-detection.specialist-review` template is:

```text
Image → Object Detection → Human Review → Commit
```

Cropping is still the generic Core Crop node. To build Detect & Crop, compose the detector with
Filter and Crop rather than adding crop behavior to this Skill.

The Generic example in `examples/object-detection/` can be opened without enabling a domain Skill.
The template begins as a Draft; edit its target labels/class mapping, Dry Run it, and publish an
immutable version before starting a formal Run.

See [Specialist Detection](SPECIALIST_DETECTION.md) for Registry/version requirements and
[Hybrid Detection Workflows](HYBRID_DETECTION_WORKFLOWS.md) for specialist-first fallback, Cache
and Replay behavior.
