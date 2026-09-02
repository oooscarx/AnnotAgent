# RF-DETR Rust plugin

`org.annotagent.rfdetr-onnx@1.0.0` is a native Rust process for the official RF-DETR detection ONNX
boundary. It advertises only `ObjectDetection` and emits only `DetectionSetArtifact`.

## Audited contract

The 2026-09-02 implementation audit used Roboflow's official
[ONNX export documentation](https://rfdetr.roboflow.com/develop/learn/export/) and official
[`_onnx/inference.py`](https://github.com/roboflow/rf-detr/blob/develop/src/rfdetr/export/_onnx/inference.py).
The Rust plugin therefore requires:

- one fixed-size float32 `[1,3,H,W]` input;
- RGB conversion, antialias-free half-pixel bilinear resize and ImageNet normalization;
- float32 `dets` shaped `[1,Q,4]` containing normalized `cxcywh` boxes;
- float32 `labels` shaped `[1,Q,C]` containing logits;
- per-class sigmoid, explicit background-slot policy and flattened query/class top-k selection;
- no NMS, matching RF-DETR's model-specific official postprocessor.

The Workflow configuration must record `training_dataset_version`. It may also record exact
`class_labels`, Project `class_mapping`, confidence threshold, result limit and background class.
The Registry separately freezes the provisioned ONNX SHA-256, package digest and contract digest.

## Readiness

The implementation status is `live_conditional`. Rust ONNX loading, preprocessing, inference,
decode, score, normalized geometry, class mapping, Artifact construction and cancellation are
implemented. No checkpoint is bundled or automatically downloaded, and no legal external export
was supplied for this milestone. The package remains `NeedsWeights`, then `Installed` after local
hash-bound provisioning, and can become `Ready` only after its actual installed-process sample
smoke passes.

The opt-in real test uses:

```text
ANNOTAGENT_TEST_RFDETR_ONNX
ANNOTAGENT_TEST_RFDETR_IMAGE
```

It starts the real Rust process, runs plugin conformance, invokes the generic Object Detection Skill
and checks typed checkpoint/dataset evidence. No Python exporter or runtime is invoked by the test.
