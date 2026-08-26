# RoboCup Skill

## Workflow

The Skill supplies this DAG while Runtime only executes dependencies:

```text
scene_type → field_region ┬→ field_line
                          ├→ penalty_mark
                          └→ objects → robot_attributes
```

Task markdown is loaded only when its task is active. The Skill manifest is the source of UI correction reasons and resource listings.

## Field containment

`FieldContainmentValidator` measures candidate evidence inside the field polygon. Object centers and keypoints need majority containment; a polyline needs a stronger ratio with boundary tolerance. Missing field geometry produces `field_region_missing` warning rather than a panic or fabricated pass.

## Field-line pixel refinement

`RoboCupFieldLineRefiner` converts the coarse normalized line to pixels, samples segments, searches the local normal direction for high-brightness/low-saturation response, respects the field polygon, rejects unsupported offsets, smooths samples, applies Ramer–Douglas–Peucker simplification, clips normalized output, and reports support/continuity confidence. `WhiteLineAppearanceValidator` and `PolylineContinuityValidator` check support, length, jumps, continuity, and width behavior. Weak evidence keeps the original audit trail and raises `weak_pixel_support`.

## Ball hard negatives

`BallHardNegativeValidator` combines geometry and pixels: overlap or lower-body proximity to a robot, aspect/relative size, local white ratio, distance to a penalty keypoint, distance to a field line, original confidence, and project correction risk. It emits explainable codes including white-shoe, penalty-mark, and line-intersection risks. In the offline demo, the first shoe candidate triggers a retry and only the real ball is accepted.

## Robot evidence

`TeamColorEvidenceTool` crops the torso portion of a checked robot rectangle, measures red/blue evidence, and recommends red, blue, or unknown. `RobotAttributeValidator` checks required `team_color` and `state`, their schema values, deterministic color conflict, and geometry-based state warnings without pretending state is a deterministic fact.

## Correction memory and policy

Human decisions store project/Skill/task/labels/reason, before/after snapshots, note, geometry/color summaries, and time. SQLite retrieval scores recent frequency by project, Skill, task, and label. Runtime injects a compact risk summary and the policy routes frequent or conflicting cases to review. No vector database is used.

## Verification

```bash
cargo test -p annotagent-skill-robocup --test robocup_algorithms
cargo test -p annotagent-storage --test robocup_loops
cargo run -p annotagent -- demo robocup
```
