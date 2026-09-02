# Expert Model Plugin SDK

The official SDK is the Rust crate `annotagent-plugin-sdk`. It provides the authenticated loopback
server, health/model/contract/infer/cancel/warmup/shutdown endpoints, input validation, typed Artifact
builders and conformance helper.

See [Writing a Rust Model Plugin](WRITING_A_RUST_MODEL_PLUGIN.md) and [Rust Plugin SDK](RUST_PLUGIN_SDK.md).
The superseded external-worker SDK is archived under
[`docs/legacy/python-workers/`](legacy/python-workers/); it is not part of install, Run, CI or release.
