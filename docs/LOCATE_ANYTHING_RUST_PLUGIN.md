# LocateAnything Rust feasibility audit

Audit date: 2026-09-02.

`org.annotagent.locate-anything-rust@1.0.0` expresses the open-vocabulary detection and phrase
grounding protocol contract, but is intentionally `UnsupportedPlatform` and disabled.

## Official release findings

The official [NVIDIA model card](https://huggingface.co/nvidia/LocateAnything-3B) and
[NVlabs/Eagle implementation](https://github.com/NVlabs/Eagle/tree/main/Embodied) describe a 3B
vision-language model composed of MoonViT, Qwen2.5, an MLP projector and custom Parallel Box
Decoding with hybrid generation. The official inference path uses PyTorch/Transformers plus custom
generation and processing code. The audited release does not provide:

- a complete supported ONNX export executable by AnnotAgent's Rust ONNX Runtime;
- a Candle or Burn implementation;
- a stable Rust tokenizer/model runtime or official native ABI.

A third-party [`locate-anything.cpp`](https://github.com/mudler/locate-anything.cpp) GGUF/C++ port
exists. It is useful feasibility evidence, but it is not the requested verified Rust-native model
path and is not silently linked into an official AnnotAgent plugin.

## Product behavior

- the package installs with exact Manifest/package identity as `UnsupportedPlatform`;
- it is disabled and never appears as a selectable Ready model;
- enable, weight provisioning and readiness-smoke promotion cannot bypass the unsupported state;
- the production Rust executable fails setup with a structured unsupported-runtime error;
- no legacy HTTP/Python Worker is started as a fallback;
- no repository, weight, tokenizer or Python environment is downloaded.

A separate scripted Rust fixture executable proves HTTP Vision authentication, capability and typed
`DetectionSet` transport in tests. The fixture is not copied into the `.annotplugin` package, marks
every response `protocol_fixture=true` and `real_inference=false`, supplies no model score and never
produces readiness evidence.

LocateAnything may become live-conditional only in a later immutable plugin version after a complete
legal Rust-callable runtime path and an actual checkpoint smoke are available.
