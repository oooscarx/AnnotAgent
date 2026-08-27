# Workflow Alpha Demo

Both release demos are stable, offline, and need no API key, GPU, or external model weights.

## Generic Workflow

```bash
cargo run -p annotagent -- demo generic-workflow
```

This has no RoboCup Skill dependency. It publishes and executes a typed graph containing detector and prompted-segmentation model categories, a generic shape Validator, Review Gate, and Commit. Expected stable result:

```text
status=completed artifacts=2 committed=2 review=false model_calls=2
```

The trace names candidate, specialist/semantic, validation, review, and Commit nodes and reports each node's typed Artifact count.

## RoboCup Hybrid

```bash
cargo run -p annotagent -- demo robocup-hybrid
```

This runs detector candidates plus VLM semantic evidence through the real RoboCup hard-negative Validator, Review Gate, and Commit. The offline fixture deliberately includes a white-shoe false positive. Expected stable result:

```text
status=completed_with_review artifacts=3 committed=0 review=true model_calls=2
validation possible_white_shoe
```

This is not a fake successful detector: mock model use is named in the command, while the Skill Validator and DAG routing are real. A configured live Provider can use the same OpenAI-compatible contract; a real external detector/segmenter uses the shared HTTP worker protocol. Live results are reported only when current credentials/weights are explicitly configured.

## Product journey

Build and launch:

```bash
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

In the GUI:

1. Create a Generic Project and import or add workspace images.
2. Open Workflows, create or suggest a Draft, edit nodes/edges/bindings/retry/fallback/review, and inspect exact validation issues.
3. run a selected-image Dry Run, publish, and select the immutable version.
4. choose **Start image run** or **Start dataset batch**. Navigate away and return; active/last state comes from the server.
5. inspect Runs for node/Artifact/validation/recovery/model/usage/checkpoint evidence.
6. edit or create annotations in Review, compare before/after, record a correction reason, and save a revision.
7. import/export Native, COCO, LabelMe, or YOLO and read compatibility warnings.

Run the complete release gate with:

```bash
./scripts/acceptance.sh
```
