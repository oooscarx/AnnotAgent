# EfficientSAM-Ti ONNX Model Supply Recipe

This Recipe builds the first real, non-Fixture AnnotAgent `PromptedSegmentation` Bundle from the
revision-pinned split ONNX files published in the EfficientSAM authors' linked Hugging Face Space.

The Rust Recipe runner downloads only the three declared HTTPS resources, allows only the declared
Hugging Face CDN redirect host, verifies exact byte lengths and SHA-256 digests, copies the audited
static Contract/test/license files, and packages the finished `.annotmodel`. It never executes a
download, shell command, Python, converter, package manager, or Git client.

From the repository root:

```bash
cargo run -p annotagent -- models recipe audit model-recipes/efficientsam-ti
cargo run -p annotagent -- models recipe fetch model-recipes/efficientsam-ti
cargo run -p annotagent -- models recipe build model-recipes/efficientsam-ti \
  --output dist/model-catalog/bundles/efficientsam-ti-onnx-1.0.0.annotmodel \
  --catalog-entry dist/model-catalog/efficientsam-ti-onnx-1.0.0.json \
  --verification-report dist/model-catalog/verification/efficientsam-ti-onnx-1.0.0.json
cargo run -p annotagent -- models catalog build dist/model-catalog \
  --output dist/model-catalog/catalog.json \
  --catalog-id org.annotagent.catalog.local-models
cargo run -p annotagent -- models catalog add-local dist/model-catalog
```

The generated weights, cache, Bundle and Catalog are ignored by Git. The license and source notice
remain inside the Bundle and license acceptance is required before installation.
