# PIDNet Rust plugin

`org.annotagent.pidnet-onnx@1.0.0` is an independent Rust process implementing
`SemanticSegmentation`. Core sees only the capability, the Image input contract and a typed
`SemanticMask` output.

## Contract

The model `pidnet-semantic-onnx` accepts one float32 NCHW RGB input with batch one and three
channels. Fixed spatial dimensions are read from the ONNX graph. A dynamic graph requires bounded
`input_width` and `input_height` node parameters in `[32,4096]`.

The single output is float32 NCHW class logits. The plugin validates finiteness, performs per-pixel
channel argmax and nearest-neighbor restoration to exact source-image dimensions. It returns a
dense row-major `SemanticMaskArtifact` containing:

- original width and height;
- one lossless model class ID per pixel;
- optional explicit model-class to Project-label mapping;
- source Image Artifact reference;
- exact checkpoint SHA-256 and tensor-size metadata;
- unvalidated state requiring a later policy/review decision.

`SemanticMaskArtifact` is distinct from prompted `MaskSetArtifact`: semantic logits assign a class
to every pixel, while each prompted mask has a prompt parent and an independent score.

## Weights and readiness

The package declares one controlled component named `model`, stored as `pidnet.onnx`. No checkpoint
or download recipe is bundled. Local provisioning copies the file, records its SHA-256 and original
filename, and does not depend on the source path afterward. Status remains `NeedsWeights` before
provisioning and cannot become `Ready` without successful contract loading and real sample smoke.

## Verification state

Offline tests cover exact capability/output projection, dynamic input constraints, multi-channel
argmax, original-size coordinate restoration, dense Artifact validation and deterministic package
installation as `NeedsWeights`. An ignored opt-in process test accepts explicitly supplied legal
weights and an image and validates the real native ONNX process through the generic Semantic
Segmentation runner.

No PIDNet checkpoint has been supplied in this milestone. The process implementation is complete,
but its real-weight smoke and accuracy are `live-conditional` and are not reported as passing.
