# Failure and Geometry Evaluation

AnnotAgent records semantic confidence, geometry quality, and execution health as separate facts.
A high model score does not imply that a bounding box is tight, and a Provider failure is not
evidence that a geometry refiner is needed.

## Failure classes

Runtime and validation codes are projected into one of these stable classes:

- `infrastructure_failure`: a Worker or local service cannot be reached;
- `provider_failure`: Provider authentication, rate limit, request, or timeout failure;
- `no_candidate`: execution succeeded but produced no candidate prompt;
- `semantic_error`: the candidate represents the wrong class or target;
- `geometry_error`: the target is plausible but its geometry needs correction;
- `missing_score`: the backend did not provide a comparable score;
- `domain_risk`: a Domain Validator or correction history reports risk;
- `invalid_artifact`: the Artifact or its contract is malformed;
- `budget_limit`: execution stopped at a configured cost or token boundary.

Only `geometry_error` is direct evidence for adding a geometry refiner such as prompted
segmentation. In particular, SAM cannot repair Provider failure, missing prompts, or a semantically
wrong object.

## Geometry evidence

Every Detection has explicit `geometry_semantics`. Vision-language boxes default to
`coarse_hypothesis`; ordinary detector boxes default to `predicted_geometry`; boxes derived from a
prompted-segmentation mask are `mask_refined_geometry`.

`GeometryQualityReport` reports only observed or computable evidence. Foreground, edge, and mask
support remain `null` unless a node actually measured them. The current deterministic checks cover
area, aspect ratio, image-boundary contact, declared clipping, and refiner comparison. Refiner
metrics are normalized center shift, absolute relative area change, and IoU between the original
and refined boxes.

When a reviewer edits a bounding box, the annotation revision API returns the same normalized
center-shift, area-change, and IoU metrics. Accepting or rejecting that review persists the metrics
with Correction Memory. Later Dry Runs expose the historical geometry-correction rate and aggregate
manual-adjustment statistics without copying image data into the Agent context.

## Dry Run API

`WorkflowDryRunReport.summary` includes Provider/Worker failures, no-candidate, semantic/geometry/
domain reviews, missing scores, manual resize metrics, refiner use/success/fallback counts, and a
`GeometryQualitySummary`. Each sample and node also exposes bounded failure classes; each detection
outcome may include its `GeometryQualityReport`.

All fields are persisted in the existing versioned sample-test record. Missing fields on older
records deserialize to zero or `null`.
