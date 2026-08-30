# Lean Agent Alpha Acceptance Evidence

Status values: `PASS`, `OPEN`, `LIVE-CONDITIONAL`, `NOT-IN-SCOPE`.

## M0 baseline

| Requirement | Status | Evidence |
|---|---|---|
| Git and remote inspected without mutation | PASS | `main...origin/main`; origin remains `git@github.com:oooscarx/AnnotAgent.git`. |
| Master task stored in repository | PASS | `docs/execution/LEAN_AGENT_MASTER_PROMPT.md`. |
| Full Rust baseline | PASS | `cargo test --workspace --all-features`: 221 passed, 0 failed on 2026-08-31. |
| Existing runtime capability inventory | PASS | `docs/LEAN_ARCHITECTURE_MIGRATION.md` records preserved contracts and compatibility policy. |
| Duplicate product concepts inventoried | PASS | Migration document records Select detections, Decision, Combine model evidence and Automation. |
| Unavailable backends inventoried | PASS | Ports 8790–8792 unavailable; registered external workers remain disabled/unconfigured. |

## Release matrix

- A. Architecture subtraction: PASS. Public Skills are the three generic capabilities; model brands
  are Backends; Guided Automation presents Select detections, Decision and Combine model evidence;
  Grid is Detection configuration; legacy Workflow, Provider and Artifact entry points converge on
  Project Automation, Settings / Models and Run Debug.
- B. Agent authenticity: PASS. ScriptedMock and live Provider paths both execute a multi-turn,
  audited invalid→repair→Dry Run→evidence revision→human approval loop through real Rust tools.
- C. Pipeline safety: PASS. Tool allow-list, Registry-bound mutations and Builder Grammar pass;
  Label Pipeline validation and publish both enforce the same grammar at the Application boundary.
- D. Offline capability: PASS. ScriptedMock, Registry RuleBased, generic Classification/Detection
  demos, RoboCup Domain demo and the three-image Lean Agent revision demo pass without a key.
- E. UX: PASS. The Agent is project-local, its persisted stages/Tools/validation/Dry Run/usage are
  observable and cancellable, and structured Diff Apply selected/Undo never publishes.
- F. RoboCup Domain boundary: PASS. The Domain owns advice and Validators, the default recipe uses
  one availability-qualified Detection backend, and unavailable Labs bindings are not selected.
- G. Course requirements: PASS. The five-minute guide, labelled three-image Mock demonstration,
  full regression evidence and explicit live-conditional boundary are complete.

Evidence is added per milestone; an item is not marked PASS merely because a type or button exists.

## M1 Capability and Model convergence

| Requirement | Status | Evidence |
|---|---|---|
| Exactly three public Capability Skills | PASS | Server API test asserts `annotagent.classification`, `annotagent.detection`, `annotagent.segmentation`. |
| Legacy Projects remain resolvable | PASS | Compatibility aliases remain Registry entries but are filtered from the public API; all 28 Application tests pass. |
| Model brands are not public Skills | PASS | Open-vocabulary/VLM/YOLO manifests are compatibility-only; public API test rejects their appearance. |
| SAM/RF-DETR/LocateAnything/YOLO are Labs while unavailable | PASS | Model Binding `availability_group` plus server API assertions. |
| Segmentation does not claim a runnable model | PASS | Generic Segmentation Skill has no node/template until a compatible healthy backend exists; unit test passes. |
| Example Project migration | PASS | Detection and hybrid examples use generic Capability IDs; old inline fixtures continue to test aliases. |
| Rust M1 regression | PASS | 28 Application tests and 9 Server tests pass; Capability crate tests pass. |
| Web M1 regression | PASS | TypeScript passes and 35 Vitest tests pass. |

## M2 public Pipeline vocabulary

| Requirement | Status | Evidence |
|---|---|---|
| Filter and Map Label merge in Guided UI | PASS | `guidedWorkflowNodes` and `guidedPipelineStepGroups` collapse adjacent technical operations under `Select detections`; Vitest verifies the projection. |
| Confidence and Evidence gates share Decision | PASS | Core `GuidedPipelineConcept` and Web title/group helpers map both gates to `Decision`; Rust and Web unit tests pass. |
| Candidate matching/merging is one concept | PASS | Attach, match and candidate-merge nodes project to `Combine model evidence`; their typed Runtime identities remain unchanged. |
| Expert node types remain available | PASS | Pipeline drawer moves IDs, ports, fallback and raw parameters under `Expert details`; technical graph editing remains available. |
| Grid is bounded Detection configuration | PASS | `GroundingAssistConfig` validates rows/columns in `[2,16]`; VLM adapter accepts nested `grounding_assist`, preserves the original image and adds only a calibration view. |
| Duplicate entry points converge | PASS | Navigation tests cover `/workflows`, `/providers`, `/settings/providers`, `/artifacts` and `/artifact-inspector` redirects to canonical Project Automation, Settings / Models and Run Debug routes. |
| ONNX is not shown as available | PASS | M1 Model registry evidence remains valid; M2 introduces no ONNX binding. |
| Rust M2 regression | PASS | Core 43, Provider 34, Application 28 and Server 9 tests passed with 0 failures. |
| Web M2 regression | PASS | TypeScript passed and 36 Vitest tests passed. |

## M3 Pipeline Builder Agent Core

| Requirement | Status | Evidence |
|---|---|---|
| Dedicated Builder session model | PASS | `PipelineBuilderSession`, constraints, statuses, stop reasons, backend kind, usage and audit envelope are typed and serialized in Core. |
| Registered tools only | PASS | `PipelineBuilderToolRegistry` contains 31 bounded tools and rejects Shell, Python, install, download, arbitrary URL and code execution names; unit test passes. |
| Draft mutations are Rust-owned | PASS | `PipelineDraftTools` validates mutable status, Registry node/model identity, capability, enabled Skill, typed connections and cycle safety. |
| Pipeline Grammar is enforced in Rust | PASS | `PipelineGrammarValidator` composes static validation with Commit count, Decision-before-Commit, uncertainty route, fallback depth, model-call budget, external-model and forbidden-node rules. |
| Budget and stop boundaries | PASS | Core test covers maximum turns, tool calls, Dry Runs and the explicit human-approval stop; cost is inherited by the audit budget. |
| Tool result separation | PASS | `AgentToolResult` separates persisted payload reference, model payload and display summary; Application audit assertions verify the shape. |
| ScriptedMock complete policy | PASS | State-machine test verifies invalid validation, repair, first Dry Run high review rate, Crop verification addition, second validation/Dry Run and human submission in order. |
| Real Application invalid→repair | PASS | Application integration disconnects the Commit edge through `PipelineDraftTools`, observes `valid=false`, reconnects it, observes `valid=true`, performs sandbox Dry Run and stops for approval without publishing. |
| Core M3 regression | PASS | 48 Core tests passed, 0 failed. |
| Application/HTTP/persistence M3 regression | PASS | Application 28, Server 9, Storage unit 9 and Storage integration 16 tests passed; 0 failed. |
| M3 lint gate | PASS | Clippy passed for Core, Application and Server all targets with warnings denied. |

## M4 OpenAI-compatible multi-turn provider

| Requirement | Status | Evidence |
|---|---|---|
| Live Advisor is a real Tool Loop | PASS | `run_workflow_advisor_with_provider` repeatedly sends assistant Tool Calls and Rust Tool Results until approval/stop; the old `submit_workflow_advice` one-shot path was removed. |
| Context is progressively loaded | PASS | Initial prompt has only bounded Project/target/constraints/Skill summaries; Models, nodes, Label Schema and image metadata require explicit read tools. No image library, Run history or Artifact corpus is injected. |
| Required inspection precedes Draft creation | PASS | Application state rejects `create_draft_from_template` until Project, target Label, enabled Skills, available nodes and Models were inspected. Integration test starts with an early rejected create and then recovers from the Tool error. |
| Provider can mutate only through Rust tools | PASS | Exposed schemas contain Registry-enumerated node/model IDs and bounded parameters; `PipelineDraftTools` and Pipeline Grammar own changes and validation. |
| Validation and Dry Run are real tools | PASS | Live integration persists a real Draft, runs Rust validation and the sandbox Runtime, then returns bounded summary metrics to the next provider turn. |
| Human boundary is explicit | PASS | Submission requires a valid report and a completed Dry Run; session stops `waiting_for_human`. Integration test asserts zero Published Versions. |
| Usage and audit persist per call | PASS | Mock-provider integration verifies 11 provider turns, 110 input/55 output tokens, rejected/successful Tool actions and persisted Agent Session. |
| Hidden reasoning is not stored | PASS | Assistant prose is retained only in the transient provider context; persisted history contains Tool name, arguments, structured result, timestamps, success and aggregate usage. |
| HTTP LLM response exposes Agent envelope | PASS | Server `llm` branch now returns `agent_session`, validation, Dry Run and approval state just like ScriptedMock. |
| Provider retry/cancellation boundary | PASS | OpenAI-compatible Provider owns bounded transport retries; loop checks cancellation before every provider and Tool step and reports Provider failures as terminal session reasons. |
| Real external request | LIVE-CONDITIONAL | Requires an operator-provided configured credential; no conversation key was read or used. |
| M4 regression/lint | PASS | Application 29 and Server 9 tests passed; focused 11-turn integration and Clippy with warnings denied passed. |

## M5 validation and Dry Run revision loop

| Requirement | Status | Evidence |
|---|---|---|
| Invalid Draft is repaired from validator output | PASS | Offline and live Application tests disconnect the typed Commit edge, observe validation `false`, reconnect the exact typed ports, then observe `true`. |
| Dry Run feedback is structured and bounded | PASS | `AgentDryRunSummary` contains image/result/review/failure/warning/model-call/latency/cost fields; sample inspection permits 1–5 records and excludes image bytes and full Artifact values. |
| Poor quality triggers a real Draft revision | PASS | A project with a 0.99 acceptance threshold produces one real mock detection in Review; the Agent records 1/1 (100%), adds Crop → Classification → Attach Result, and cites that observation in rationale. |
| Revision uses Registry models | PASS | Crop verification selects a healthy or explicitly available Classification Model Descriptor; no arbitrary model ID or unavailable Labs backend can be inserted. |
| Revision validates and runs again | PASS | Both M5 tests assert validation outcomes `[false, true, true]` and two sandbox Dry Runs; the second result moves the candidate from Review to auto-accepted. |
| Bounded sample and node inspection | PASS | Live tool schemas and executor limit failed/Review/node results to five; the 17-turn test asserts Review output has no full `value` Artifact body. |
| Human approval boundary | PASS | Final state is `waiting_for_human`; zero Published Versions and zero formal Runs are asserted for both M5 projects. |
| Publish boundary enforces Builder Grammar | PASS | `validate_workflow_draft` adds Builder Grammar issues for Label Pipelines and `publish_workflow` invokes the publish-ready form of that same boundary. |
| Complete live Mock Provider loop | PASS | 17 provider calls, 17 registered Tool Calls, 170 input/85 output tokens, full persisted history and explicit approval stop pass offline. |
| M5 regression/lint | PASS | Core 49, Application 31 and Server 9 passed; Web typecheck and 36 Vitest tests passed; 24 Playwright E2E tests passed; workspace Clippy all targets passed with warnings denied. |

## M6 Guided Project Automation UX

| Requirement | Status | Evidence |
|---|---|---|
| Agent is integrated in Project Automation | PASS | The bounded objective and `Ask AnnotAgent` action live in `Project → Build → Automation`; no standalone chat or Agent route was added. |
| Structured objective reaches Rust | PASS | Priority, per-image cost/latency/model-call limits, Review target, external-model/human policy and Agent turn/tool/Dry-Run/cost limits are serialized as `PipelineBuilderConstraints`, validated and stored with the Agent Session. |
| Progress is server truth | PASS | Application persists the Session before work and after every registered Tool Call; Web polls the project Agent audit during execution and renders status, stage, budgets, token/cost usage, validation, Dry Run, stop reason and ordered actions. |
| GUI and TUI cancellation | PASS | Web calls the existing session cancellation endpoint; Application cancels the active token and persists `cancelled`. TUI `/advisor cancel` selects Pipeline Builder or compatibility Advisor sessions. |
| Structured Draft Diff | PASS | Rust `PipelineDraftDiff` emits stable node, parameter, edge, model-binding and policy change IDs; unknown/empty selections, cross-Project comparisons and immutable bases are rejected. |
| Apply selected and Apply all | PASS | Both actions call the Application-owned selective apply boundary and save into the existing editable Draft ID; the Agent proposal remains a separate auditable Suggested Draft. |
| Reject and Undo | PASS | Reject mutates no Draft. Apply returns the exact previous snapshot; one-level GUI Undo restores it through the normal PATCH/save boundary. Application tests assert zero Published Versions and zero formal Runs. |
| Human Publish boundary | PASS | The proposal can only be applied to an editable Draft. Test & Activate remains a separate user navigation and Pipeline Grammar/publish validation remain server-owned. |
| Guided internal-ID boundary | PASS | Default Diff rows use human node/action names; node IDs, ports and raw configuration remain in Expert details/Debug. Tool protocol names are visible only in the explicitly expandable Tool actions audit. |
| First-Draft experience | PASS | If no editable Current Draft exists, Automation creates a real empty Draft before invoking the Agent, then diffs the proposal against it; it does not fake a client-only baseline. |
| M6 Rust regression | PASS | Core 50, Application 32, Server 9 and TUI 6 tests passed; focused selective apply/Undo and strict workspace Clippy with warnings denied pass. |
| M6 Web/E2E regression | PASS | TypeScript, production build and 36 Vitest tests passed. Guided Chromium suite: 25 passed, including final Agent trace and real Apply selected → persisted DAG change → Undo restoration. |

## M7 minimal RoboCup Ball Domain

| Requirement | Status | Evidence |
|---|---|---|
| One lean default workflow | PASS | Compatibility RoboCup exposes only `robocup.ball.vlm-bootstrap`; the Draft contains one Detection model call followed by football selection, both Domain Validators and Decision/Review/Commit. |
| Domain Advisor resource is real | PASS | `resources/advisor.md` is declared by the Pack and Ball manifests. Registry loading is traversal safe, bounded by the Application, recorded as `load_skill_resource`, and required before Domain template creation. |
| Domain Validators remain domain-owned | PASS | Hard-negative and field-relation Validators are registered by RoboCup; generic Core/Application contains no football-label branching. |
| Model availability affects selection | PASS | Application test selects ready `default-vision` for VLM Detection and rejects an enabled but Unknown LocateAnything Labs binding as not ready. |
| Labs are not default recommendations | PASS | The default Draft test rejects SAM, recovery, Crop and multiple model nodes; explicit specialist/hybrid templates remain compatibility-only alternatives. |
| Offline Agent demonstration | PASS | `cargo run -p annotagent -- demo lean-agent-robocup` loads advice, repairs/validates, Dry Runs three labelled images, revises with Crop Classification, Dry Runs again and stops `waiting_for_human`; it prints `offline ScriptedMock` and `labelled_mock_evidence`, with zero Publish/formal Run. |
| External Qwen request | LIVE-CONDITIONAL | No operator credential was available under the task's credential restrictions; no conversation API Key was read or used. |
| M7 Rust regression | PASS | Application 34, Runtime 35, Server 9 and RoboCup 17 tests passed with zero failures after the lean one-template expectation was updated. |

## M8 release and course evidence

| Requirement | Status | Evidence |
|---|---|---|
| Full Rust checks and build | PASS | `./scripts/acceptance.sh` passed `cargo fmt --all -- --check`, strict all-target/all-feature Clippy, 238 Rust unit/integration tests plus doc tests, and the all-feature workspace build. |
| 100-image Batch and Pause/Resume | PASS | `persistent_batch_pauses_restarts_and_resumes_one_hundred_images` completed 100 unique children after pause, application reopen and resume; storage lease/budget/cancel tests also passed. |
| Replay and checkpoint | PASS | Published DAG branch/cache/replay tests and `crop_classification_replay_keeps_shared_detector_checkpoint` passed; classifier Replay preserves the shared detector checkpoint. |
| Review | PASS | Runtime suspension/resume, server review revision/budget flow and keyboard-operable Review browser journey passed. |
| Export | PASS | Native, COCO, LabelMe and YOLO round trips plus the unresolved-Review → acceptance → durable Export E2E passed. |
| Web checks | PASS | TypeScript, 36 Vitest tests and the production Vite build passed. |
| Required browser journeys | PASS | 26/26 Chromium scenarios passed, covering all 14 required Agent/immutability/generic/Run-Review-Export paths, responsive states and server error recovery. |
| Budget refresh recovery | PASS | New Application test persists `budget_exceeded`; browser test reloads Project activity and still shows `Stopped at budget`. |
| Three-image evidence revision | PASS | Offline Lean demo reports two `dry_run_pipeline` calls over three labelled synthetic images, a registered `add_pipeline_node` revision, valid final Draft and explicit human stop. |
| Course demonstration | PASS | `docs/DEMO_LEAN_AGENT_ALPHA.md` provides the timed five-minute script, preflight command, human boundary and Mock/live truth labels. |
| Unified release command | PASS | `./scripts/acceptance.sh` passed boundary and secret scans, doctor and all four offline demos, ending `AnnotAgent Lean Agent Alpha acceptance checks completed successfully.` |
| Real external inference | LIVE-CONDITIONAL | Qwen and external/local-weight Workers require operator-supplied credentials/services. No conversation key or weight was used. |
