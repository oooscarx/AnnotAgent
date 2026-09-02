# Real Prompted-Segmentation Candidate Audit

Last updated: 2026-09-03 CST

This ledger records only official or author-linked primary sources. A candidate is not Supported
until exact files, licenses, graph Contract and real Rust inference all pass.

## Candidate A — EfficientSAM-Ti split ONNX

| Field | Evidence |
| --- | --- |
| Family / variant | EfficientSAM / Ti |
| Official repository | `yformer/EfficientSAM` |
| Official asset source | Author Yunyang Xiong's EfficientSAM Hugging Face Space, linked by the official repository |
| Fixed revision | `d8dbb1eee73bfb3392aa6f6e8944aeb13f3f4036` |
| Format | Split ONNX, opset 17 according to upstream exporter |
| Files | encoder 24,799,761 bytes; decoder 16,565,728 bytes |
| SHA-256 | encoder `84ed466ffcc5c1f8d08409bc34a23bb364ab2c15e402cb12d4335a42be0e0951`; decoder `a62f8fa5ea080447c0689418d69e58f1e83e0b7adf9c142e2bd9bcc8045c0b11` |
| Code / weights license | Apache-2.0 declared by official repository and author-hosted model source; exact license bytes/digest still to be frozen in M1 |
| Prompt support | Official examples include box and point prompts |
| Known interface | encoder `batched_images` → `image_embeddings`; decoder consumes embedding, batched points/labels and `int64 orig_im_size`, returns masks/IoU |
| Existing SAM Plugin compatibility | No; requires a dedicated Plugin Contract |
| macOS ARM64 CPU | Plausible at ~41.4 MB total; real ORT run pending |
| Linux x86_64 | Format is platform-neutral; real host evidence pending |
| Current decision | Preferred candidate; pending graph and real inference |

Temporary audit download reproduced both exact sizes and hashes on 2026-09-03. The files were not
copied into Git or the workspace.

## Candidate B — MobileSAM ViT-T

| Field | Evidence |
| --- | --- |
| Family / variant | MobileSAM / ViT-T |
| Official repository | `ChaoningZhang/MobileSAM` |
| Official model source | Repository checkpoint (`mobile_sam.pt`) |
| Format | PyTorch checkpoint; official script exports an ONNX prompt/mask component |
| Direct compatible split ONNX | Not established from the official release |
| Code / weight license | Apache-2.0 repository; exact checkpoint notice/digest pending |
| Prompt support | Box and point through SAM-compatible API |
| Current Rust compatibility | No finished two-file Bundle; user-side export would violate delivery requirements |
| Current decision | Secondary candidate; likely reject if no official fixed split ONNX is found |

## Candidate C — Meta SAM 1 ViT-B

| Field | Evidence |
| --- | --- |
| Family / variant | Segment Anything / ViT-B `sam_vit_b_01ec64.pth` |
| Official repository/source | `facebookresearch/segment-anything` and Meta checkpoint URL |
| Format | PyTorch checkpoint; official ONNX script exports the prompt encoder/mask decoder portion |
| Direct compatible split ONNX | No official complete encoder+decoder distribution established |
| Code / weights license | Apache-2.0 |
| Prompt support | Box and point |
| Current Rust compatibility | Existing Plugin Contract expects two ONNX graphs, but official distribution does not satisfy the no-conversion user path |
| Current decision | Audited fallback; not deliverable without maintainer-built and published supply-chain artifacts |

## Candidate D — Meta SAM 2.1 Tiny

| Field | Evidence |
| --- | --- |
| Family / variant | SAM 2.1 / Hiera Tiny |
| Official repository/source | `facebookresearch/sam2`, official `.pt` checkpoint |
| Format | PyTorch checkpoint and Python/PyTorch runtime |
| Direct compatible ONNX | No complete official fixed ONNX release established |
| License | Apache-2.0 for code and checkpoints |
| Prompt support | Box and point for images; video support also exists upstream |
| Current Rust compatibility | Existing local `.pt` is not loadable by the Rust ONNX Plugin |
| Current decision | Labs; not the first delivery candidate |

## M1 work remaining

- Freeze exact official URLs, repository revisions and license digests.
- Inspect EfficientSAM encoder/decoder graphs through the Rust ONNX Runtime.
- Confirm preprocessing, coordinate convention, output selection and mask threshold from primary
  source code.
- Run a real local inference prototype before final acceptance.

