# Evidence-driven model selection

The Pipeline Builder selects immutable Model Profiles; it does not select Provider credentials or
Worker endpoints. Provider-backed models and Worker-backed expert models are inspected through the
same bounded Agent tools, while their infrastructure remains separate.

## Required evidence

Before applying a model, the Builder checks:

1. the public Node Definition and required capability;
2. model availability (`Available` is the only executable state);
3. typed input, output, and prompt contracts;
4. fixed Label Space coverage when applicable;
5. score semantics and geometry semantics;
6. a legal Artifact conversion path;
7. the latest Dry Run failure classes and geometry-quality report.

Missing-weights, disabled, unconfigured, unknown, unreachable, incompatible, invalid-contract and
failed-smoke models may appear as setup-only alternatives. They are never bound into a publishable
Draft.

## Failure policy

| Evidence | Primary response | Prompted segmentation |
| --- | --- | --- |
| Provider or Worker failure | repair availability, bounded retry/fallback, Review | never a repair |
| No candidate | Resize/Tile, specialist or open-vocabulary detector, Review | unavailable: no prompt |
| Semantic false positive | Crop Classification, Validator, second detector, Review | not the primary repair |
| Geometry error with a plausible candidate | geometry validation or specialist; refine if supported | eligible when model and conversion path are Available |
| Missing score | Evidence Decision, secondary verification, Review | does not invent a score |
| Domain risk | Domain Validator, Correction Memory, Review | optional only after semantics are supported |

VLM scores describe model confidence, not bbox tightness. VLM boxes therefore enter the Pipeline as
`CoarseHypothesis`; Dry Run and human-correction metrics decide whether refinement is justified.

## Prompted-segmentation revision

The Builder's evidence-gated revision is always explicit:

```text
DetectionSet
→ core.detections_to_box_prompts
→ capability.segment
→ MaskSet
→ core.mask_to_bbox
→ DetectionSet
```

It is applied only after a Dry Run produced promptable candidates and recorded geometry problems,
and only when a prompted-segmentation Model Profile is `Available`. If setup is incomplete, the
current Draft keeps Review and the model is returned as an unapplied alternative.

Every resulting Draft must pass Rust static validation, run in the non-committing Dry Run sandbox,
and stop for explicit human approval. The Agent cannot publish or start a formal dataset Run.
