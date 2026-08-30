# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M4 — OpenAI-compatible multi-turn Pipeline Builder provider.
- Completed: M0 baseline; public Capability catalog now contains only
  `annotagent.classification`, `annotagent.detection`, and `annotagent.segmentation`; legacy Skill
  IDs remain hidden compatibility aliases; model bindings are grouped as Ready, Configured but
  unavailable, Labs, or Disabled; SAM/YOLO/RF-DETR/LocateAnything are Model Backends rather than
  public Skills. M2 now projects the technical graph into Select detections, Decision and Combine
  model evidence; adjacent internal operations collapse into one Guided action, while Expert details
  retain immutable node IDs and ports. Grid assistance is bounded Detection configuration, not a
  standalone Skill or node. Legacy Workflow, Provider and Artifact routes redirect to their single
  canonical destinations.
- M3 completed: typed Pipeline Builder constraints/status/stop reasons; a 31-tool allow-list;
  registry-bounded Draft mutations; Rust Pipeline Grammar; turn/tool/Dry Run/cost limits; structured
  Tool results; and a ScriptedMock policy that proves invalid Draft → validation error → repair →
  valid Draft → high-review Dry Run → crop-verification revision → second Dry Run → human approval.
  The Application integration now creates its invalid Draft with a real disconnect tool, repairs it
  with a connect tool, and persists only registered Tool actions.
- In progress: replace the one-shot live Advisor submission with the same bounded multi-turn Tool
  loop over the OpenAI-compatible provider.
- Next: translate Registry tools into provider schemas and feed each Rust-owned Tool result back to
  the model without exposing hidden reasoning.
- Recent Rust tests: `cargo test --workspace --all-features` — 221 passed, 0 failed.
- Recent Rust tests: Core 48, Application 28, Server 9, Storage unit 9 and Storage integration 16
  passed; 0 failed.
- Recent Web tests: `npm run typecheck` passed; `npm test -- --run` — 36 passed, 0 failed.
- Recent E2E: inherited baseline only; not rerun in M0 yet.
- Recent commit: `5ee5cd4 refactor(workflow): simplify the public pipeline vocabulary`.
- Release Blocking remaining: all Lean Agent Alpha A–G items until evidenced in
  `LEAN_AGENT_ACCEPTANCE.md`.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
