# YOLOX Nano ONNX plugin

This package is an independent Rust process implementing the Object Detection capability. It
accepts one Image Artifact and emits one typed DetectionSet Artifact. It never crops or commits
annotations.

The package does not contain model weights. Installation stays in NeedsWeights until the user
reviews the upstream terms and provisions a checkpoint. The fixed recipe records the official
YOLOX Nano ONNX release URL, an 8 MiB download ceiling and the verified checkpoint SHA-256.

The implemented model contract is:

- input: one BGR float32 NCHW tensor at 416 by 416, top-left padded with value 114;
- output: 3,549 YOLOX rows with 4 box values, objectness and 80 COCO class values;
- decoding: strides 8, 16 and 32, then class-aware NMS;
- geometry: normalized to the original image, never the padded tensor;
- score: objectness multiplied by the selected class value.

Other YOLO-family exports are not accepted merely because their filename ends in ONNX. A different
tensor contract requires a distinct model revision or plugin version.
