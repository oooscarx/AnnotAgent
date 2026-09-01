# Geometry Safety Blockers

## External/live blockers

### B-001 — No healthy real SAM binding

The repository contains a SAM-compatible worker example, prompted-segmentation contracts and Core
conversion nodes. The active Registry contains no enabled, healthy SAM Model Profile with verified
weights and smoke evidence. Real SAM accuracy remains live-conditional.

### B-002 — No independent stored ground truth for the current four images

The current model boxes may be used as diagnosis inputs, never as their own reference. A human must
correct or approve independent bbox references before objective IoU/calibration claims are possible.

### B-003 — Specialist detector is not configured

No available specialist football detector and weight identity are registered in the active
workspace. Contract and offline behavior can be tested; live inference cannot be claimed.

## Not blockers

- The Qwen Provider is not required for deterministic contract, static-validation, storage or UI
  tests.
- SAM source adapters and mock test runners can validate protocol behavior, but will remain clearly
  labeled as test-only evidence.
