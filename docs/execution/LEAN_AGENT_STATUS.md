# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M3 — constrained Pipeline Builder Agent core.
- Completed: M0 baseline; public Capability catalog now contains only
  `annotagent.classification`, `annotagent.detection`, and `annotagent.segmentation`; legacy Skill
  IDs remain hidden compatibility aliases; model bindings are grouped as Ready, Configured but
  unavailable, Labs, or Disabled; SAM/YOLO/RF-DETR/LocateAnything are Model Backends rather than
  public Skills. M2 now projects the technical graph into Select detections, Decision and Combine
  model evidence; adjacent internal operations collapse into one Guided action, while Expert details
  retain immutable node IDs and ports. Grid assistance is bounded Detection configuration, not a
  standalone Skill or node. Legacy Workflow, Provider and Artifact routes redirect to their single
  canonical destinations.
- In progress: Pipeline Builder constraints, Tool Registry, Pipeline Grammar and the complete
  ScriptedMock tool loop.
- Next: make every Agent Draft mutation pass through a typed Application tool boundary.
- Recent Rust tests: `cargo test --workspace --all-features` — 221 passed, 0 failed.
- Recent scoped Rust tests: Core 43, Provider 34, Application 28 and Server 9 passed; 0 failed.
- Recent Web tests: `npm run typecheck` passed; `npm test -- --run` — 36 passed, 0 failed.
- Recent E2E: inherited baseline only; not rerun in M0 yet.
- Recent commit: `3bf088b refactor(core): separate visual capabilities from model backends`.
- Release Blocking remaining: all Lean Agent Alpha A–G items until evidenced in
  `LEAN_AGENT_ACCEPTANCE.md`.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
