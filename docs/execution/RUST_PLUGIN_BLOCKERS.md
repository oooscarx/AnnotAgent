# Rust Expert Model Plugin Alpha — Blockers

No architecture blocker is active.

Potential live-conditional constraints are tracked separately from implementation blockers:

- production checkpoints are intentionally not committed;
- CUDA/TensorRT validation requires compatible hardware and native libraries;
- RF-DETR and LocateAnything require a legally usable Rust-executable export before a real Ready
  claim;
- SAM may require multiple ONNX files whose individual digests must be provisioned.
