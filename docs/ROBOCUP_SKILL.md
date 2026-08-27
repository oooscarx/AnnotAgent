# RoboCup Skill

## Agent + Skill Alpha structure

`robocup` is a one-Domain-Skill Pack. `robocup.ball` depends on generic Detection capability by
ID/version. The compatibility `RoboCupSkill` now exposes the same single ball task; field, line,
penalty-mark, robot, person and robot-attribute annotation tasks are no longer product options.

The Ball Skill owns only domain resources, correction taxonomy, one hard-negative Validator,
review policy and two model-agnostic templates. Detection is supplied by
`vlm-detection` or `yolo-detection`; Filter/Crop/Gates/Review/Commit are Core.

The hard-negative validator covers white footwear, penalty-mark proximity, line support, duplicate
overlap, unusual geometry and correction-memory risk. Those concepts are evidence for deciding a
ball candidate, never additional annotation outputs.

## Workflows

The compatibility task graph is deliberately one node:

```text
objects[ball]
```

The only task resource is `tasks/ball.md`. The Workflow Designer exposes
`robocup.ball.vlm-bootstrap` and `robocup.ball.detector-first` only.

The release hybrid path is:

```text
Image → generic detector → ball filter
→ RoboCup Ball hard-negative Validator
→ Review Gate
→ Commit
```

Detector geometry remains in its original Artifact. A VLM may verify a crop but never rewrites the
box.

## Ball hard negatives

`BallHardNegativeValidator` combines geometry and pixels: overlap or lower-body proximity to a robot, aspect/relative size, local white ratio, distance to a penalty keypoint, distance to a field line, original confidence, and project correction risk. It emits explainable codes including white-shoe, penalty-mark, and line-intersection risks. In the Workflow Alpha hybrid demo, the white-shoe candidate is prevented from auto-Commit and routed to Review.

## Correction memory and policy

Human decisions store project/Skill/task/labels/reason, before/after snapshots, note, geometry/color summaries, and time. SQLite retrieval scores recent frequency by project, Skill, task, and label. Runtime injects a compact risk summary and the policy routes frequent or conflicting cases to review. No vector database is used.

## Verification

```bash
cargo test -p annotagent-skill-robocup --test robocup_algorithms
cargo test -p annotagent-storage --test robocup_loops
cargo run -p annotagent -- demo robocup-ball
cargo run -p annotagent -- evaluate \
  --ground-truth examples/robocup/evaluation/ground-truth.synthetic.json \
  --predictions examples/robocup/evaluation/predictions.synthetic.json \
  --bbox-iou-threshold 0.5
```

Synthetic labelled evaluation covers geometry and operational metrics. Accuracy is refused for unlabelled real data. Real Qwen and external worker smoke remain live-conditional and are never inferred from fixture output.
