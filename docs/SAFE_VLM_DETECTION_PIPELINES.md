# Safe VLM Detection Pipelines

## VLM only

```text
Image → VLM coarse detection → Select Label → Domain validation
      → Mandatory Human Review → Commit
```

Automatic box acceptance is disabled. This is the runnable fallback when no healthy compatible
refiner or calibrated specialist detector exists.

## Prompted refinement available

```text
Image → VLM coarse detection → Box Prompts → Prompted Segmentation → Mask
      → Mask to BBox → Geometry Evaluation → Geometry Decision
      ├─ stable evidence → Commit
      └─ weak/conflicting evidence → Human Review → Commit
```

The Runtime preserves the coarse box, prompt, mask, refined box, IoU, center shift, area change and
decision lineage. A returned mask is evidence, not automatic approval.

## Calibrated specialist detection

```text
Image → Specialist Detection → Geometry Evaluation → Geometry Decision
      → Commit / Human Review
```

A specialist detector starts as predicted, uncalibrated geometry. Project-specific calibration and
an explicit geometry decision are required for automatic acceptance.

Static validation rejects uncalibrated coarse/predicted boxes that reach Commit through only a
semantic, relative or detector score. `allow_unvalidated_commit` cannot bypass this rule.
