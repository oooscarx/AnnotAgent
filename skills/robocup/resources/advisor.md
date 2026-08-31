# RoboCup Ball Pipeline Builder guidance

This Project annotates one thing only: a football bounding box. Build the smallest Pipeline that
the currently available Model Registry can actually execute:

1. Image → one available Detection backend.
2. Select and map only football candidates.
3. Run `ball_hard_negative` and `robocup_ball_field_relation` when field evidence exists.
4. Use one Decision with explicit Save and Human Review routes.

Prefer an available, label-compatible specialist detector when its fixed Label Space contains
football. An available open-vocabulary detector may cold-start when no trained specialist exists;
a configured vision-language detector remains a coarse semantic fallback. Model names are selected
from the Registry, never prescribed by this Domain Skill.

Treat vision-language bounding boxes as uncalibrated coarse geometry. Do not add a Refiner merely
because that general limitation exists: first run a Dry Run and inspect structured evidence.

- Provider or Worker failure: repair availability or use an already-available detector. Prompted
  segmentation cannot repair a request that produced no DetectionSet.
- No candidate: consider Tile/Resize, an available specialist or open-vocabulary detector, or
  Review. Prompted segmentation has no prompt and must not be added.
- Possible white shoe, white sock, penalty mark, or field-line intersection: use Crop
  Classification, `ball_hard_negative`, field relation, a second detector, or Review. Tightening a
  semantically wrong object does not repair it.
- Semantically plausible football with repeated bbox resizing, center shift, area reduction, or
  geometry-review evidence: if an Available Prompted Segmentation model and the registered
  DetectionSet → BoxPromptSet → MaskSet → DetectionSet path exist, it may refine geometry.
- Missing score: use Evidence Decision or Review. Never synthesize a default confidence.

Segmentation, dual-model evidence, specialist fallback and open-vocabulary fallback are conditional
alternatives, not defaults. Never add an unavailable, Unknown, disabled, unconfigured,
missing-weights, incompatible, unreachable or failed-smoke model to an executable Draft. Such a
model may be named only as an unapplied setup alternative.

White shoes, white socks, penalty marks, and field-line intersections are hard negatives. They are
context for validation and Review, never output Labels. Missing field geometry is not permission to
invent it. Explain recommendations with observed counts and geometry metrics, not hidden reasoning
or invented benchmarks. The Agent may submit only an editable Draft for human approval and may
never Publish or start a formal Run.
