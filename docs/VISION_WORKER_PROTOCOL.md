# Vision Worker Protocol v1

AnnotAgent uses one versioned HTTP contract for detectors, prompted segmenters, semantic
segmenters, and other vision backends. A worker exposes:

```text
GET  /health
GET  /v1/capabilities
GET  /v1/models
GET  /v1/contracts
POST /v1/infer
POST /v1/cancel
POST /v1/warmup
```

`/health` returns a `VisionModelHealth`. `/v1/capabilities` returns the protocol version,
worker and model identities, supported operations, input and output types, and limits.
`/v1/models` returns immutable model/version/checkpoint/capability and availability summaries.
`/v1/contracts` returns validated Expert Model Manifests with typed Artifact and Prompt contracts,
score semantics and geometry semantics. Multi-model discovery extends the existing single-model
capability response without changing protocol version 1. Warmup is optional.
`/v1/infer` accepts a `VisionInferenceRequest` and returns a `VisionInferenceResponse`.
The canonical serializable contracts live in
`crates/annotagent-core/src/vision_backend.rs`.

Every inference request carries a request ID, operation, run/image/task/node/model scope,
an optional bounded inline image, typed input Artifacts, parameters, and cancellation and
timeout metadata. Every response carries the protocol and model identity, typed Artifacts,
usage, warnings, timings, and an optional structured error. The Rust HTTP adapter rejects
protocol mismatches, unexpected model identities, out-of-scope or invalid Artifacts, and
inline images above its fixed upload bound.

Model and contract discovery reject incompatible protocol versions, empty/duplicate identities,
invalid checkpoint digests, omitted configured models and Worker-connection identity spoofing.
Warmup responses must repeat request and model scope. All discovery and inference responses share
the existing bounded HTTP transport and redirect/remote-origin policy.

## Reference worker

Run the fixture-only worker without model weights:

```bash
python3 examples/http_vision_worker.py
```

In this mode health is `degraded`, inference returns the structured
`weights_unavailable` error, and metadata identifies the process as a fixture. It never
claims to have performed real inference.

To exercise real local object detection, install Pillow and Ultralytics in an isolated
Python environment and point the worker at compatible local weights:

```bash
ANNOTAGENT_MODEL_PATH=/absolute/path/to/model.pt \
  python3 examples/http_vision_worker.py
```

The reference adapter then loads the configured weights and converts real detector output
to normalized `bounding_box` Artifacts. SAM-class prompted segmentation and PIDNet-class
semantic segmentation use the same protocol by advertising `prompted_segmentation` or
`semantic_segmentation` and returning `instance_mask` or `semantic_mask` Artifacts. The
Rust fixture server covers all three capability families and parses every supported
Artifact shape without requiring large model files in the repository.

The reusable Python implementation, generated templates and conformance suite are documented in
`docs/EXPERT_VISION_WORKER_SDK.md`.

GUI-managed Provider secrets belong in the native system credential store; CLI secrets may use an
environment variable. Registry descriptors store only opaque references; workers and adapters must not log
authorization headers or inline image payloads.
