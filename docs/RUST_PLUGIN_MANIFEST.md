# Rust model plugin manifest

Every `.annotplugin` package contains `annotagent-plugin.toml` with schema version `1`. The parser
rejects unknown fields and validates the package before any executable starts.

Required sections describe:

- reverse-domain plugin ID and semantic version;
- the `native_rust_process` runtime and `http-vision-v1` protocol;
- application version range, targets and accelerators;
- least-privilege permissions and bounded resources;
- capability-oriented models with typed input/output Artifact contracts;
- weight provisioning, immutable SHA-256 recipes and license metadata.

`implementation_status` distinguishes `runnable`, `live_conditional` and `unsupported` package
versions. The default preserves existing runnable manifests. Live-conditional packages contain an
executable Rust inference path but still need external checkpoint evidence. Unsupported packages
remain `UnsupportedPlatform`, disabled and unable to provision weights or record a readiness smoke;
their contracts may be inspected, but they cannot be selected or published.

Multi-file models declare named weight components under `weights.components`. Each component binds
one model ID to one controlled filename and may include an expected SHA-256. Local provisioning
preserves the original filename for audit, copies bytes under the controlled filename, hashes each
component and requires every declared component before the model can leave `NeedsWeights`.
Published identity uses the single checkpoint hash unchanged for legacy one-file models and a
deterministic ordered aggregate hash for multi-file models. Fixed recipes identify both the model
and component whenever a component manifest is present.

Official manifests cannot request provider secrets, arbitrary project files, child processes or
non-loopback network access. A manifest is declaration, not availability evidence: runtime
discovery, checkpoint identity, conformance and smoke testing still determine `PluginStatus`.

The stable Rust definitions live in `annotagent-plugin-api`. A `PluginModelReference` freezes the
plugin ID/version, package digest, API/protocol versions, model/revision, checkpoint digest and
capability-contract digest for publication and replay.
