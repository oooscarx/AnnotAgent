# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M5 — validation/Dry Run-driven revision and human boundary.
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
- M4 completed: the OpenAI-compatible path now runs a bounded multi-turn Tool Loop. Initial context
  contains only the Project/target/constraints/Skill summaries; models, nodes and data facts load on
  demand. Rust requires Project, Label, Skills, nodes and Models to be inspected before Draft
  creation, executes every mutation/validation/Dry Run, feeds only bounded Tool payloads back to the
  provider, accumulates token usage, persists the audit, and can stop only at human approval.
  Provider prose/hidden reasoning is not persisted. The HTTP `llm` path now returns the same Agent
  Session/validation/Dry Run envelope as ScriptedMock.
- In progress: make failed validation and poor real Dry Run metrics trigger bounded Draft revisions
  and a second validation/Dry Run before approval.
- Next: add a real Crop verification revision for high-review Detection pipelines and preserve
  upstream Artifacts across retries.
- Recent Rust tests: `cargo test --workspace --all-features` — 221 passed, 0 failed.
- Recent Rust tests: Core 48, Application 29 and Server 9 passed; focused M4 multi-turn test and
  Application/Server Clippy with warnings denied passed.
- Recent Web tests: `npm run typecheck` passed; `npm test -- --run` — 36 passed, 0 failed.
- Recent E2E: inherited baseline only; not rerun in M0 yet.
- Recent commit: `904101f feat(agent): add constrained pipeline builder tool loop`.
- Release Blocking remaining: all Lean Agent Alpha A–G items until evidenced in
  `LEAN_AGENT_ACCEPTANCE.md`.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
