# Geometry Safety Known Limitations

## Baseline limitations

- Historical VLM artifacts that were previously serialized as object-detection capability retain
  their immutable historical value; newly produced VLM artifacts are coarse hypotheses.
- Ordinary `validate_commit_safety` remains as compatibility validation, but publish and formal-Run
  paths now add geometry-aware validation that cannot be satisfied by a semantic/domain Validator.
- A generic `core.confidence_gate` still routes comparable values at Runtime; static geometry policy
  prevents semantic/relative score-only bbox publication.
- The RoboCup field-relation validator is not geometry calibration and may be inapplicable when no
  field geometry exists.
- Existing dry-run geometry reports are useful but do not provide exact project/model/config
  calibration or small/medium/large buckets.
- Review stores revised annotations and correction memory, but does not yet persist the complete
  geometry-correction lineage required by the master prompt.
- Prompted-segmentation and mask-to-bbox code exists, but no real active SAM availability can be
  inferred from source presence.
- Exact Project/model/prompt/config calibration persistence is not implemented until M4, so
  uncalibrated detector paths currently need mandatory Review or an available refiner.
- Historical unsafe Workflow Versions remain viewable and replayable in Sandbox, but cannot start a
  new formal Run. The current safe-clone repair inserts mandatory Review; richer refiner/calibration
  repair choices arrive in later milestones.
