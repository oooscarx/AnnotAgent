# LocateAnything HTTP Backend

LocateAnything is an optional Model Backend for the generic Open-vocabulary Grounding Skill. It is
not a Core node, Project label, or Runtime branch. The Rust side uses the shared Detection Worker
Protocol v1; the tracked Python adapter is `examples/locate_anything_worker.py`.

## Supported Alpha operations

- open-vocabulary category detection;
- referring-phrase grounding;
- one or multiple text queries;
- normalized bounding boxes;
- valid empty results;
- cooperative cancellation between queries;
- explicit `score=null` / `score_semantics=not_provided`.

OCR, document layout, GUI grounding, point localization, and visual exemplar prompts are not
exposed by this Alpha even if another upstream model revision can perform them. Capability
discovery reports `supports_visual_prompt=false`.

## Local setup

AnnotAgent never downloads code or weights. Obtain the official NVIDIA LocateAnything source and
model through a process permitted by their terms, verify the concrete revision, and keep both
outside this repository. Then install the official runtime dependencies in an isolated Python
environment and start:

```bash
export ANNOTAGENT_LOCATEANYTHING_MODEL_PATH=/absolute/path/to/local/model
export ANNOTAGENT_LOCATEANYTHING_CODE_PATH=/absolute/path/to/NVlabs/Eagle/Embodied
python3 examples/locate_anything_worker.py
```

The Worker listens on `127.0.0.1:8791` by default. In AnnotAgent, open **Settings → Provider &
budgets → Detection Workers**, enable `LocateAnything Local`, save, then open **Settings → Models**
and choose **Test Worker**. Health and capabilities shown after that test are live Worker responses,
not frontend constants.

Without both explicitly configured local paths, the Worker still starts and reports
`status=unavailable`; AnnotAgent startup is unaffected. Use `mock-open-vocabulary` for offline
workflow tests.

## Coordinate and score handling

The official helper parses model coordinates from its native normalized 0–1000 representation to
pixel `xyxy`. The Python adapter divides by the decoded image dimensions and validates normalized
`xyxy`; the Rust adapter validates again and converts to Core normalized `xywh`. Contract tests
cover conversion and out-of-range/reversed responses.

The released model output does not provide a comparable detector confidence through this adapter.
The Worker therefore returns no score. AnnotAgent does not fabricate a percentage or pass these
candidates through an ordinary Confidence Gate; the starter template routes them to Review.

## Security and license boundary

Only bounded inline PNG/JPEG data crosses the protocol. No local image path is accepted. Request,
base64, headers, raw model text, and local paths are not logged. Loopback is the default; remote
Workers require explicit opt-in and HTTPS, and redirects are disabled.

The configured default descriptor links to NVIDIA's official model license and marks commercial
use and redistribution as restricted. The released LocateAnything-3B model is described for
non-commercial research/evaluation. This metadata is informational, applies to the configured
model revision, and is not legal advice. Verify the concrete source and weight terms before use.

## Verification

```bash
python3 -m py_compile examples/locate_anything_worker.py
cargo test -p annotagent-skill-open-vocabulary
cargo test -p annotagent-provider http_vision_worker
```

Real five-image accuracy/latency smoke remains live-conditional until a legal local model and
supported NVIDIA environment are explicitly provided.
