# RF-DETR Detection Worker

`examples/rfdetr_vision_worker.py` adapts one explicitly configured local RF-DETR checkpoint to
AnnotAgent Detection Worker Protocol v1. It provides only `ObjectDetection`; segmentation,
keypoints and training are not claimed.

## Required local configuration

Install a compatible `rfdetr`, PyTorch/CUDA and Pillow environment outside AnnotAgent. AnnotAgent
does not install the package or download weights. Then provide all immutable model facts:

```bash
export ANNOTAGENT_RFDETR_CHECKPOINT_PATH=/absolute/path/checkpoint_best_total.pth
export ANNOTAGENT_RFDETR_CHECKPOINT_SHA256=<64-hex-sha256>
export ANNOTAGENT_RFDETR_ARCHITECTURE=rfdetr-small
export ANNOTAGENT_RFDETR_MODEL_VERSION=robocup-ball-v1
export ANNOTAGENT_RFDETR_TRAINING_DATASET_VERSION=robocup-ball-v3
export ANNOTAGENT_RFDETR_LABEL_SPACE='["football","robot"]'
python3 examples/rfdetr_vision_worker.py
```

The Worker resolves and hashes the exact local file, requires safe checkpoint loading and CUDA,
and uses RF-DETR's official `from_checkpoint`/`predict` APIs. A full existing checkpoint path keeps
the package loader on the local-file path; the Worker never requests a download.

Without complete metadata or a usable checkpoint, the process still starts and returns
`unavailable` from `/health`; `/v1/capabilities` remains inspectable. It never substitutes fixture
boxes. Use `mock-object-detector` for offline contract testing.

## AnnotAgent Settings

Open **Settings → Detection Workers → RF-DETR Specialist Local** and set the same:

- Worker URL and explicit remote opt-in if applicable;
- architecture and model version;
- checkpoint SHA-256 and training dataset version;
- exact model label space;
- concrete checkpoint weight license.

AnnotAgent refuses to enable the versioned specialist profile until those fields exist. Then use
**Models → Test Worker** to read live health, capability, score semantics and label space before
binding the model to `object_detection.detect`.

## Protocol behavior

The Worker returns normalized `xyxy`, finite RF-DETR scores as `relative_confidence`, model-native
class names, timing and device. The generic Skill applies class mapping and retains independent
evidence. Confidence filtering, class-aware IoU suppression and maximum-result bounds are applied
without changing the source score.

Requests contain bounded inline PNG/JPEG bytes, never a host path. Request/image/header/prediction
payloads are not logged. Timeout is bounded by Rust; cancellation is forwarded and checked before
and after model inference.

## License metadata

The official RF-DETR repository states that the open-source package and Apache-designated models
use Apache 2.0, while Plus components including XL/2XL detection models use PML 1.0. Therefore the
default disabled profile records package provenance but leaves the checkpoint weight permission
unknown until the operator supplies the exact checkpoint terms. The UI metadata is informational,
not legal advice.

- Official repository: https://github.com/roboflow/rf-detr
- Official license: https://github.com/roboflow/rf-detr/blob/develop/LICENSE
