# PIDNet ONNX plugin

This Rust process implements the `SemanticSegmentation` capability for a PIDNet-compatible ONNX
export. It accepts one float32 NCHW RGB input and one float32 NCHW class-logit output, restores the
argmax class map to original image dimensions, and returns a typed `SemanticMask` artifact.

The checkpoint is not bundled. A user must provision `pidnet.onnx`, accept its upstream terms, and
run a real smoke test. The registry therefore reports `NeedsWeights` before setup is complete.
Fixed or dynamic spatial dimensions are supported; a dynamic export uses bounded `input_width` and
`input_height` node parameters. Project label mapping is explicit and does not alter model class
identifiers.
