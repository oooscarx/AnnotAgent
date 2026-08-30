# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M2 — public Pipeline vocabulary and route convergence.
- Completed: M0 baseline; public Capability catalog now contains only
  `annotagent.classification`, `annotagent.detection`, and `annotagent.segmentation`; legacy Skill
  IDs remain hidden compatibility aliases; model bindings are grouped as Ready, Configured but
  unavailable, Labs, or Disabled; SAM/YOLO/RF-DETR/LocateAnything are Model Backends rather than
  public Skills.
- In progress: Select detections, Decision and Combine model evidence guided abstractions; Grid
  assist as node configuration; removal of duplicate product entry points.
- Next: preserve Expert node types while simplifying the default Automation editor.
- Recent Rust tests: `cargo test --workspace --all-features` — 221 passed, 0 failed.
- Recent Web tests: `npm run typecheck` passed; `npm test -- --run` — 35 passed, 0 failed.
- Recent E2E: inherited baseline only; not rerun in M0 yet.
- Recent commit: `f38ee6f docs: establish lean agent architecture baseline`.
- Release Blocking remaining: all Lean Agent Alpha A–G items until evidenced in
  `LEAN_AGENT_ACCEPTANCE.md`.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
