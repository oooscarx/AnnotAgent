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

Official manifests cannot request provider secrets, arbitrary project files, child processes or
non-loopback network access. A manifest is declaration, not availability evidence: runtime
discovery, checkpoint identity, conformance and smoke testing still determine `PluginStatus`.

The stable Rust definitions live in `annotagent-plugin-api`. A `PluginModelReference` freezes the
plugin ID/version, package digest, API/protocol versions, model/revision, checkpoint digest and
capability-contract digest for publication and replay.
