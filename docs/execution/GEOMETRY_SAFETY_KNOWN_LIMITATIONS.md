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
- Runtime Geometry Evaluation currently evaluates prompted-refinement traces. A historical Passed
  calibration can satisfy static policy only through the same explicit geometry decision boundary,
  but direct runtime consumption of a calibration report without a prompted trace is not yet a
  supported execution mode; such a mismatch fails closed.
- Calibration creation currently uses explicitly selected reviewed Runs and up to 1,000 structured
  correction records per Project. Evaluation-set/diagnosis-set separation and comparative
  improvement promotion arrive in M7.
- Historical unsafe Workflow Versions remain viewable and replayable in Sandbox, but cannot start a
  new formal Run. The current safe-clone repair inserts mandatory Review; richer refiner/calibration
  repair choices arrive in later milestones.
- Pipeline Builder correction inspection intentionally returns bounded aggregates, not images or
  free-form reviewer notes. Per-sample comparison and holdout promotion remain M7 work.
- Geometry Decision currently routes stable prompted refinements to `accept` at Runtime, but the
  generated first improvement Draft deliberately retains mandatory Human Review until M7 adds an
  independent holdout comparison and an explicit human promotion step.
- The Qwen-only first-Draft behavior is deterministically verified with a scripted Provider. A
  billable live Provider response is still conditional and cannot weaken Core publication safety.
