# Geometry Safety Known Limitations

## Baseline limitations

- Historical VLM artifacts that were previously serialized as object-detection capability retain
  their immutable historical value; newly produced VLM artifacts are coarse hypotheses.
- `validate_commit_safety` treats any Validator ancestor as sufficient, even when the Validator does
  not measure geometry.
- A generic `core.confidence_gate` compares relative scores without knowing whether they are semantic
  or localization evidence.
- The RoboCup field-relation validator is not geometry calibration and may be inapplicable when no
  field geometry exists.
- Existing dry-run geometry reports are useful but do not provide exact project/model/config
  calibration or small/medium/large buckets.
- Review stores revised annotations and correction memory, but does not yet persist the complete
  geometry-correction lineage required by the master prompt.
- Prompted-segmentation and mask-to-bbox code exists, but no real active SAM availability can be
  inferred from source presence.
- Historical unsafe Workflow Versions remain runnable until Milestone 2 introduces explicit safety
  compatibility and formal-Run guards.
