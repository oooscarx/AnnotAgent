# Rust Expert Model Plugin Alpha — five-minute demo

This course demo uses the Rust Dummy Detector to prove package and process behavior offline. It is
protocol evidence, not a model-accuracy claim. No credential, external checkpoint or scripting
runtime is required.

## Prepare the package

```bash
cargo build -p annotagent-plugin-dummy-detector
mkdir -p /tmp/annotagent-dummy/bin/$(rustc -vV | awk '/host:/{print $2}')
```

The manifest target names use AnnotAgent's `macos-aarch64`, `linux-x86_64`, or
`windows-x86_64` convention rather than the Rust host triple. For a classroom run, copy
`plugins/dummy-detector/annotagent-plugin.toml` into a temporary package directory and copy the
built executable into `bin/<annotagent-target>/annotagent-plugin-dummy-detector`. Do not change the
repository package source.

Then:

```bash
cargo run -p annotagent -- plugin pack /tmp/annotagent-dummy \
  --output /tmp/dummy-detector.annotplugin
cargo run -p annotagent -- plugin verify /tmp/dummy-detector.annotplugin
```

## Inspect the product lifecycle

Start AnnotAgent:

```bash
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

Open **Settings → Expert Model Plugins**, install `/tmp/dummy-detector.annotplugin`, review the
requested loopback/cache/temporary-image permissions and license, then run **Test**. Inspect the
exact package digest, Plugin API/protocol, capability contract, process health and test evidence.
The Dummy model becomes Ready without weights because its manifest declares none.

In a Project Automation Draft, select its Object Detection capability and run selected-image Dry
Run. Publish only that tested Draft. The immutable version now freezes the package/model/contract
identity. Open the Run Debug inspector to see the DetectionSet and lineage.

## Demonstrate isolation and protection

- Stop or crash the foreground test process: the Host records a structured failure and the server
  remains alive.
- Install a second package version: both versions coexist; the old Published Workflow retains v1.
- Try to uninstall v1 while referenced: Registry rejects it and lists the Workflow reference.
- Disable a version: it remains inspectable but disappears from runnable model choices.
- Open a Project that needs Prompted Segmentation without a Ready segmenter: Pipeline Builder saves
  an unresolved Draft and suggests setup; it does not install anything.

For real inference evidence, the opt-in YOLOX Nano test uses an explicitly supplied official
checkpoint and sample. The repository does not ship those files. SAM, PIDNet and RF-DETR real-weight
smokes remain live-conditional, and LocateAnything remains UnsupportedPlatform.

Finish with the executable release checks:

```bash
./scripts/check-rust-plugin-boundary.sh
cargo test -p annotagent-plugin-api
cargo test -p annotagent-plugin-sdk
cargo test -p annotagent-plugin-host
cargo test -p annotagent-model-runtime-onnx
```
