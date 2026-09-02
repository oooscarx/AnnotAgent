# Model Bundle Provisioning Alpha Demo

Build the product, then inspect the offline Catalog without any credentials or external weights:

```bash
cargo build -p annotagent
demo_workspace=$(mktemp -d)
target/debug/annotagent models --workspace "$demo_workspace" catalog
target/debug/annotagent models --workspace "$demo_workspace" search prompted-segmentation
target/debug/annotagent models --workspace "$demo_workspace" show \
  org.annotagent.models.fixture-prompted-segmentation@1.0.0
target/debug/annotagent models --workspace "$demo_workspace" install \
  org.annotagent.models.fixture-prompted-segmentation@1.0.0
target/debug/annotagent models --workspace "$demo_workspace" list
```

The final output must say setup is still required when no compatible Plugin is installed. It must
also say `fixture=true` and `publishable=false`; never present the fixture as SAM or workflow-ready.

The release integration test installs the actual SAM Plugin package into an isolated Registry and
executes the full path:

```bash
cargo test -p annotagent-plugin-sam-onnx \
  --test fixture_bundle_workflow --all-features -- --nocapture
```

It covers Pack → Verify → Install → Contract → Bind → real Rust Plugin/ORT inference → Smoke →
Ready fixture instance → Mask → BBox → Geometry Quality → Disable → Remove. Use the GUI Plugin page
for the same review/install/test state machine with a real publishable Bundle when one is available.
