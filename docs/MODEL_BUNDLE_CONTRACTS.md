# Model Bundle Contracts

Model Contracts are versioned JSON documents that describe actual ONNX tensor names, aliases,
dtypes, ranks/dimensions, dynamic axes, and cross-file connections. Generic file roles such as
`image_encoder`, `mask_decoder`, or a future `depth_auxiliary_2` are Manifest data rather than Core
branches.

Binding requires all of the following to agree:

- installed Plugin ID/version/model and its complete capability Contract hash;
- Bundle compatibility range, required file roles, model format and opset;
- current platform and explicit execution provider;
- ONNX Runtime discovery of every declared input/output tensor;
- exact accepted license digest when acceptance is required.

A valid static Contract produces a `Preparing` Model Instance, not a Ready one. The smoke suite is a
separate runtime gate. Contract aliases permit audited exporter variation, but inferred or silently
renamed tensors are not accepted. Runtime receives only role-to-verified-file mappings beneath the
Bundle root.
