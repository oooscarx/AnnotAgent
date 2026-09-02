# Writing a Rust Model Plugin

An AnnotAgent model plugin is a Rust executable plus a versioned manifest. It implements model
capability and Artifact conversion; it does not add a brand-specific branch to Core.

## 1. Create the crate and manifest

Use `annotagent-plugin-sdk` for the server and `annotagent-plugin-api` for manifest/protocol types.
Choose a reverse-domain plugin ID and a package-local model ID. Declare exact input/output
contracts, score and geometry semantics, resource bounds, permissions, targets, license metadata
and weight requirements in `annotagent-plugin.toml`.

Start from `plugins/dummy-detector/` for protocol structure and from the closest official model
plugin for preprocessing/postprocessing. Do not copy model-family logic into
`annotagent-model-runtime-onnx`: that crate remains session- and tensor-oriented.

## 2. Implement typed inference

Implement `PluginImplementation` from the SDK. Validate operation, model ID, Artifact cardinality,
image dimensions, prompts, tensor names/shapes/dtypes and all finite numeric values before native
execution. Convert output to the declared typed `PipelineArtifact`, preserve input/subject/parent
references, restore geometry to the original image coordinate system and validate the response.

Use `annotagent-model-runtime-common` for deterministic resize, letterbox, normalization, layout,
NMS and mask/contour utilities. Use `annotagent-model-runtime-onnx` for native ONNX sessions,
provider selection, shape discovery, warmup and exact checkpoint identity.

## 3. Run development conformance

Build the executable and place it at the manifest entrypoint for the current target, for example:

```text
my-plugin/
├── annotagent-plugin.toml
└── bin/macos-aarch64/my-plugin
```

Then run:

```bash
cargo run -p annotagent -- plugin dev ./my-plugin
```

Development mode reads the local manifest, starts the local Rust binary with Host isolation, runs
conformance and shows bounded protocol logs. It does not install or register the plugin and cannot
be referenced by a Published Workflow. Stop it with Ctrl-C.

Unit tests should cover preprocessing, decode, invalid shapes/dtypes, NaN/infinity, geometry
projection, class mapping, cancellation boundaries and Artifact lineage. Process tests should cover
handshake, missing/bad token, health, models/contracts, malformed/oversized input, inference,
cancel, timeout, crash and clean shutdown.

## 4. Package and verify

Create the target binary tree, licenses, schemas and optional legal tiny fixtures, then run:

```bash
cargo run -p annotagent -- plugin pack ./my-plugin --output my-plugin.annotplugin
cargo run -p annotagent -- plugin verify my-plugin.annotplugin
```

The packer sorts paths, fixes archive timestamps/modes, emits `checksums.json` and produces a stable
SHA-256 for identical contents. It rejects links, traversal, duplicates, unsupported targets,
missing executables and expansion/file-count violations. Do not bundle large checkpoints.

## 5. Prove readiness

Install with explicit permission/license approval, provision every declared component with its
expected digest, and run `plugin test`. Contract presence alone is insufficient. Only an installed,
enabled version with complete weight identity and a passed conformance/smoke report becomes Ready.
Record whether a test used a synthetic fixture, a tiny runtime graph or real model weights.

## 6. Integrate by capability

Add no brand enum or conditional to Core. The plugin manifest projects a Model Profile into the
Registry. Existing generic Classification, Detection or Segmentation Skill runners consume the
capability and Artifact contract. Verify static validation, Draft Dry Run, publication snapshot,
Run, Replay, cache identity, Review and uninstall protection.

Before submitting a package, run:

```bash
./scripts/check-rust-plugin-boundary.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The official active path must contain no scripting-runtime implementation or launcher. Native
libraries are allowed only behind maintained Rust bindings and plugin-local code.
