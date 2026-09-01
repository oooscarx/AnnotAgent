# Geometry Quality Model

AnnotAgent treats a model score, geometry provenance, measured quality, calibration and human
validation as separate facts. A single provider confidence value is never expanded into invented
localization or geometry confidences.

## Score semantics

`ScoreSemantics` records what the producer claims a numeric score means:

- `semantic_confidence` — confidence that the target has the requested meaning;
- `detection_confidence` — detector-native confidence, still uncalibrated for a Project by default;
- `calibrated_probability` — a probability backed by its declared calibration scope;
- `relative_confidence` — a comparable but uncalibrated source score;
- `ranking_score` — ordering evidence only;
- `not_provided` and `unknown` — no invented fallback value is permitted.

## Geometry semantics

- `coarse_hypothesis` — a VLM or other proposal source located a plausible region;
- `predicted_geometry` — a detector produced geometry without Project calibration;
- `refined_geometry` — a geometry refiner produced a tighter representation;
- `human_verified` — a human explicitly verified or corrected the geometry.

`mask_refined_geometry` and `calibrated_geometry` remain readable legacy values. New prompted
segmentation contracts use `refined_geometry`; calibration is represented independently.

## Operation-scoped quality contracts

A `ModelCapabilityQualityContract` is bound to:

```text
Model Profile ID + revision + capability + operation
```

This prevents a model's text, classification and VLM-detection uses from sharing an inaccurate
global quality claim. Frozen Workflow snapshots include the effective contracts and never include
credentials or pricing.

Conservative defaults are:

| Operation | Geometry | Score | Score-only auto-accept |
|---|---|---|---|
| OpenAI-compatible VLM detection | Coarse hypothesis | Semantic confidence | Never |
| Specialist/open-vocabulary detection | Predicted geometry | Detection confidence | Requires Project calibration |
| Prompted segmentation | Refined geometry | Not provided | Requires Project calibration |

User-provided metadata is stored as `user_declared`. It cannot claim human-verified geometry and it
does not create a passed calibration.

## Detection quality

`DetectionQuality` contains optional semantic and detector scores plus a
`GeometryQualityReference` and validation state. Missing values remain missing. Geometry reports and
calibration are added only when a Runtime measurement, reference annotation or human correction
actually exists.

## Compatibility

Legacy Model Profile JSON without `quality_contracts` remains readable. AnnotAgent derives
conservative effective contracts at read/freeze time. Existing Published Workflow snapshots are not
rewritten.
