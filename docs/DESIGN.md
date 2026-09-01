# Design Decisions

## Product scope

AnnotAgent is the product shell for composable annotation workflows for vision data. Projects are concrete data efforts, Skills are reusable domain extensions, Workflows are typed execution definitions, Models are bound resources, and Runs pin the configuration they execute. The bundled RoboCup implementation is one Skill and example Project, not the product identity.

Project schema accepts zero, one, or multiple enabled Skills. The application/API/frontend expose independent Project, Skill, Workflow, Model, Run, Batch, and Review contracts. An unselected legacy single-Skill Project can still derive its compatibility graph; exact Published Workflow selection uses the generic DAG Runtime.

## Workflow lifecycle and the LLM boundary

The Workflow lifecycle is Draft → Valid → Published → Archived. Published versions are immutable and exact-version Runs pin Workflow, Skill, model, and prompt/resource snapshots.

An LLM may only suggest a constrained Workflow Draft from registered data:

```text
Project Schema
→ Available Skills
→ Node Catalog
→ Model Registry
→ LLM Workflow Suggestion
→ Rust Static Validation
→ Human Editing
→ Dry Run
→ Publish
→ Execute
```

The LLM cannot invent or execute code, Shell commands, unknown models, or unregistered nodes. Suggestion, editing, Dry Run, publication, immutable snapshot persistence, and general graph execution are implemented. Rust static validation and explicit human Publish remain mandatory boundaries.

## Checked geometry

Coordinates use private fields and checked constructors, preventing invalid normalized values from reaching validators, exporters, and revision APIs. Conversion to pixels happens only at image-tool or export boundaries.

## Model proposes; Rust decides

The model cannot commit directly. It submits typed candidates. Runtime parses them, deterministic Skill code refines and validates them, and review policy selects accept, retry, review, or reject. The trace stores visible model content and structured actions, never hidden reasoning.

Open-vocabulary grounding and trained Object Detection are Model capabilities behind the same
versioned Worker boundary. Candidate Match and Evidence Gate are generic Core operations; model
brands and RoboCup policy remain outside Core. Independent detector boxes and score semantics are
retained, and a bounded Recovery Agent may call one configured fallback only after an observable
evidence decision and budget check.

## Task-scoped tools and context

Runtime exposes generic tools plus Skill tools applicable to the current task. Model turns, tool calls, recovery turns, Provider timeout, task timeout, and retry are separate bounded budgets. Repeated deterministic calls can reuse cached Artifacts; available actions narrow according to state without permanently forcing submit after the first tool call.

Detection Cache Keys include image content identity, Registry model/version/checkpoint/protocol,
node configuration, query text and Project Label mapping. Editing only a downstream Gate leaves
detector keys unchanged; Replay preserves checkpointed ancestors and never persists a second Commit.

## Persistence

SQLite provides local transactions and exportable audit history. Revision records append before/after snapshots. Money uses `rust_decimal::Decimal`. Run history stores Project and immutable Workflow snapshots, provider/model identity, node/task state, typed Artifacts, usage, annotations, validation, events, and checkpoint. Dataset Batches add a durable queue, leases, monotonic events, and exact consumed/reserved budget ledger.

## Expert model boundary

Provider models and Vision Workers share capability-driven selection but keep different connection
lifecycles. An Expert Model Manifest declares identity, Capability, typed input/output and prompt
contracts, score/geometry semantics, runtime/license facts and observed availability. Only complete
health, protocol, contract, weights and selected-image conversion evidence makes a Worker
publishable. Core owns neither model brands nor Worker process launch policy.

Prompted geometry refinement is an explicit auditable chain:
DetectionSet → BoxPromptSet → MaskSet → refined DetectionSet. Every conversion keeps parent/item
references; Replay can resume from the refiner without rerunning an upstream detector.

## Frontends

CLI, TUI, and HTTP use `LocalApplication`; none duplicates the agent loop. React renders product DTOs and sends review/control requests. The server owns validation, state transitions, correction records, exports, and settings validation.

## Guided product state

The Project workspace is a projection over persisted product truth, not a second lifecycle. `ProjectWorkspaceSummary` combines Project facts, readiness, blockers, an ordered eight-step Journey, and exactly one recommended primary action. React renders that decision; it does not infer the next action from local UI state.

The guided path is Create → Data → Labels → Automation → Test & Activate → Full Run → Review → Export. Build step, Run Results/Debug context, Review selection, explicit global filters, and Export destination are canonical URL state. Active execution, sample-test evidence, queue progress, and Export readiness are restored from SQLite/Application projections. SSE recovery performs a full resynchronization after any interrupted connection.

Results are outcome-first. Technical graph JSON, IDs, node payloads, Provider request context, and Replay remain available through Expert or Debug views. Error surfaces describe the failed action, preserve server ownership of saved data, and reload the latest server state before retrying.

## Visual system

AnnotAgent Core owns the mark, tokens, generic components, semantic statuses, and `annotation-1` through `annotation-8`. A Skill may add a badge and a `SkillVisualProfile`. Label resolution is deterministic: Project override, stable Skill-id order, schema mapping, then stable label hash. The generic canvas contains no domain vocabulary.

Canonical sources live in `design/annotagent-visual-system/`. Vite delivery copies are separated into `web/public/brand/core/` and `web/public/brand/skills/<skill-id>/`.

## Assumptions

- A configured workspace is the local security boundary.
- Folder import is controlled copying; arbitrary external reads are not exposed over HTTP.
- OpenAI-compatible Chat Completions is the production network protocol in this release.
- Deterministic protocol fixtures are test-only evidence for offline CI; product-generated runnable
  Drafts never bind Mock Providers or present fixture output as live model inference.

## Geometry safety boundary

Model operation metadata declares score and geometry semantics, Project policy declares required
quality, Rust validation blocks unsafe candidate-to-Commit paths, and reviewed Dry Runs supply
measured evidence. The GUI projects those same persisted contracts as Model score, Box quality and
Geometry verification. Improve Automation patches an immutable baseline and requires a separate
holdout comparison plus human-selected Draft application; publication remains outside the Agent.
