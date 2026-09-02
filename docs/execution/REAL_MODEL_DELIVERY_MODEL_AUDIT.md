# Real Prompted-Segmentation Candidate Audit

Last updated: 2026-09-03 CST

This ledger uses official repositories, official releases, or model files linked by the model
authors. A candidate accepted for delivery is not `Supported`: support still requires a verified
non-Fixture Bundle, real Rust smoke test, selectable Model Instance, Workflow run, and Replay.

## Decision summary

| Candidate | Delivery decision | Reason |
| --- | --- | --- |
| EfficientSAM-Ti split ONNX | **Accepted for the first real Bundle** | Author-linked, revision-pinned split ONNX; Apache-2.0; box prompts; both real graphs load in the current Rust Runtime on macOS ARM64; approximately 41.4 MB total. |
| MobileSAM ViT-T | Rejected for this delivery | Official source distributes a PyTorch checkpoint and a Python export path, not a fixed, complete encoder/decoder ONNX package suitable for the no-conversion user path. |
| Meta SAM 1 ViT-B | Rejected for this delivery | Official ONNX tooling exports the prompt encoder/mask decoder only; the heavyweight image encoder still requires Python/PyTorch preprocessing or a maintainer conversion artifact that is not officially distributed. |
| Meta SAM 2.1 Tiny | Labs only | Official path is a 156 MB PyTorch checkpoint plus Python/PyTorch; no official complete, fixed ONNX package was established. |

## Candidate A — EfficientSAM-Ti split ONNX

| Field | Audited evidence |
| --- | --- |
| Model family / exact variant | EfficientSAM / Ti (`build_efficient_sam_vitt`) |
| Official repository | `https://github.com/yformer/EfficientSAM`, inspected at `d525f622e6f640acf5a0fc37c7ca1f243da5bde0` |
| Official model source | The official README links author Yunyang Xiong's `yunyangx/EfficientSAM` Hugging Face Space for the separate encoder and decoder ONNX files. |
| Fixed model revision | `d8dbb1eee73bfb3392aa6f6e8944aeb13f3f4036` |
| Model file format / count | Two ONNX graphs, upstream exporter opset 17: `efficientsam_ti_encoder.onnx` and `efficientsam_ti_decoder.onnx` |
| Exact sizes | encoder 24,799,761 bytes; decoder 16,565,728 bytes; total 41,365,489 bytes |
| Exact SHA-256 | encoder `84ed466ffcc5c1f8d08409bc34a23bb364ab2c15e402cb12d4335a42be0e0951`; decoder `a62f8fa5ea080447c0689418d69e58f1e83e0b7adf9c142e2bd9bcc8045c0b11` |
| Code license | Apache-2.0 in the official repository. Frozen license bytes SHA-256: `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`. |
| Weight license | The author-hosted Space declares `apache-2.0` at the fixed model revision. The Bundle will reproduce the exact upstream Apache-2.0 notice and source/revision provenance. |
| Redistribution / commercial use | Apache-2.0 permits redistribution and commercial use subject to its notice, license-copy, modification-notice, patent, trademark, and warranty terms. License acceptance remains explicit in AnnotAgent. |
| Encoder input | `batched_images`: `f32 [batch, 3, height, width]` (dynamic batch/height/width) |
| Encoder output | `image_embeddings`: `f32 [batch, 256, *, *]`; a 1024-square encoder input produces `[1,256,64,64]`. |
| Decoder inputs | `image_embeddings: f32 [batch,256,64,64]`; `batched_point_coords: f32 [1,1,num_points,2]`; `batched_point_labels: f32 [1,1,num_points]`; `orig_im_size: i64 [2]`. |
| Decoder outputs | `output_masks` logits, `iou_predictions`, and a third low-resolution logits tensor whose exported name is `onnx::Shape_1830`. For one query, upstream code defines three mask candidates and chooses by predicted IoU. |
| Box / point prompts | Both. The author Space encodes a box as top-left/bottom-right points with labels `2` and `3`; ordinary positive/negative point labels are `1` and `0`; `-1` pads unused prompt slots. |
| Expected preprocessing | Decode RGB; preserve source dimensions; convert to NCHW `f32` in `[0,1]`. The exported encoder graph performs resize to 1024×1024 and ImageNet mean/std normalization. Decoder receives prompt coordinates in the original input pixel space plus original `[height,width]`. |
| Expected postprocessing | Select the highest finite IoU candidate, threshold mask logits at `0`, validate finite/non-empty/bounded geometry, encode the mask, then derive the tight bounding box from foreground pixels. |
| Current Rust Runtime compatibility | **Graph load passed.** Both audited files loaded with `annotagent-model-runtime-onnx` / ORT CPU on the current host and descriptors matched the values above. Real inference remains an M3 gate. |
| Existing Plugin compatibility | **Incompatible.** `org.annotagent.sam-onnx` expects the SAM 1 split contract (`input_image`; five decoder inputs including `mask_input`/`has_mask_input`). EfficientSAM requires its own Plugin. |
| macOS ARM64 CPU feasibility | Accepted. Both real graphs load on Darwin ARM64; compact 41.4 MB asset size. M3 must still measure real encoder/decoder latency and mask output. |
| Linux x86_64 feasibility | ONNX is platform-neutral and the repository already supports ORT CPU there, but no physical Linux host was available for M1. Linux remains CI/runtime verification, not a claimed measured pass. |
| Decision | **Accepted for delivery** as `org.annotagent.models.efficientsam-ti-onnx` with a dedicated `org.annotagent.efficientsam-onnx` Plugin exposing the generic `PromptedSegmentation` capability. |

### Reproducible source URLs

- Encoder: `https://huggingface.co/spaces/yunyangx/EfficientSAM/resolve/d8dbb1eee73bfb3392aa6f6e8944aeb13f3f4036/efficientsam_ti_encoder.onnx?download=true`
- Decoder: `https://huggingface.co/spaces/yunyangx/EfficientSAM/resolve/d8dbb1eee73bfb3392aa6f6e8944aeb13f3f4036/efficientsam_ti_decoder.onnx?download=true`
- Export contract: `https://github.com/yformer/EfficientSAM/blob/d525f622e6f640acf5a0fc37c7ca1f243da5bde0/export_to_onnx.py`
- ONNX example: `https://github.com/yformer/EfficientSAM/blob/d525f622e6f640acf5a0fc37c7ca1f243da5bde0/EfficientSAM_onnx_example.py`
- License: `https://github.com/yformer/EfficientSAM/blob/d525f622e6f640acf5a0fc37c7ca1f243da5bde0/LICENSE`

The exact ONNX files were downloaded only into a temporary audit directory. Their local sizes and
hashes reproduced the values above. They did not enter Git or the workspace.

## Candidate B — MobileSAM ViT-T

| Field | Audited evidence |
| --- | --- |
| Model family / exact variant | MobileSAM / TinyViT (`vit_t`) |
| Official repository | `https://github.com/ChaoningZhang/MobileSAM`, inspected at `f706ad9c4eb7f219c00d9050e46328518ffb65d2` |
| Official model source | The official README links `mobile_sam.pt` through Google Drive. The link is not an immutable revision-addressed release asset. |
| Format / required files | PyTorch checkpoint. The official ONNX flow invokes `python scripts/export_onnx_model.py`; it does not directly distribute a fixed complete Rust-ready pair. |
| Model size | Official materials describe a 9.66M-parameter model. No immutable checkpoint byte length and digest are published by the repository, so this audit does not invent one. |
| Code / weight license | Repository declares Apache-2.0; frozen repository license SHA-256 is `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`. The Drive-hosted weight lacks a separately versioned license artifact. |
| Redistribution / commercial notes | Apache-2.0 terms are compatible, but immutable weight provenance and a finished redistributable ONNX artifact are not established strongly enough for this supply path. |
| Input / output contract | SAM-compatible image encoder plus prompt/mask decoder. Official export produces the prompt/mask ONNX contract from a checkpoint; image embedding generation remains a separate model step. |
| Box / point support | Both through the SAM API. |
| Expected preprocessing | SAM resize-longest-side, normalization and square padding; exact exported encoder is not supplied. |
| Rust Runtime compatibility | A maintainer could potentially produce compatible ONNX graphs, but the task forbids making the normal user run Python conversion. No finished audited input exists for the Rust Runtime. |
| Existing Plugin compatibility | The decoder family is close to `org.annotagent.sam-onnx`, but a matching, hashed encoder+decoder pair is absent. |
| macOS ARM64 / Linux x86_64 | CPU use is reported upstream, but the audited official distribution does not provide the package needed to verify AnnotAgent on either target. |
| Decision | **Rejected for the first delivery.** Reconsider only after maintainers publish immutable, licensed, fully paired ONNX assets with exact hashes. |

## Candidate C — Meta SAM 1 ViT-B

| Field | Audited evidence |
| --- | --- |
| Model family / exact variant | Segment Anything / ViT-B `sam_vit_b_01ec64.pth` |
| Official repository | `https://github.com/facebookresearch/segment-anything`, inspected at `dca509fe793f601edb92606367a655c15ac00fdf` |
| Official model source | `https://dl.fbaipublicfiles.com/segment_anything/sam_vit_b_01ec64.pth` |
| Format / required files | Official 375,042,383-byte PyTorch checkpoint. The official Python exporter creates only the prompt encoder/mask decoder ONNX graph. |
| Code / weight license | Official README states the model is Apache-2.0; frozen repository license SHA-256 is `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`. |
| Redistribution / commercial use | Apache-2.0 compatible subject to its terms. This does not solve the missing official complete ONNX distribution. |
| Input / output contract | Image encoder creates `[1,256,64,64]` embedding. Exported decoder consumes embedding, point coordinates/labels, optional prior-mask inputs and original size; returns masks, IoU predictions and low-resolution logits. |
| Box / point support | Both. |
| Expected preprocessing | Resize longest side to 1024, RGB normalization, square padding, coordinate transform, then image encoder inference. |
| Rust Runtime compatibility | The current Rust Plugin implements this family, but official source requires Python/PyTorch to produce the two ONNX files it needs. No official fixed pair was found. |
| Existing Plugin compatibility | Contract-compatible in intent, asset-incomplete in official distribution. |
| macOS ARM64 / Linux x86_64 | ORT could run a correctly exported pair, but this audit cannot claim platform success without the missing immutable encoder artifact. The 375 MB checkpoint is also much larger than EfficientSAM-Ti. |
| Decision | **Rejected for the first delivery.** It would require a separately controlled maintainer conversion/release pipeline and provenance for generated ONNX files. |

## Candidate D — Meta SAM 2.1 Tiny

| Field | Audited evidence |
| --- | --- |
| Model family / exact variant | SAM 2.1 / Hiera Tiny |
| Official repository | `https://github.com/facebookresearch/sam2`, inspected at `2b90b9f5ceec907a1c18123530e92e794ad901a4` |
| Official model source | `https://dl.fbaipublicfiles.com/segment_anything_2/092824/sam2.1_hiera_tiny.pt` |
| Format / size | 156,008,466-byte PyTorch checkpoint plus configuration; no official complete fixed ONNX release established. |
| Code / weight license | Official README states checkpoints and code are Apache-2.0; frozen repository license SHA-256 is `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`. |
| Redistribution / commercial use | Apache-2.0 compatible subject to its terms. |
| Input / output contract | Official image predictor accepts point/box prompts and emits masks and scores through PyTorch. There is no official frozen ONNX graph contract to bind. |
| Box / point support | Both for static images. |
| Expected preprocessing | Defined by the Python/PyTorch SAM2 image predictor and configuration, not by a directly supplied ONNX artifact. |
| Rust Runtime / Plugin compatibility | Not compatible with the current ONNX Runtime path or SAM 1 Plugin. The pre-existing local `.pt` file cannot be consumed by either. |
| macOS ARM64 / Linux x86_64 | Official instructions require Python ≥3.10, PyTorch ≥2.5.1 and torchvision. That violates the normal-user Rust-only requirement on both targets. |
| Decision | **Labs.** Do not present as available until a separately audited Rust-ready Bundle and dedicated Contract exist. |

## M1 verification evidence

- Candidate asset sizes and SHA-256 values were reproduced locally with `shasum -a 256`.
- The fixed Hugging Face revision reports public, non-gated access and `apache-2.0` metadata.
- `annotagent-model-runtime-onnx` loaded both EfficientSAM-Ti files through ORT CPU on Darwin ARM64.
- The inspected descriptors prove the existing SAM 1 Plugin cannot truthfully bind these graphs.
- No real inference success is claimed yet; that remains release-blocking in M3.
