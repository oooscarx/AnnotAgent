# YOLO Backend

YOLO is represented as an Expert Vision Worker with the generic `ObjectDetection` Capability. It
does not introduce a YOLO Core node or Runtime branch.

The Worker must expose Vision Protocol v1 health, capability, model and contract discovery. Its
Manifest declares Image input, DetectionSet output, score semantics, label space, immutable model
version/checkpoint identity, runtime requirements and checkpoint license. A selected-image sample
must convert to a valid non-empty Artifact before registration becomes available.

Crop remains a Core operation. A “YOLO Detect & Crop” product template composes:

```text
Object Detection → Select & Map → Crop
```

The repository contains protocol/scaffold support but no bundled or downloaded YOLO weights.
Actual inference is `LIVE-CONDITIONAL` on user-supplied legal weights, dependencies and hardware.
