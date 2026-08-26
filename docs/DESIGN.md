# Design Decisions

## Product scope

AnnotAgent is the product shell for composable annotation workflows for vision data. Projects are concrete data efforts, Skills are reusable domain extensions, Workflows are typed execution definitions, Models are bound resources, and Runs pin the configuration they execute. The bundled RoboCup implementation is one Skill and example Project, not the product identity.

The current Project schema still names one Skill and Runtime derives one task graph from it. The application/API/frontend compatibility DTO exposes `EnabledSkill`, `WorkflowSummary`, `WorkflowVersion`, `WorkflowStatus`, and `ModelBinding` without pretending that arbitrary multi-Skill or multi-Workflow execution is complete. The active Workflow shown in the UI is built from the actual configured tasks and dependencies.

## Workflow lifecycle and the LLM boundary

A target Workflow lifecycle is Draft → Valid → Published → Archived. Published versions are intended to be immutable and every Run must pin a Workflow version, Skill versions, and model bindings.

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

The LLM cannot invent or execute code, shell commands, unknown models, or unregistered nodes. Suggestion, editing, dry run, publishing, immutable snapshot persistence, and general graph execution remain roadmap work; the UI keeps those controls disabled. Rust static validation and human approval are mandatory boundaries before future publication and execution.

## Checked geometry

Coordinates use private fields and checked constructors, preventing invalid normalized values from reaching validators, exporters, and revision APIs. Conversion to pixels happens only at image-tool or export boundaries.

## Model proposes; Rust decides

The model cannot commit directly. It submits typed candidates. Runtime parses them, deterministic Skill code refines and validates them, and review policy selects accept, retry, review, or reject. The trace stores visible model content and structured actions, never hidden reasoning.

## Task-scoped tools and context

Runtime exposes generic tools plus Skill tools applicable to the current task. A task may spend one evidence/refinement call before it must submit. Prompt resources are loaded per task rather than injecting a whole Skill corpus.

## Persistence

SQLite provides local transactions and exportable audit history. Revision records append before/after snapshots. Money uses `rust_decimal::Decimal`. Existing Run history stores the Project snapshot, provider/model, Skill id, usage, annotations, and events; durable Workflow-version snapshots are not implemented yet.

## Frontends

CLI, TUI, and HTTP use `LocalApplication`; none duplicates the agent loop. React renders product DTOs and sends review/control requests. The server owns validation, state transitions, correction records, exports, and settings validation.

## Visual system

AnnotAgent Core owns the mark, tokens, generic components, semantic statuses, and `annotation-1` through `annotation-8`. A Skill may add a badge and a `SkillVisualProfile`. Label resolution is deterministic: Project override, stable Skill-id order, schema mapping, then stable label hash. The generic canvas contains no domain vocabulary.

Canonical sources live in `design/annotagent-visual-system/`. Vite delivery copies are separated into `web/public/brand/core/` and `web/public/brand/skills/<skill-id>/`.

## Assumptions

- A configured workspace is the local security boundary.
- Folder import is controlled copying; arbitrary external reads are not exposed over HTTP.
- OpenAI-compatible Chat Completions is the production network protocol in this release.
- Mock mode is the authoritative offline demo and CI fallback.
