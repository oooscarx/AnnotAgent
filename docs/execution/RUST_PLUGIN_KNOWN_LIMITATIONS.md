# Rust Expert Model Plugin Alpha — Known Limitations

- The Alpha process boundary is not a universal OS-level sandbox.
- Publisher signature verification is optional for local packages; unsigned state is visible.
- Production weight download recipes require explicit user action and license acceptance.
- Model packages without compatible checkpoints remain setup-only and cannot enter runnable
  drafts.
- Historical external HTTP Workers remain readable during migration but are not represented as
  installed Rust plugins.
- M4 validates the native ONNX CPU provider. CUDA and TensorRT remain live-conditional on compatible
  hardware, drivers and native provider libraries.
- Portable inference cancellation is enforced at native-call boundaries. Host process termination
  is the hard boundary if one native operator does not return.
