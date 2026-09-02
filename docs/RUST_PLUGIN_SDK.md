# Rust model plugin SDK

`annotagent-plugin-sdk` turns an `ExpertModelPlugin` implementation into a loopback-only,
authenticated HTTP Vision v1 process. Plugin authors implement descriptor/model discovery,
warmup, inference and cancellation; the SDK owns protocol routing and process safety boundaries.

The host writes one bounded `PluginStartupConfig` JSON document to the child standard input and
closes it. The document contains a one-process session token, nonce and isolated state, read-only
weight, cache and temporary directories. The SDK binds `127.0.0.1:0` and prints one JSON handshake
line containing the selected address and nonce, never the token.

All endpoints require bearer authentication:

```text
GET  /health
GET  /v1/capabilities
GET  /v1/models
GET  /v1/contracts
POST /v1/infer
POST /v1/cancel
POST /v1/warmup
POST /v1/shutdown
```

The SDK bounds bodies and responses, validates protocol/model/capability/Artifact inputs, converts
panics into structured failures, tracks cancellation tokens and provides image decoding for PNG and
JPEG byte payloads. `run_conformance` verifies authentication, identity, capability/model/contract
parity, invalid request handling and optional typed sample inference.

`plugins/dummy-detector` is an executable conformance fixture. Its output is explicitly marked as a
fixture and must never be registered as a production-ready model.
