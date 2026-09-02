# SAM Model Provisioning

`org.annotagent.sam-onnx` is a Rust prompted-segmentation Plugin with separate `image_encoder` and
`mask_decoder` roles. It implements Image + Box/Point Prompt → MaskSet with subject lineage and
embedding cache. The repository does not ship third-party SAM weights.

Current product truth:

- the built-in prompted-segmentation Fixture uses two AnnotAgent-generated deterministic ONNX
  graphs and the real SAM Plugin/ONNX Runtime path, but is explicitly non-publishable and is not
  SAM or accuracy evidence;
- EfficientSAM's official Apache-2.0 repository/exporter and author-owned ONNX Space were audited
  at fixed revisions and hashes in [Model Cards](MODEL_CARDS.md);
- those exported tensor names/shapes differ from the SAM ViT-B Plugin Contract, so AnnotAgent uses
  the dedicated `org.annotagent.efficientsam-onnx` Plugin;
- the real `org.annotagent.models.efficientsam-ti-onnx@1.0.0` Bundle is available through the
  trusted local development Catalog and has passed real Rust CPU smoke on macOS ARM64; remote
  Catalog publication remains pending;
- SAM 2 also remains Labs until a complete legally redistributable Rust-ONNX Bundle and matching
  Plugin Contract pass real smoke evidence.

Do not download two files by guessed names. Install the single verified EfficientSAM Catalog Bundle
after reviewing its source and Apache-2.0 license, or import a locally authored `.annotmodel` with
equivalent source, license, Contract, and smoke evidence.
