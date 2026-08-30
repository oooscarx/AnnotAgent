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
- B. Agent authenticity: PARTIAL. The ScriptedMock and Application tool paths are real and audited;
  the live OpenAI-compatible policy remains M4.
- C. Pipeline safety: PARTIAL. Tool allow-list, Registry-bound mutations and Builder Grammar pass;
  wiring that grammar into every publish boundary remains M5.
- D. Offline capability: PARTIAL. ScriptedMock and Registry RuleBased paths are available; the full
  offline demo matrix is finalized in M8.
- E. UX: OPEN.
- F. RoboCup Domain boundary: OPEN.
- G. Course requirements: OPEN.

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
