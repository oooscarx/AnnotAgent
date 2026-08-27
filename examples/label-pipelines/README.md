# Label Pipeline Alpha examples

These three domain-neutral Project Schemas are the formal offline examples:

- `whole-image-classification`: Image → Classification → Commit;
- `yolo-detection`: Image → YOLO Detection → Filter → Confidence Gate → Commit;
- `yolo-crop-classification`: Image → shared YOLO Detection → label route → Core Crop →
  Classification → Attach Result → Review/Confidence Gate → Commit.

The executable mock acceptance lives in
`crates/annotagent-skill-yolo/tests/label_pipeline_runtime.rs`. Tests generate bounded synthetic
PNG data at runtime, so no opaque or credential-bearing dataset is stored in Git. Crop is a Core
node and is not implemented by the YOLO Detection Skill.
