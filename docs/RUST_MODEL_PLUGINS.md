# Rust Expert Model Plugins

AnnotAgent runs native expert vision models as authenticated, isolated Rust child processes. Core
selects only a capability and typed Artifact contract; model-family code stays in the package.

```text
Workflow node
  → exact PluginModelSnapshot
  → Rust Plugin Host
  → authenticated HTTP Vision v1 process on 127.0.0.1
  → native ONNX Runtime or another declared native runtime
  → typed Artifact with lineage
```

## What is shipped

| Package | Capability | Implementation truth |
| --- | --- | --- |
| `org.annotagent.dummy-detector` | Object detection | Rust conformance fixture; not accuracy evidence |
| Generic ONNX identity fixture | tensor runtime | Real native CPU ONNX execution; not an expert model |
| `org.annotagent.yolo-onnx` | Object detection | Rust YOLOX Nano path; official checkpoint smoke passed explicitly |
| `org.annotagent.sam-onnx` | Prompted segmentation | Rust encoder/decoder path; real external weights live-conditional |
| `org.annotagent.pidnet-onnx` | Semantic segmentation | Rust ONNX path; real external weights live-conditional |
| `org.annotagent.rfdetr-onnx` | Object detection | Rust official-export path; real external export live-conditional |
| `org.annotagent.locate-anything-rust` | Open vocabulary / phrase grounding | Unsupported until a complete audited Rust-callable runtime exists |

Weights are not committed. New setup uses an independent verified `.annotmodel` Bundle rather than
raw weight upload. A contract-complete Plugin is not Ready for a Workflow until the exact Bundle
files, license, ONNX tensor Contract and installed-process smoke/conformance report pass. See
[Model Bundles](MODEL_BUNDLES.md) and [Model Provisioning](MODEL_PROVISIONING.md).

## Install in the product

Open **Settings → Expert Model Plugins** and choose **Install package**. The wizard streams a local
`.annotplugin`, verifies its deterministic archive and checksums, shows publisher, target,
permissions and licenses, and installs only after explicit approval. Then choose **Install
compatible model**, review the Catalog-pinned Bundle and license, and run its fixed test. The model
selector includes only enabled, publishable, Ready instances. Raw components remain a labelled
legacy migration surface only.

The Registry reports these states without guessing:

- `Installed`: package is valid, but readiness evidence is incomplete.
- `NeedsWeights`: one or more required checkpoint components are missing.
- `Ready`: exact weights, process discovery, contracts and smoke/conformance all passed.
- `Disabled`, `FailedSmokeTest`, `UnsupportedPlatform`: visible but not runnable.

The equivalent CLI is:

```bash
cargo run -p annotagent -- plugin inspect model.annotplugin
cargo run -p annotagent -- plugin install model.annotplugin --accept
cargo run -p annotagent -- plugin provision org.example.detector \
  --version 1.0.0 --model detector --weights /path/to/model.onnx --sha256 <digest>
cargo run -p annotagent -- plugin test org.example.detector --version 1.0.0
cargo run -p annotagent -- plugin enable org.example.detector --version 1.0.0
```

`update` installs another version side by side. `references` explains which immutable Workflows use
one version, and `uninstall` refuses to remove a referenced version.

## Workflow identity and execution

Publishing freezes plugin ID/version, package digest, Plugin API, protocol, package-local model ID,
Model Profile revision, checkpoint digest, capability-contract digest and capabilities. A new Run
fails closed if any frozen identity is missing or different. Replay resolves the same snapshot and
does not silently upgrade to a newer package. Historical records remain readable even when a local
executable is unavailable.

Pipeline Builder receives credential-free installed-model manifests. It can inspect capability,
contract, readiness and setup alternatives, but cannot install packages, accept licenses, provision
weights, reveal secrets or start arbitrary programs. A missing model produces an unresolved Draft.

## Security boundary

The Host clears the child environment, supplies private state/cache/temporary/weight directories,
uses a one-use nonce and random session token, requires authentication on every request, accepts
only a loopback handshake, bounds request/response/log sizes, and terminates a crashed or timed-out
child without taking down Core. Provider credentials, the SQLite connection and arbitrary Project
paths are not passed to plugins.

This is process isolation, not a universal OS sandbox. See [Plugin Security](RUST_PLUGIN_SECURITY.md)
for the exact claim and [Known Limitations](execution/RUST_PLUGIN_KNOWN_LIMITATIONS.md) for what is
not implemented.

## Rust-only proof

Official package, Host, SDK, plugins, browser protocol fixture and active tests are Rust. Native
inference uses the Rust ONNX Runtime binding. `scripts/check-rust-plugin-boundary.sh` rejects active
scripting-runtime files, child-process launch sites and release setup commands. Historical sources
exist only under `docs/legacy/python-workers/` and are not compiled, packaged, started or tested.

See [Writing a Rust Plugin](WRITING_A_RUST_MODEL_PLUGIN.md), [Manifest](RUST_PLUGIN_MANIFEST.md),
[Packaging](RUST_PLUGIN_PACKAGING.md), [Versioning](RUST_PLUGIN_VERSIONING.md) and the
[five-minute demo](DEMO_RUST_PLUGIN_ALPHA.md).
