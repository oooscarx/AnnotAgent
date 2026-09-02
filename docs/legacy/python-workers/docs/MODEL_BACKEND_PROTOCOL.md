# Model Backend Protocol

The runtime Model Registry describes executable backend kind, model/version identity, capabilities, accepted inputs, produced Artifact types, price metadata, health, limits, and endpoint or local path. Durable Provider and revisioned Model Profiles live above this runtime registry; only opaque credential references may cross that boundary. Plaintext credential-like configuration is rejected recursively.

Implemented backend classes are:

- `mock`: deterministic typed fixtures for offline acceptance;
- `openai_compatible`: VLM requests with native tool calls or strict JSON-only action promotion;
- `http_json`: external detector, segmenter, classifier, or keypoint worker through the v1 wire protocol;
- deterministic pixel CV: an in-process bounded image algorithm;
- `onnx`: represented as a backend kind and registry contract, but no general ONNX runtime dependency is bundled in Workflow Alpha.

An HTTP worker exposes `GET /health`, `GET /v1/capabilities`, and `POST /v1/infer`. Inference carries protocol/request/run/image/task/node/model identity, a bounded image, typed input Artifacts, parameters, cancellation and timeout metadata. Responses carry typed Artifacts, usage, warnings, timings, and a structured error. Rust rejects protocol/model/scope/type mismatches before persistence or Commit.

The reference Python worker in `examples/http_vision_worker.py` is honest about capability. Without configured local weights it reports degraded health and `weights_unavailable`; with compatible Ultralytics weights it converts real detections to normalized bbox Artifacts. Detector, prompted-segmentation, and semantic-segmentation workers use the same contract.

See [VISION_WORKER_PROTOCOL.md](VISION_WORKER_PROTOCOL.md) for the wire-level fields and worker example.
