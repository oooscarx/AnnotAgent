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
- Dry Run candidate reports remain intentionally unscoped; only reference-backed Review evidence
  with exact Project/Run/model lineage can enter calibration.
- Historical Runs that predate frozen Model Profiles retain their bbox correction evidence with an
  explicit insufficient-evidence marker and cannot contribute to calibration until reviewed under a
  revisioned Workflow.
- Prompted-segmentation and mask-to-bbox code exists, but no real active SAM availability can be
  inferred from source presence.
- Exact calibration is implemented, but the Runtime Geometry Quality Evaluation and Geometry
  Decision node behavior arrives in M6. Until then, uncalibrated detector paths still need
  mandatory Review or an available refiner, and a passing report cannot be consumed by a score
  gate.
- Calibration creation currently uses explicitly selected reviewed Runs and up to 1,000 structured
  correction records per Project. Evaluation-set/diagnosis-set separation and comparative
  improvement promotion arrive in M6.
- Historical unsafe Workflow Versions remain viewable and replayable in Sandbox, but cannot start a
  new formal Run. The current safe-clone repair inserts mandatory Review; richer refiner/calibration
  repair choices arrive in later milestones.
