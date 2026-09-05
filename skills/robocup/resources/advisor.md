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

Treat vision-language bounding boxes as uncalibrated coarse geometry. A whole-image VLM proposal
is a search hint, not automatically a valid segmentation prompt. For a football expected to be
roughly 10–20 pixels across, prefer the bounded recovery chain registered by the Project when its
required models are Ready: coarse localization → target-scale profile → relative search region →
crop the untouched original image → local re-localization in crop coordinates → project to root
coordinates → hard-negative validation → independent prompt-coverage gate → prompted segmentation
→ mask bbox → geometry decision → review or commit. Use the versioned `robocup.ball` prompt
resources at whole-image detection, local re-localization, and crop verification; do not replace
them with a model-brand-specific prompt.

Do not add a Refiner merely because the VLM limitation exists: first run a Dry Run and inspect
structured evidence. Diagnose the failure before editing the Draft:

- Provider or Worker failure: repair availability or use an already-available detector. Prompted
  segmentation cannot repair a request that produced no DetectionSet.
- Coarse localization miss or prompt outside target: expand around the coarse proposal and perform
  local re-localization. If the bounded relative search does not recover the object, use a bounded
  Tile fallback; when its model-call, tile, latency, or cost budget is exhausted, route to Review.
  Prompted segmentation has no validated target prompt yet and must not run.
- No candidate at all: consider bounded Tile/Resize, an available specialist or open-vocabulary
  detector, or Review. Prompted segmentation has no prompt and must not be added.
- Possible white shoe, white sock, penalty mark, or field-line intersection: use Crop
  Classification, `ball_hard_negative`, field relation, a second detector, or Review. Tightening a
  semantically wrong object does not repair it.
- Semantically plausible football with independently Covered prompt evidence and loose geometry:
  if an Available Prompted Segmentation model and the registered DetectionSet → Prompt Coverage →
  BoxPromptSet → MaskSet → DetectionSet path exist, it may refine geometry. Partially covered,
  outside, or unknown prompt coverage routes to re-localization, bounded search, or Review.
- Refiner drift: reject the refined geometry, preserve the coarse/local candidate as evidence, and
  route to Review. Never reinterpret a drifting mask as better semantic confidence.
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

When correcting an existing Draft, preserve its valid label route and bindings. Add the smallest
typed localization-recovery subgraph needed for the observed failure class; do not rebuild the
entire plan or silently replace the user's detector choices.
