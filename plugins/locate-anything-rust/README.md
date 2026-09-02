# LocateAnything Rust contract package

This package is intentionally marked `unsupported`, not Ready and not live-conditional.

The 2026-09-02 feasibility audit found that NVIDIA's official LocateAnything-3B release uses a
MoonViT vision tower, Qwen2.5 language model, custom Parallel Box Decoding and framework-specific
generation code. The official repository does not publish a
complete ONNX export, Candle/Burn implementation, Rust tokenizer/runtime, or stable native ABI.
A third-party C++/GGUF port exists, but it is not the requested verified Rust path and is therefore
not silently adopted by this official plugin.

The production executable rejects setup with a structured unsupported-runtime error. Registry
installation keeps the exact capability and Artifact contract visible but sets
`UnsupportedPlatform`, disables selection and refuses smoke promotion. A separate Rust scripted
fixture binary exists only for protocol tests; it is never copied into the `.annotplugin` package
and its output is explicitly marked `protocol_fixture`.

There is no legacy scripting fallback, project download, weight download, Provider credential, model accuracy
claim or hidden legacy worker route.
