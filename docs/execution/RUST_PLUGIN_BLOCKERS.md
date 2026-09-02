# Rust Expert Model Plugin Alpha — Blockers

No architecture blocker is active.

Potential live-conditional constraints are tracked separately from implementation blockers:

- production checkpoints are intentionally not committed;
- the verified YOLOX recipe remains NeedsWeights in product until a user explicitly reviews its
  upstream terms, provisions the checkpoint and runs the installed smoke test;
- CUDA/TensorRT validation requires compatible hardware and native libraries;
- RF-DETR and LocateAnything require a legally usable Rust-executable export before a real Ready
  claim;
- SAM real smoke requires a compatible encoder/decoder pair supplied under explicit upstream terms;
  both component digests are now supported and enforced.
- PIDNet real smoke requires a legal ONNX export matching the declared NCHW input/logit contract.
