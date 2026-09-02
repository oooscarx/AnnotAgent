# Model Bundles

An AnnotAgent Model Bundle (`.annotmodel`) is a data-only, immutable package. It carries model
files, tensor Contracts, transforms, source/export provenance, an exact license, checksums, and a
fixed smoke suite. It never carries executable code. Executable inference belongs to a separately
installed `.annotplugin`.

```text
.annotplugin (code) + .annotmodel (data) + execution provider
  → compatible Model Instance
  → Contract inspection
  → fixed smoke test
  → Ready Model Instance
  → selectable Model Profile only when the Bundle is also publishable
```

This separation lets one Plugin support multiple immutable model versions and lets one Bundle be
audited without running it. `Installed`, `Preparing`, and even a Fixture `Ready` state do not imply
workflow availability. Publication requires an enabled, publishable Bundle and a smoke-tested Ready
instance with matching Plugin, Contract, file, and execution-provider identity.

Use **Settings → Expert Model Plugins → Install compatible model**, or the `annotagent models`
commands described in [Model Provisioning](MODEL_PROVISIONING.md). Format details are in
[`.annotmodel` Format](ANNOTMODEL_FORMAT.md).
