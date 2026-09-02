# Migration from external vision workers to Rust model plugins

| Existing active/reference path | Capability | Wire input/output | Existing evidence | Rust target | Initial state |
| --- | --- | --- | --- | --- | --- |
| `examples/http_vision_worker.py` | Object detection | Image → bounding-box artifacts | provider protocol tests | `org.annotagent.yolo-onnx` | migrate after parity |
| `examples/sam2_vision_worker.py` | Prompted segmentation | Image + box/point prompts → MaskSet | Python SDK and RoboCup integration tests | `org.annotagent.sam-onnx` | migrate after parity |
| `examples/rfdetr_vision_worker.py` | Object detection | Image → DetectionSet-compatible artifacts | adapter-only tests | `org.annotagent.rfdetr-onnx` | live-conditional |
| `examples/locate_anything_worker.py` | Open-vocabulary detection, phrase grounding | Image + queries → DetectionSet | adapter-only tests | `org.annotagent.locate-anything-rust` | feasibility required |
| `sdk/python/annotagent_vision_worker` | Worker SDK | protocol/manifest/conformance | Python unit tests | `annotagent-plugin-sdk` | replace in official path |
| `web/e2e/fixtures/expert_vision_worker.py` | E2E fixture | protocol fixture | browser E2E | Rust dummy plugin | replace in release E2E |

Migration rules:

1. Historical published workflows and runs are never rewritten.
2. A legacy HTTP binding remains an `External Legacy Model Profile`; it is not an installed plugin.
3. A new draft may bind an equivalent Rust plugin only after installation, exact checkpoint
   provisioning, conformance, dry run and explicit publication.
4. Once contract and artifact parity exist, official UI, examples, CI and release packaging stop
   referencing the old workers. Historical documentation moves under `docs/legacy/`.
