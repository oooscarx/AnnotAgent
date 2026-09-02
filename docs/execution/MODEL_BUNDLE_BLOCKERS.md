# Model Bundle Provisioning Blockers

Last updated: 2026-09-02 CST

## External model publication

No real prompted-segmentation `.annotmodel` may be shipped until an official source provides enough
evidence to fix the checkpoint/export bytes, redistribution status, code and weight licenses,
input/output tensors and expected sample output. AnnotAgent will not copy a third-party ONNX mirror
or convert a checkpoint on a user's machine to evade this requirement.

## Signatures and hosting

Official signature verification needs a pinned project signing key and an official catalog release
process. Alpha can fully validate unsigned local developer bundles and signed official fixtures, but
external hosting remains conditional on publishing infrastructure outside this repository.

## Accelerator/platform evidence

CPU on the development macOS aarch64 host is the available release environment. CUDA, TensorRT,
Windows and Linux real-model results require those systems and are not inferred from compilation.

There is no blocker to implementing the format, verifier, fixture, local catalog, content store,
compatibility resolver, workflow pins, GUI, CLI/TUI or offline release tests.
