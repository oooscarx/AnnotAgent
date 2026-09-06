# Recovery semantics and model-input repair

This log separates graph/runtime correctness from visual accuracy. Historical Drafts, Sample Tests,
Runs, Reviews, annotations and model records are preserved. A saved model prediction or exported
box is not treated as ground truth unless its human-review provenance and matching image revision
can be established.

## M0 — verified failure baseline

### Repository and scope

- Branch: `main`; the branch started this repair 21 commits ahead of `origin/main`.
- Pre-existing modified files are fourteen PNGs under `docs/execution/screenshots/`. They are not
  part of this repair and are not staged or overwritten.
- The active persistence database is `workspace/.annotagent/history.db`.
- The legacy zero-byte `workspace/.annotagent/annotagent.db` is not the active evidence store.

### Saved Sample Test identity

| Field | Verified value |
| --- | --- |
| Sample Test | `53c4c39d-cb9a-4e3d-9bea-79852a5d130a` |
| Draft | `183d793c-c8ce-433e-8fdc-d1bf81186725`, revision 2 |
| Workflow hash | `5db86c94acb29065b2e3a8004683bf3bfdb2b54820a5adec4638ce404c7f1b28` |
| Image-set hash | `2492440a3543d7b791b80a6aea60fccbcc905c0b2474bfcec2529759f8298122` |
| Model-snapshot hash | `fd267f7dd142d44b7ddf0829a2e20525d14f412ab29c87384917a023a8ab53e8` |
| Start / completion | `2026-09-06T06:13:59.864583Z` / `2026-09-06T06:14:03.484017Z` |

The input is image `dbe3d88d-ed0a-55af-b762-b5a1d33d3b35`, relative path
`images/color_1001525.png`, 544×448 pixels, SHA-256
`b0b470a93ed334a52462316029d1cf8794ce4287eaba37a2f551af3848043cfd`.
The PNG has no EXIF orientation value; the decoded orientation is therefore the stored 544×448
pixel raster, not a separately observed camera orientation.

### Frozen model and prompt inputs

- `coarse_localization`, `local_relocalization` and the misleadingly named
  `independent_crop_verification` all resolved to Model Profile
  `b9c5bbe8-e21a-5784-9c52-cade259b434f@2`, remote identity
  `qwen3.7-flash-2026-07-15`.
- `refine_validated_prompt` resolved to ready Model Instance
  `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`, EfficientSAM-Ti ONNX, plugin
  `org.annotagent.efficientsam-onnx@1.0.0` and Bundle
  `org.annotagent.models.efficientsam-ti-onnx@1.0.0`.
- Coarse prompt: resource version `2.0.0`, SHA-256
  `5530dcdecf14a07f5bcfada7ae03660ad0013631ad0037ded6eac17b48bd9fce`.
- Local prompt: resource version `2.0.0`, SHA-256
  `c10efdd07415809ecbcff79f640ea619aaa293bbeb98b5f35972bb647ed09a9e`.
- Repeated crop-verification prompt: resource version `2.0.0`, SHA-256
  `7d8c2980b405cb1ab2f37e176c4b5f8a9983873d17d7c404e0f30230b57b9516`.

No secret, bearer header or image base64 is recorded here.

### Actual saved execution

1. Whole-image Qwen localization returned normalized `xywh`
   `[0.482, 0.510, 0.024, 0.028]`.
2. `core.expand_region` produced root ROI
   `[0.4057647, 0.41685712, 0.1764706, 0.21428572]`.
3. `core.crop` described a 96×96 search image.
4. Both local Qwen nodes consumed that same source crop. The application runner reconstructs a
   crop from `root_region` and calls `to_model_image` with the crop's own maximum edge, so the
   declared 96×96 input was submitted without an explicit 4× resize.
5. Projected local boxes were
   `[0.4830588, 0.46292856, 0.02152941, 0.03321429]` and
   `[0.48164704, 0.46292856, 0.03000000, 0.03321429]`. Their overlap is correlated
   same-model/same-view agreement, not independent evidence and not target truth.
6. `core.prompt_coverage_gate` selected `PartiallyCovered · ExpandAndRelocalize`.
7. The selected `relocalize` edge targeted `review_final_ball` directly. There was no search view B,
   no second localization and no recovery model input.
8. `refine_validated_prompt` was skipped because only the `refine` route reaches it.
   `refiner_usage_count=0`; this Sample Test contains no evidence about EfficientSAM's accuracy.
9. The terminal candidate remained the first projected local box and was marked `needs_review`.

At M0 the stored model-call trace does not retain an outbound-image digest, decoded submitted
dimensions, color format, interpolation, or transform fingerprint. The 96×96 conclusion is verified
from the saved Image Artifact dimensions plus the runner implementation, not yet from a persisted
provider-boundary trace. M2 adds that direct evidence.

### Reference provenance and visual mismatch

The active database contains no `annotations` or `annotation_revisions` for this image. The file
`workspace/robocup-ball/exports/yolo_detection/color_1001525.txt` contains an older exported box
with normalized center `(.504, .451)` and size `(.024, .032)`, but the export report does not retain
an annotation/revision ID or human-review state. The repository's preserved fixture explicitly sets
`ground_truth: null` and `accuracy_status: provisional_pending_human_calibration`.

Consequently, the low overlap between the current terminal box and that export is recorded only as
**a mismatch with an unverified historical export**. It is not reported as model accuracy or IoU
against ground truth.

### Regression test written before the fix

`relocalization_route_cannot_go_directly_to_review` in the RoboCup Ball Skill inspects the executable
template and rejects a `relocalize` route whose immediate target is Human Review. It is temporarily
ignored in M0 so the repository remains buildable while preserving a test that fails against the
unmodified behavior. M1 removes the ignore after implementing the second bounded search.

Command used to reproduce the red test:

```text
cargo test -p annotagent-skill-robocup relocalization_route_cannot_go_directly_to_review -- --ignored
```

Expected M0 failure: the route target is `review_final_ball`, a `HumanReview` node.

## Remaining milestones

- M1: machine-checkable route semantics and a real bounded search view B.
- M2: materialized Resize/model-input trace, shared coordinate transforms and correlated-evidence
  de-duplication.
- M3: separate refinement eligibility from automatic acceptance and pass the selected recovered
  candidate through SAM/geometry/review safely.
- M4: Builder inspection/validation tools, branch-injection tests, UI execution summary and fixed-set
  real evaluation. Engineering correctness and visual quality will be reported separately.
