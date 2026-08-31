# Expert Vision Known Limitations

Updated: 2026-09-01

## Baseline limitations before M1

- Provider-backed `ModelProfile` records contain only a Provider connection; expert worker models
  live in a separate descriptor/settings projection.
- Model availability lacks explicit `Unconfigured`, `InvalidContract` and `FailedSmokeTest` states
  and is not yet derived from a complete registration gate.
- The detection-specific Worker protocol exposes health, capabilities, infer and cancel; generic
  model/contract discovery and warmup are absent.
- There is no reusable Python package or CLI scaffold for third-party workers.
- `capability.segment` currently treats DetectionSet as an implicit prompt and emits item-level
  instance masks. Box/Point Prompt Set and Mask Set pipeline Artifacts are not public contracts.
- Mask-to-bbox is hidden inside the legacy RoboCup SAM refiner instead of represented as a Core DAG
  node.
- The public Pipeline Builder has no Artifact conversion-path tool and cannot verify a complete SAM
  chain before suggesting it.
- Dry Run reports counts, warnings, cost and duration but not structured failure classes or geometry
  quality/human-adjustment/refiner metrics.
- Settings can edit existing Detection Workers, but there is no generic discovery-driven Expert
  Model onboarding flow.

These limitations describe the pre-change baseline and will be removed or narrowed milestone by
milestone. Live model accuracy without configured legal weights remains explicitly conditional.
