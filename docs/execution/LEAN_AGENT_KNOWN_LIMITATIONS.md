# Lean Agent Alpha Known Limitations

Baseline limitations before implementation:

- The live Workflow Advisor performs one constrained submission call; it is not yet the required
  multi-turn Pipeline Builder tool loop.
- The offline iterative Advisor proves an invalid-Draft repair but uses a fixed Application sequence,
  does not yet expose the complete Pipeline Builder tool grammar and tests only a limited Dry Run
  revision.
- Agent sessions use generic statuses and free-form stop strings rather than the Lean Builder
  constraints/status/stop-reason model.
- Pre-Lean Capability implementations still exist as Registry compatibility adapters and internal
  node IDs. New authoring exposes the generic Skills, but removal awaits persisted-version migration.
- Expert mode intentionally exposes internal Workflow node IDs, ports and raw parameters. Guided
  mode groups adjacent operations, but a non-adjacent pair remains separate so graph order is never
  hidden or changed.
- Published versions using the legacy `localization_grid` parameter continue to run. New authoring
  writes `grounding_assist`; the compatibility reader is not yet removed.
- Draft proposals support whole-apply/dismiss, not structured selective Diff application plus Undo.
- SAM, LocateAnything and RF-DETR workers are not running in the audited environment. YOLO has no
  repository weight. Real inference is not claimed.
- Runtime Recovery remains named as an Agent in code and some UI copy even though its behavior is
  deterministic and bounded.
