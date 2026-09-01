# RoboCup Skill

## Agent + Skill Alpha structure

`robocup` is a one-Domain-Skill Pack. `robocup.ball` depends on generic Detection capability by
ID/version. The compatibility `RoboCupSkill` now exposes the same single ball task; field, line,
penalty-mark, robot, person and robot-attribute annotation tasks are no longer product options.

The Ball Skill owns only domain resources, correction taxonomy, hard-negative/field-relation
Validators, review policy and model-agnostic template hints. Detection is supplied by generic
Object Detection, Open Vocabulary Detection or VLM capabilities;
Filter/Crop/Gates/Review/Commit are Core.

The hard-negative validator covers white footwear, penalty-mark proximity, line support, duplicate
overlap, unusual geometry and correction-memory risk. Those concepts are evidence for deciding a
ball candidate, never additional annotation outputs.

## Workflows

The compatibility task graph is deliberately one node:

```text
objects[ball]
```

The compatibility Project exposes one default starter, `robocup.ball.vlm-bootstrap`, and the Agent
loads `resources/advisor.md` before drafting. The default is deliberately small:

```text
Image → Detection → Select football candidates → RoboCup Validators → Decision
      → Save strong candidates
      → Human Review uncertain candidates
```

An explicitly enabled `robocup.ball` extension retains the specialist/fallback template for
advanced, configured deployments and immutable-version compatibility. It is not a recommendation
when its required Worker is Disabled, Unknown, unhealthy, or missing checkpoint metadata.

The optional hybrid path is:

```text
Image → specialist Object Detection → primary validation → Detection Recovery
  accepted → Commit
  fallback evidence → Candidate projection → Ball validation → Crop
                    → Classification verification → Review / Reject / Commit
```

Capability bindings live in Project configuration, not the Skill template. Detector geometry and
each source score semantic remain in independent evidence. White-shoe, penalty-mark, field-relation
and exact-scope Correction Memory risks can force fallback or Crop verification; a configured
`not_football` classification takes an explicit Reject terminal.

The Domain Skill registers one dependency-free foreground refiner. SAM is a Model Backend in Labs,
not a RoboCup Skill action. A deployment may compose prompted Segmentation through HTTP Vision
Protocol v1 after the Worker is explicitly configured, sample-tested and Available. The bounded
Advisor adds the generic Prompt→Mask→BBox path only for observed geometry error; Provider failure,
no candidate, white-shoe/semantic risk and missing availability do not trigger SAM.

## Ball hard negatives

`BallHardNegativeValidator` combines geometry and pixels: overlap or lower-body proximity to a robot, aspect/relative size, local white ratio, distance to a penalty keypoint, distance to a field line, original confidence, and project correction risk. It emits explainable codes including white-shoe, penalty-mark, and line-intersection risks. In the Workflow Alpha hybrid demo, the white-shoe candidate is prevented from auto-Commit and routed to Review.

## Correction memory and policy

Human decisions store project/Skill/task/labels/reason, before/after snapshots, note, geometry/color summaries, and time. SQLite retrieval scores recent frequency by project, Skill, task, and label. Runtime injects a compact risk summary and the policy routes frequent or conflicting cases to review. No vector database is used.

## Verification

```bash
cargo test -p annotagent-skill-robocup --test robocup_algorithms
cargo test -p annotagent-storage --test robocup_loops
cargo run -p annotagent -- demo robocup-ball
cargo run -p annotagent -- demo lean-agent-robocup
cargo run -p annotagent -- evaluate \
  --ground-truth examples/robocup/evaluation/ground-truth.synthetic.json \
  --predictions examples/robocup/evaluation/predictions.synthetic.json \
  --bbox-iou-threshold 0.5
```

`lean-agent-robocup` is the no-key Pipeline Builder demonstration: it loads the Domain Advisor
resource, creates an invalid Draft, consumes Rust validation, repairs, performs a real sandbox Dry
Run, and stops at human approval. Its visual evidence is explicitly labelled Mock. Synthetic
labelled evaluation covers geometry and operational metrics. Accuracy is refused for unlabelled
real data. Real Qwen and external worker smoke remain live-conditional and are never inferred from
fixture output.

## Ball bbox geometry policy

The RoboCup Ball Project uses Core's `TrainingBoundingBox` policy. A Qwen VLM proposal is a coarse
hypothesis even when its semantic score is 0.99, so it routes to Human Review unless measured
refinement or exact Project calibration supplies geometry evidence. Ball-specific hard negatives
(white shoe, white sock, penalty mark and field-line intersection) remain Domain Skill reasons;
generic geometry acceptance contains no RoboCup or ball branch. Small football boxes are reported in
their own size bucket.
