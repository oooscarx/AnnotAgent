# YOLOX Rust plugin

The official package identity is org.annotagent.yolo-onnx version 1.0.0. Its first model revision is
yolox-nano-coco-onnx, an exact YOLOX Nano 416×416 COCO-80 tensor contract. The plugin is an
independent Rust process and communicates only through authenticated HTTP Vision v1.

## Capability boundary

Input is one Image Artifact plus the bounded Object Detection node parameters. Output is exactly one
DetectionSet Artifact containing:

- normalized boxes in original-image coordinates;
- YOLOX objectness × class value as DetectionConfidence;
- model-native COCO label and numeric class ID;
- optional Project label mapping;
- source model, capability and Artifact lineage;
- exact checkpoint SHA-256 in Artifact metadata.

The plugin never crops, refines, reviews or commits. Those remain explicit Core/Skill nodes.

## Exact model contract

Preprocessing scales the image into 416×416 while preserving aspect ratio, places it at the
top-left of a 114-filled tensor, converts RGB pixels to BGR and produces contiguous float32 NCHW
without value scaling.

The single output must have 3,549 rows and 85 columns. Rows are decoded across strides 8, 16 and 32
into center/size boxes, objectness and 80 COCO classes. Boxes are projected through the exact
preprocessing scale, clipped to the source image, filtered by score, processed by class-aware NMS
and bounded by max_detections.

An export with 84 columns, another input size, decoded boxes, anchors, segmentation heads or a
different label space is rejected. Supporting it requires a separate declared model contract and
version.

## Weights and license state

Weights are not bundled. The manifest records one fixed upstream recipe:

- source: the official YOLOX 0.1.1 release asset yolox_nano.onnx;
- maximum size: 8 MiB;
- verified SHA-256: c789161ed43c8269fcd4e67c67eeeb4e80c622da2eb296a20bc6007bd18a0b7d;
- upstream terms link: the official YOLOX repository license.

Installation requires an explicit permission/code-license/weight-terms confirmation. Without it,
installation is rejected. After installation and before provisioning, the Registry projects the
model as MissingWeights and the plugin as NeedsWeights. The recipe is never downloaded by the
Pipeline Builder or Agent.

## Verification

Offline unit tests cover:

- exact manifest capability and no Crop/Commit claim;
- BGR NCHW preprocessing and top-left padding;
- stride/grid decode and true score composition;
- class-aware NMS and source-coordinate projection;
- rejection of a different YOLO tensor contract;
- deterministic package verification and exact NeedsWeights Model Profile projection.

The explicit real-model test supplies the checkpoint and sample image through test-only paths,
starts the actual packaged process through Plugin Host, performs authenticated discovery and
conformance, executes native ONNX inference, sends its DetectionSet through the generic Object
Detection Skill and then through Core Filter. The test passes with the exact checkpoint above. The
checkpoint and image remain outside the repository.
