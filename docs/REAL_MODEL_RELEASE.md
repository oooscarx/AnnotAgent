# EfficientSAM-Ti Real Model Release

This document is the release and installation record for the first real, non-Fixture
`PromptedSegmentation` model delivered by AnnotAgent.

## Supported release target

| Target | Provider | Status | Evidence |
| --- | --- | --- | --- |
| macOS ARM64 | Rust ONNX Runtime, CPU | Supported | Package verification, fresh install, real smoke, Workflow, Review, Replay and restart passed |
| Linux x86_64 | Rust ONNX Runtime, CPU | Build-compatible only | Workspace CI/build target exists; no real-model host run or release package was executed in this milestone |
| Other targets | — | Unsupported | No compatible Plugin package is published |

`Supported` is intentionally limited to the machine class on which the real model ran. The model
Bundle itself is platform-independent, but it is usable only with a verified compatible Plugin
package for the host.

## Release identity

- Model: EfficientSAM-Ti split ONNX
- Bundle: `org.annotagent.models.efficientsam-ti-onnx@1.0.0`
- Capability: `PromptedSegmentation`
- Bundle SHA-256: `3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c`
- Plugin: `org.annotagent.efficientsam-onnx@1.0.0`
- Plugin source milestone: local commit `3a0e50b`
- macOS ARM64 Plugin SHA-256:
  `283a9486edaa7b25ae3cf111cd859ca90fa38de488cd3a8c9196d297d10099cd`
- Capability Contract SHA-256:
  `ad3f23abcadb04561dcced33bae9cbfccbce4c13910a715fc964f1281c8f56ee`
- Encoder SHA-256: `84ed466ffcc5c1f8d08409bc34a23bb364ab2c15e402cb12d4335a42be0e0951`
- Decoder SHA-256: `a62f8fa5ea080447c0689418d69e58f1e83e0b7adf9c142e2bd9bcc8045c0b11`
- License SHA-256: `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`
- Model/code license: Apache-2.0 upstream model assets; MIT AnnotAgent Plugin code
- Publisher verification: unsigned Developer Preview (`publisher_verified=false`)

## Build the local release set

Model weights are deliberately Git-ignored. First build the audited Recipe if the generated
Bundle is absent:

```bash
cargo run -p annotagent -- models recipe fetch \
  model-recipes/efficientsam-ti-onnx/recipe.toml
cargo run -p annotagent -- models recipe build \
  model-recipes/efficientsam-ti-onnx/recipe.toml \
  --output dist/model-catalog/bundles/efficientsam-ti-onnx-1.0.0.annotmodel \
  --catalog-entry dist/model-catalog/efficientsam-ti-onnx-1.0.0.json \
  --verification-report dist/model-catalog/verification/efficientsam-ti-onnx-1.0.0.json
cargo run -p annotagent -- models catalog build dist/model-catalog \
  --output dist/model-catalog/catalog.json \
  --catalog-id org.annotagent.catalog.local-models
```

Then prepare and verify the current-platform release assets:

```bash
./scripts/prepare-real-model-release.sh
```

The script stages the immutable macOS ARM64 Plugin candidate that passed the real M3–M5 evidence,
verifies both packages, refuses unexpected pinned digests, and writes `SHA256SUMS` under
`dist/releases/models-v1/`. It does not execute Python and does not download code. Rebuilding the
Plugin after a dependency or source change requires a new Plugin version; the script will not
silently replace the already frozen `1.0.0` bytes.

## GitHub Release asset list

The following files are the exact `models-v1` release assets. Uploading them is an operator step
and was not performed by this task.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `efficientsam-onnx-1.0.0-macos-aarch64.annotplugin` | 8,848,933 | `283a9486edaa7b25ae3cf111cd859ca90fa38de488cd3a8c9196d297d10099cd` |
| `efficientsam-ti-onnx-1.0.0.annotmodel` | 38,577,735 | `3c9004b3f69ce3d48af9f46231fa0cec65b510d4adc05bb5679513a9d5556d6c` |
| `catalog.json` | 2,272 | `a6de92a5017e58a23543326469a2e9e9d21e1866a6f6a277791cc2e2bb0d27d1` |
| `efficientsam-ti-onnx-1.0.0-verification.json` | 2,202 | `3f9344b61ead0876dd20d5525d2e1d9a50dbd5d5e587faccbca97bb6dd1692b6` |
| `SHA256SUMS` | generated | verify with `shasum -a 256 -c SHA256SUMS` |

The Catalog already points to the expected GitHub `models-v1` Bundle URL. Until those assets are
actually uploaded, use the trusted local Catalog below; do not describe the remote Catalog as
available.

## GUI installation

1. Start AnnotAgent with the intended workspace.
2. In **Settings → Expert Model Plugins**, install
   `efficientsam-onnx-1.0.0-macos-aarch64.annotplugin` and explicitly approve its displayed
   permissions and code-license terms.
3. Add `dist/model-catalog` as a trusted local Catalog. This is a development/operator action;
   the normal model picker cannot browse arbitrary disk paths.
4. Open the Prompted Segmentation Plugin card and choose **Install compatible model**.
5. Select **EfficientSAM-Ti ONNX**, review source, platform, size, Bundle digest and the exact
   Apache-2.0 license digest, then accept and install.
6. Keep the installation modal open or return through **View installation**. AnnotAgent verifies
   the Bundle and graph Contract, starts the Rust Plugin, performs the real sample inference, and
   registers the Model Profile only after the Model Instance reaches **Ready**.
7. Return to Pipeline Builder and retry the saved Draft. Choose the concrete Ready local model;
   do not choose a Fixture or a SAM 2 Labs entry.

No Python, model conversion, raw encoder/decoder upload or Provider API key is required.

## CLI installation

From the repository root on macOS ARM64:

```bash
cargo build -p annotagent

./target/debug/annotagent plugin \
  --data-dir ./workspace/.annotagent/plugins \
  install ./dist/releases/models-v1/efficientsam-onnx-1.0.0-macos-aarch64.annotplugin \
  --accept

./target/debug/annotagent models --workspace ./workspace \
  catalog add-local ./dist/model-catalog

./target/debug/annotagent models --workspace ./workspace \
  search prompted-segmentation

./target/debug/annotagent models --workspace ./workspace \
  install org.annotagent.models.efficientsam-ti-onnx@1.0.0 \
  --accept
```

`models install` now runs the declared real smoke automatically. A successful final summary is:

```text
Model instance: <uuid> · Ready
Smoke test: Passed
Fixture only: No
```

The same instance can then be checked explicitly:

```bash
./target/debug/annotagent models --workspace ./workspace test <model-instance-id>
./target/debug/annotagent models --workspace ./workspace doctor <model-instance-id>
```

`doctor` must report `bundle_fixture=false`, `bundle_publishable=true`, a passed smoke result and
`diagnosis=workflow_ready`.

## Publication boundary

This release set is unsigned and locally verified. Before offering the Catalog as a curated remote
source, an operator must create the `models-v1` GitHub Release, upload exactly the listed assets,
verify the remote bytes against `SHA256SUMS`, and only then publish the Catalog endpoint. This
task intentionally did not push Git commits, create a GitHub Release or change publisher trust.
