# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M6 — Guided Project Automation UX, Draft Diff and undo.
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
- M5 completed: both the deterministic policy and live provider loop can create an invalid Draft,
  receive Rust validation errors, repair it, run the sandbox, inspect a bounded
  `AgentDryRunSummary`, and revise a high-Review Detection flow with Crop Classification. The
  revision is selected from healthy/available Registry Models, validates again, performs a second
  real Dry Run, records evidence-based rationale, and stops at explicit human approval. Failed and
  Review sample inspection is limited to five summaries and omits image bytes and Artifact bodies;
  node inspection exposes only status, output types, latency, cost and structured issues. Label
  Pipeline publish now runs Pipeline Grammar at the Application boundary.
- Next: project-local Agent entry/progress, structured objective controls, Draft Diff, selective
  apply/undo and the matching TUI session projection.
- Full-workspace baseline: `cargo test --workspace --all-features` — 221 passed, 0 failed at M0.
- Recent Rust tests: Core 49, Application 31 and Server 9 passed; focused M5 offline and 17-turn live
  revision loops passed. `cargo clippy --workspace --all-targets -- -D warnings` passed.
- Recent Web tests: `npm run typecheck` passed; `npm test -- --run` — 36 passed, 0 failed.
- Recent E2E: `npm run test:e2e` — 24 passed, 0 failed. The run also repaired stale
  accessibility/selectors for the collapsed Expert details drawer and current Guided action names.
- Recent milestone commit subject: `feat(agent): revise workflow drafts from validation and dry runs`.
- Release Blocking remaining: offline capability matrix and UX/Domain/course evidence in D–G;
  architecture, Agent authenticity and Pipeline safety are evidenced PASS.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
