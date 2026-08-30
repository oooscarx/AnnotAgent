# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M8 complete — Lean Agent Alpha release evidence closed.
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
- M6 completed: Project Automation now owns the only Pipeline Builder entry. Its structured
  objective captures target Label, priority, Review target, cost/latency, external-model and human
  boundaries plus Agent/Dry-Run budgets. The server persists each real Tool action and the GUI polls
  that audit while work is active, exposes cancellation, validation, Dry Run, usage and stop state,
  and never renders invented progress.
- M6 completed: Rust computes a typed node/edge/model/policy Draft Diff. Users may Apply selected or
  Apply all changes through the normal Draft save boundary, reject the proposal, and restore the
  exact pre-apply Draft content with one-level Undo. A no-Draft Project first receives a real empty
  editable Draft so the Agent proposal still has a comparison target. Apply/Undo creates neither a
  Published Version nor a formal Run.
- M6 completed: the TUI `/advisor status` view shows the latest Builder objective, budget, status
  and ordered Tool audit; `/advisor cancel` addresses both new Pipeline Builder and compatibility
  Workflow Advisor sessions.
- M7 completed: the default RoboCup authoring surface exposes one lean football-bounding-box recipe.
  The Agent must load the enabled Domain Skill's bounded `advisor.md` resource before selecting a
  template, then chooses one ready Detection backend, selects/maps football detections, applies the
  hard-negative and field-relation Validators, and routes through Decision to Commit or Review.
  SAM, RF-DETR, LocateAnything, YOLO and multi-model recovery are alternatives in Labs unless their
  concrete binding is healthy and available; none is injected into the default Draft.
- M7 completed: explicit preferred-model selection is capability and health aware. A configured
  available `default-vision` binding creates a VLM Detection step, while an Unknown Labs worker is
  rejected. A real offline `lean-agent-robocup` demonstration loads Domain advice, repairs an
  invalid Draft, validates, Dry Runs labelled synthetic images and stops for human approval;
  output names ScriptedMock evidence and asserts zero Published Versions and zero formal Runs.
- M8 completed: Pipeline Builder now samples up to three available Project images in its offline
  sandbox loop. The release demo deliberately produces a measured Review overload, adds registered
  Crop Classification, validates again, repeats the three-image Dry Run, and stops for human
  approval. It still creates no Published Version or formal Run.
- M8 completed: Provider-turn and Tool-action budgets are independent. The generic Agent audit
  counts Tool actions against the Tool limit, while the live loop enforces Provider turns separately.
  A previously missing early-return persistence path was fixed so `budget_exceeded` survives SQLite
  reload and is rendered correctly after browser refresh.
- M8 completed: the five-minute course script is `docs/DEMO_LEAN_AGENT_ALPHA.md`; the unified
  acceptance script includes the new three-image Lean Agent demo and reports the correct release
  name.
- Next: no offline Release Blocking work remains. Run an optional external smoke only with an
  operator-supplied local credential/healthy Worker, never with conversation credentials.
- Full-workspace baseline: `cargo test --workspace --all-features` — 221 passed, 0 failed at M0.
- Recent Rust tests: `./scripts/acceptance.sh` passed on 2026-08-31: strict workspace Clippy, 238
  Rust unit/integration tests, all doc tests, all-feature build, doctor and four offline demos. The
  suite includes the persistent 100-image pause/restart/resume Batch, cancellation, exact checkpoint,
  Replay, Review, export formats, immutable Workflow and budget-refresh recovery.
- Recent Web tests: typecheck and production build passed; Vitest — 36 passed, 0 failed.
- Recent E2E: full Chromium guided suite — 26 passed, 0 failed. It covers generic Classification and
  Detection Agent paths, invalid repair, two-Dry-Run revision, human approval, cancellation, budget
  stop, refresh restoration, Diff/Undo, unavailable Backend exclusion, immutable Publish, generic
  isolation, Run/Review/Export and responsive/keyboard/error states.
- Recent milestone commit subject: `test(release): validate annotagent lean agent alpha` (pending the
  M8 local commit at this status update).
- Release Blocking remaining: none. A–G are evidenced PASS; external inference remains explicitly
  live-conditional rather than an offline release blocker.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation or release validation. External credentials/weights
  are not required for ScriptedMock, RuleBased, protocol, UI or test work.
