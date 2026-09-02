# Migration from external vision workers to Rust model plugins

| Existing active/reference path | Capability | Wire input/output | Existing evidence | Rust target | Initial state |
| --- | --- | --- | --- | --- | --- |
| `docs/legacy/python-workers/examples/http_vision_worker.py` | Object detection | Image → bounding-box artifacts | archived only | `org.annotagent.yolo-onnx` | Rust replacement complete; official path removed |
| `docs/legacy/python-workers/examples/sam2_vision_worker.py` | Prompted segmentation | Image + box/point prompts → MaskSet | archived only | `org.annotagent.sam-onnx` | Rust contract/process complete; real two-file smoke live-conditional |
| `docs/legacy/python-workers/examples/rfdetr_vision_worker.py` | Object detection | Image → DetectionSet-compatible artifacts | archived only | `org.annotagent.rfdetr-onnx` | Rust ONNX implementation live-conditional; real export smoke pending |
| `docs/legacy/python-workers/examples/locate_anything_worker.py` | Open-vocabulary detection, phrase grounding | Image + queries → DetectionSet | archived only | `org.annotagent.locate-anything-rust` | UnsupportedPlatform after official-runtime audit; no fallback |
| `docs/legacy/python-workers/sdk-python` | Worker SDK | protocol/manifest/conformance | archived and excluded from CI | `annotagent-plugin-sdk` | Rust replacement complete |
| `docs/legacy/python-workers/web-e2e/expert_vision_worker.py` | Browser protocol fixture | HTTP Vision v1 | archived and excluded from CI | `annotagent-e2e-fixture` | Rust replacement complete |

PIDNet compatibility now targets `org.annotagent.pidnet-onnx`: its Rust process, semantic tensor
decode and typed `SemanticMask` are complete, while the old external preset stays active only until
M8 product lifecycle wiring and M9 migration remove that reference. No old worker result is reused
as plugin smoke evidence.

Migration rules:

1. Historical published workflows and runs are never rewritten.
2. A legacy HTTP binding remains an `External Legacy Model Profile`; it is not an installed plugin.
3. A new draft may bind an equivalent Rust plugin only after installation, exact checkpoint
   provisioning, conformance, dry run and explicit publication.
4. Once contract and artifact parity exist, official UI, examples, CI and release packaging stop
   referencing the old workers. Historical material lives only under `docs/legacy/`.

The shipped workspace has no default legacy HTTP model. Existing serialized endpoint settings still
deserialize, remain inspectable under **Legacy HTTP**, and can execute their historical immutable
Workflow without being represented as a plugin. A migration is an explicit product sequence named
**Create Rust plugin binding**:

1. install and test an equivalent capability in **Expert Model Plugins**;
2. clone the historical Published Workflow to an editable Draft;
3. replace the external binding with the exact Ready plugin model;
4. run the Draft against selected Project images;
5. publish only after the Dry Run passes.

The original Published Workflow and Run records are unchanged. This operation is performed through
the existing Clone, model-binding and Dry Run controls; AnnotAgent never rewrites a historical
binding automatically.

Release enforcement is executable: `scripts/check-rust-plugin-boundary.sh` rejects scripting-runtime
files in `apps/`, `crates/`, `plugins/`, `examples/`, `scripts/` and `web/e2e`, rejects Rust child
process launches of scripting runtimes, and rejects scripting-runtime setup in release scripts.
