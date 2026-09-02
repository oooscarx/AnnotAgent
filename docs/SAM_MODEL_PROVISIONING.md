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
- those exported tensor names/shapes differ from the current Plugin Contract, and no reproducible,
  hosted AnnotAgent Bundle exists, so EfficientSAM remains `live-conditional` outside the Catalog;
- SAM 2 also remains Labs until a complete legally redistributable Rust-ONNX Bundle and matching
  Plugin Contract pass real smoke evidence.

Do not download two files by guessed names. Install only a verified compatible Catalog Bundle or
import a locally authored `.annotmodel` with source, license, Contract, and smoke evidence.
