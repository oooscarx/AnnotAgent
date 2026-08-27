# RoboCup Skill

## Agent + Skill Alpha structure

`robocup` is now a `Pack` manifest. `robocup.ball` is the release-blocking `Domain` Skill and
depends on generic Detection capability by ID/version. Robot and Field remain Roadmap-only Domain
splits while the original broad `RoboCupSkill` stays available as a compatibility adapter.

The Ball Skill owns only domain resources, correction taxonomy, hard-negative/field-relation
Validators, review policy and two model-agnostic templates. Detection is supplied by
`vlm-detection` or `yolo-detection`; Filter/Crop/Gates/Review/Commit are Core.

The hard-negative validator covers white footwear, penalty-mark proximity, line support, duplicate
overlap, unusual geometry and correction-memory risk. The field-relation validator reports a
warning when field evidence is unavailable and an explicit issue when a candidate lies outside the
known field polygon; it never panics on absent optional evidence.

## Workflows

The Skill supplies this DAG while Runtime only executes dependencies:

```text
scene_type → field_region ┬→ field_line
                          ├→ penalty_mark
                          └→ objects → robot_attributes
```

Task markdown is loaded only when its task is active. The Skill manifest is the source of UI correction reasons and resource listings. The Workflow Designer also exposes three Skill-owned typed starters: `vlm-bootstrap`, `detector-first`, and `accurate-hybrid`. They are visible only to Projects that enable `robocup`.

The release hybrid path is:

```text
detector candidates
+ VLM semantic verification
→ RoboCup hard-negative Validator
→ Review Gate
→ Commit
```

Specialist geometry remains in its original Artifact. The VLM contributes classification/attributes and never rewrites detector or refined coordinates.

## Field containment

`FieldContainmentValidator` measures candidate evidence inside the field polygon. Object centers and keypoints need majority containment; a polyline needs a stronger ratio with boundary tolerance. Missing field geometry produces `field_region_missing` warning rather than a panic or fabricated pass.

## Field-line pixel refinement

`RoboCupFieldLineRefiner` converts the coarse normalized line to pixels, samples segments, searches the local normal direction for high-brightness/low-saturation response, respects the field polygon, rejects unsupported offsets, smooths samples, applies Ramer–Douglas–Peucker simplification, clips normalized output, and reports support/continuity confidence. `WhiteLineAppearanceValidator` and `PolylineContinuityValidator` check support, length, jumps, continuity, and width behavior. Weak evidence keeps the original audit trail and raises `weak_pixel_support`.

## Ball hard negatives

`BallHardNegativeValidator` combines geometry and pixels: overlap or lower-body proximity to a robot, aspect/relative size, local white ratio, distance to a penalty keypoint, distance to a field line, original confidence, and project correction risk. It emits explainable codes including white-shoe, penalty-mark, and line-intersection risks. In the Workflow Alpha hybrid demo, the white-shoe candidate is prevented from auto-Commit and routed to Review.

## Robot evidence

`TeamColorEvidenceTool` crops the torso portion of a checked robot rectangle, measures red/blue evidence, and recommends red, blue, or unknown. `RobotAttributeValidator` checks required `team_color` and `state`, their schema values, deterministic color conflict, and geometry-based state warnings without pretending state is a deterministic fact.

## Correction memory and policy

Human decisions store project/Skill/task/labels/reason, before/after snapshots, note, geometry/color summaries, and time. SQLite retrieval scores recent frequency by project, Skill, task, and label. Runtime injects a compact risk summary and the policy routes frequent or conflicting cases to review. No vector database is used.

## Verification

```bash
cargo test -p annotagent-skill-robocup --test robocup_algorithms
cargo test -p annotagent-storage --test robocup_loops
cargo run -p annotagent -- demo robocup-hybrid
cargo run -p annotagent -- evaluate \
  --ground-truth examples/robocup/evaluation/ground-truth.synthetic.json \
  --predictions examples/robocup/evaluation/predictions.synthetic.json \
  --minimum-field-region-iou 0.7
```

Synthetic labelled evaluation covers geometry and operational metrics. Accuracy is refused for unlabelled real data. Real Qwen and external worker smoke remain live-conditional and are never inferred from fixture output.
