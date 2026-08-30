# Lean Agent Alpha Known Limitations

Baseline limitations before implementation:

- The live Workflow Advisor performs one constrained submission call; it is not yet the required
  multi-turn Pipeline Builder tool loop.
- The offline iterative Advisor proves an invalid-Draft repair but uses a fixed Application sequence,
  does not yet expose the complete Pipeline Builder tool grammar and tests only a limited Dry Run
  revision.
- Agent sessions use generic statuses and free-form stop strings rather than the Lean Builder
  constraints/status/stop-reason model.
- Existing Capability Skills are fragmented by detection style/model adapter. Segmentation is not a
  single public Capability Skill.
- Existing UI still exposes internal Workflow vocabulary and has compatibility routes/components
  that need consolidation.
- Draft proposals support whole-apply/dismiss, not structured selective Diff application plus Undo.
- SAM, LocateAnything and RF-DETR workers are not running in the audited environment. YOLO has no
  repository weight. Real inference is not claimed.
- Runtime Recovery remains named as an Agent in code and some UI copy even though its behavior is
  deterministic and bounded.

