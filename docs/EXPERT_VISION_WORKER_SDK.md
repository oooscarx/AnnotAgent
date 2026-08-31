# Expert Vision Worker SDK

The Python SDK lives in `sdk/python/annotagent_vision_worker`. It supplies:

- strict Pydantic request/response and Expert Model Manifest models;
- FastAPI helpers for health, capability/model/contract discovery, inference, cancellation and
  optional warmup;
- bounded inline image decoding with no host-path input;
- normalized coordinate validation;
- bounding-box and COCO-RLE instance-mask Artifact helpers;
- cooperative request cancellation and stable error mapping;
- reusable black-box conformance checks;
- generic and preset scaffolding.

Install and test without model weights:

```bash
uv run --project sdk/python --extra test python -m pytest sdk/python/tests
```

Generate a generic detector:

```bash
cargo run -p annotagent -- worker scaffold \
  --name my-detector \
  --capability object_detection
```

Generate preset adapter templates:

```bash
cargo run -p annotagent -- worker scaffold --preset sam2
cargo run -p annotagent -- worker scaffold --preset yolo
cargo run -p annotagent -- worker scaffold --preset rfdetr
cargo run -p annotagent -- worker scaffold --preset locate-anything
cargo run -p annotagent -- worker scaffold --preset pidnet
cargo run -p annotagent -- worker scaffold --preset grounding-dino
```

Each scaffold contains `app.py`, `model.py`, `manifest.yaml`, `requirements.txt`, tests and a
README. It begins as `missing_weights` or `unconfigured`. The template never downloads a model and
its placeholder inference returns a structured `weights_unavailable` error. A developer completes
model/checkpoint/dataset/license identity, implements explicit local loading, passes conformance,
then uses AnnotAgent's explicit sample test before registration.

Adding an unknown detector therefore changes only a Worker directory/Manifest. It does not modify
Core, add a node kind or introduce a Runtime model-brand branch.
