# SAM ViT-B ONNX plugin

This official Rust process implements the `PromptedSegmentation` capability. It consumes an
`Image` plus exactly one `BoxPromptSet` or `PointPromptSet` and returns a lineage-preserving
`MaskSet`.

The plugin requires two user-provisioned files with the exact names declared by the manifest:

- `sam_image_encoder.onnx`: float32 `input_image` `[1,3,1024,1024]` to
  `image_embeddings` `[1,256,64,64]`.
- `sam_mask_decoder.onnx`: the standard SAM decoder inputs `image_embeddings`, `point_coords`,
  `point_labels`, `mask_input`, `has_mask_input`, and `orig_im_size`, with float32 `masks` and
  `iou_predictions` outputs.

Both files are copied into the controlled model cache and hashed by the registry. The combined
component identity is frozen into the model profile. Until both components are provisioned and a
real sample inference passes, the model remains `NeedsWeights` and is not runnable.

Encoder embeddings are cached by encoder digest and exact request-image bytes. Box-to-prompt and
mask-to-box conversion remain visible Core nodes; this plugin never commits annotations.
