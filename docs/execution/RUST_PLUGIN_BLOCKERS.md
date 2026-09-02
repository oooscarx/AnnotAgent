# Rust Expert Model Plugin Alpha — Blockers

No architecture blocker is active.

Potential live-conditional constraints are tracked separately from implementation blockers:

- production checkpoints are intentionally not committed;
- the verified YOLOX recipe remains NeedsWeights in product until a user explicitly reviews its
  upstream terms, provisions the checkpoint and runs the installed smoke test;
- CUDA/TensorRT validation requires compatible hardware and native libraries;
- RF-DETR requires a legally usable official-contract ONNX export and sample before a real Ready
  claim. Its Rust execution path is complete and live-conditional.
- LocateAnything has no audited complete official Rust-callable runtime path and is explicitly
  UnsupportedPlatform. A future package needs a legal ONNX/Candle/Burn/Rust implementation and real
  smoke; the legacy Python Worker is not a fallback.
- SAM real smoke requires a compatible encoder/decoder pair supplied under explicit upstream terms;
  both component digests are now supported and enforced.
- PIDNet real smoke requires a legal ONNX export matching the declared NCHW input/logit contract.
