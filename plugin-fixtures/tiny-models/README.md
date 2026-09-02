# Tiny model fixtures

The M4 ONNX Runtime tests generate a two-element Identity graph directly from the
published ONNX protobuf wire contract. The generated graph has no learned weights and
is executed by the real native ONNX Runtime CPU provider. Keeping the fixture generator
in the Rust test makes the provenance auditable and avoids storing an opaque binary.

This fixture proves model loading, graph introspection, typed tensor input/output,
session caching, warmup boundaries, cancellation boundaries, and SHA-256 identity. It
is not presented as an expert vision model and is not available in the product model
registry.
