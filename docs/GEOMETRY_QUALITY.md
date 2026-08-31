# Geometry Quality

AnnotAgent treats semantic confidence and geometric quality as different evidence. A high model
score can support “this is probably a football” without proving that its bounding box is tight.

## Geometry semantics

- `coarse_hypothesis`: uncalibrated VLM or grounding geometry that may be useful as a prompt.
- `predicted_geometry`: geometry directly predicted by a detector.
- `mask_refined_geometry`: geometry derived from a prompted segmentation Mask.
- `calibrated_geometry`: geometry backed by an explicit calibration process.
- `human_verified`: geometry accepted or corrected by a reviewer.

Dry Run reports geometry semantics, invalid/degenerate boxes, refiner comparison, review rate and
manual adjustment evidence separately from score confidence. Human bbox edits record normalized
center shift, relative area change and IoU. Unknown measurements remain unknown rather than being
invented as zero or a synthetic confidence.

The Pipeline Builder may add a Refiner only when the observed failure is geometric, the candidate
is promptable, an Available model satisfies the typed contracts, and the complete conversion path
exists. Provider failure, no candidate and semantic/domain errors require different actions.

See `docs/SAM_PIPELINE.md` for the explicit Mask refinement path and
`docs/ADVISOR_MODEL_SELECTION.md` for the decision policy.
