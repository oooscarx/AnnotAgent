# Model Plugin Process Protocol

Rust plugins extend HTTP Vision v1 over an authenticated, Host-created loopback process. The Host
sends one startup configuration on standard input; the process replies with a nonce-bound handshake
and serves versioned health, capabilities, models, contracts, infer, cancel, warmup and shutdown
operations. Every request requires the session token.

See [Plugin SDK](RUST_PLUGIN_SDK.md), [Security](RUST_PLUGIN_SECURITY.md) and
[Manifest](RUST_PLUGIN_MANIFEST.md). Historical external HTTP bindings remain readable under
**Legacy HTTP** and are documented only in [`docs/legacy/python-workers/`](legacy/python-workers/).
