# RF-DETR Detection ONNX Rust plugin

This independent Rust process implements the official RF-DETR detection ONNX boundary:

- one fixed-size float32 RGB NCHW image input;
- ImageNet normalization after antialias-free half-pixel bilinear resize;
- float32 `dets` normalized `cxcywh` boxes and `labels` logits;
- per-class sigmoid and flattened query/class top-k selection;
- typed `DetectionSetArtifact` output with exact checkpoint and training-dataset identity.

The package is `live_conditional`: it contains the complete Rust ONNX execution path, but the
repository does not bundle or automatically download an RF-DETR export. A user-provisioned export
must match the contract and pass the installed-process smoke before the Registry can mark it Ready.
The official RF-DETR postprocessor does not apply NMS; this plugin preserves that model-specific
behavior rather than silently adding a different suppression policy.

No scripting process, exporter, package manager, Provider credential, auto-download, annotation Commit
or Core model branch is part of this plugin.
