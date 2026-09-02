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
- The verified YOLO implementation is the exact YOLOX Nano 416×416 COCO-80 export. Other YOLO tensor
  layouts require a separately versioned model contract.
- YOLO weights are not bundled. The fixed upstream recipe requires explicit user terms confirmation
  and provisioning before a product installation can become Ready.
- SAM expects the declared 1024-pixel ViT-B encoder plus standard six-input mask decoder contract.
  Other SAM families/exports require a separately versioned contract; no accepted pair is bundled.
- PIDNet accepts one NCHW RGB input and one NCHW float32 logit output. Other export layouts require
  another model contract; no PIDNet checkpoint has been real-smoke verified here.
- Dense semantic class maps are transported as JSON arrays in HTTP Vision v1. The configured 256 MiB
  response bound limits very large source images; a future binary Artifact store can reduce this
  transport cost without changing the typed `SemanticMask` contract.
