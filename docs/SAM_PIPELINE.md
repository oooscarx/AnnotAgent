# SAM Geometry Pipeline

The native package `org.annotagent.sam-onnx` provides PromptedSegmentation and returns MaskSet with
prompt lineage. A safe Workflow composes generic nodes:

```text
coarse DetectionSet
→ DetectionsToBoxPrompts
→ Ready SAM plugin
→ MaskSet
→ MaskToBBox
→ Geometry Evaluation
→ Geometry Decision
→ Commit or Human Review
```

SAM does not make a coarse box trustworthy by existence or semantic score. Package, both checkpoint
components, contract and smoke evidence must be Ready, and geometry safety still decides whether a
result can avoid Review. See [SAM Rust Plugin](SAM_RUST_PLUGIN.md) and
[VLM Geometry Safety](VLM_GEOMETRY_SAFETY.md).
