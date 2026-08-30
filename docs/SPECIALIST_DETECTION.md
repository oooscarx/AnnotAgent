# Specialist Detection

AnnotAgent treats a trained detector as a Model Backend for the generic `ObjectDetection`
capability. A detector name never becomes a Core node type. YOLO, RF-DETR, a private service, or a
future backend can all bind `object_detection.detect` when their Registry descriptor and Worker
contract match the published Workflow.

## Binding contract

A specialist descriptor records:

- Registry and model ID;
- architecture and immutable model version;
- checkpoint SHA-256 and training-dataset version when weights are required;
- exact model-native label space;
- Detection Worker protocol version and endpoint policy;
- score semantics, request limits, estimated cost, runtime requirements, and license metadata.

The Workflow supplies Project Labels, Model-to-Project class mapping, confidence/IoU limits and a
maximum result count. The Skill returns a typed `DetectionSet`; Crop, validation, Review and Commit
remain separate nodes. An empty set is a successful no-object result.

Enabling a versioned checkpoint Worker fails closed when required identity, label-space or license
facts are missing. Discovery must report the configured capability and exact label space; inference
outside that space is rejected before an Artifact is created.

## Specialist-first execution

When an enabled specialist declares the target Label, the Advisor may suggest:

```text
Image
→ specialist Object Detection
→ Evidence Gate
→ accepted result, or one bounded open-vocabulary fallback
→ optional Crop verification
→ Review / Commit
```

The specialist's finite score remains attached to its own evidence. It is never averaged with a
score-less or differently calibrated fallback result. High accepted evidence can skip the fallback;
empty, low, conflicting, domain-risk or correction-risk evidence may request it within explicit
step, call and cost limits.

Use `mock-object-detector` for offline behavior and contract tests. See
[RF-DETR Backend](RFDETR_BACKEND.md) for the optional reference Worker,
[Detection Evidence](DETECTION_EVIDENCE.md) for matching semantics, and
[Hybrid Detection Workflows](HYBRID_DETECTION_WORKFLOWS.md) for composition and Replay.
