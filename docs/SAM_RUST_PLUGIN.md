# SAM Rust plugin

`org.annotagent.sam-onnx@1.0.0` is an independent Rust process implementing the generic
`PromptedSegmentation` capability. Core and the Segmentation Skill do not branch on this plugin ID.

## Contract

The model `sam-vit-b-onnx` consumes one `Image` and exactly one `BoxPromptSet` or `PointPromptSet`.
It returns one `MaskSet`. Each mask references the exact prompt-set item that produced it, retains
the decoder mask index and relative mask score, and uses original-image coordinates through
uncompressed column-major COCO RLE.

The workflow remains explicit:

```text
DetectionSet
→ core.detections_to_box_prompts
→ capability.segment
→ MaskSet
→ core.mask_to_bbox
→ core.geometry_quality_evaluation
→ core.geometry_decision / Human Review
```

The plugin does not hide Box Prompt conversion, Mask-to-BBox, evaluation, review or Commit.

## ONNX files

The plugin validates two separately provisioned components:

| Component | Controlled filename | Required contract |
| --- | --- | --- |
| `image_encoder` | `sam_image_encoder.onnx` | float32 `input_image` `[1,3,1024,1024]` → `image_embeddings` `[1,256,64,64]` |
| `mask_decoder` | `sam_mask_decoder.onnx` | standard SAM float32 embedding, point, prior-mask and original-size inputs → `masks` plus `iou_predictions` |

The package does not name a fixed download recipe because no single upstream two-file export has
been accepted as the official product checkpoint. A local file is copied under the controlled
component filename, SHA-256 hashed and recorded in the durable Registry. The model identity is the
SHA-256 of the ordered component names and hashes. Both files must exist before status can leave
`NeedsWeights`; a real process smoke test is still required before `Ready`.

Example provisioning:

```bash
cargo run -p annotagent -- plugin provision org.annotagent.sam-onnx \
  --version 1.0.0 --model sam-vit-b-onnx --component image_encoder \
  --weights /absolute/path/to/encoder.onnx

cargo run -p annotagent -- plugin provision org.annotagent.sam-onnx \
  --version 1.0.0 --model sam-vit-b-onnx --component mask_decoder \
  --weights /absolute/path/to/decoder.onnx
```

## Execution details

The image encoder follows SAM longest-side resize to 1024, RGB normalization and bottom/right
zero padding. Box prompts use decoder labels 2/3; point prompts use positive/negative labels 1/0
and one explicit no-point sentinel. The decoder can select the highest-scored mask or return a
bounded multi-mask set.

Embeddings are cached by exact encoder digest and decoded image-byte digest. The cache uses a
bounded versioned binary record inside the plugin cache directory, validates shape and finiteness
on read, and never changes Artifact lineage.

## Verification state

Offline tests cover manifest contracts, two-component readiness, preprocessing, coordinate
mapping, box/point labels, multi-mask score selection, RLE geometry, cache round-trip and the
existing Core geometry chain. An ignored opt-in process test accepts explicitly supplied legal
encoder/decoder files and a sample image, then runs process conformance and
Prompted Segmentation → Mask-to-BBox → Geometry Evaluation.

No production SAM checkpoint is committed or declared `Ready`; real inference is
`live-conditional` until that opt-in test passes for the provisioned component identity.
