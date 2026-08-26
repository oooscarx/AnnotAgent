# AnnotAgent Product Hierarchy

AnnotAgent is the product. It is a local-first platform for composing auditable annotation workflows for vision data.

```text
AnnotAgent
├── Projects
├── Workflows
├── Models
├── Skills
├── Runs
└── Review
```

RoboCup is not the application identity. `robocup` is one registered Domain Skill, `robocup-demo` is an example Project that enables that Skill, and the RoboCup package supplies domain nodes, validators, refiners, prompt resources, correction taxonomy, and label visuals.

## Product entities

### Project

A Project is one concrete annotation effort. It owns or references:

- a Dataset;
- an Annotation Schema;
- zero, one, or more enabled Skills;
- one or more Workflow drafts or published Workflow versions;
- one default Workflow version;
- Model Bindings used by Workflow nodes;
- Runs, reviews, revisions, and exports produced in that Project context.

A Project is not a Skill. A Dataset is never owned by a Skill.

### Skill

A Skill is a reusable domain extension. It may provide:

- registered node templates;
- Validators and Refiners;
- bounded tools and Prompt Resources;
- Workflow templates;
- correction taxonomy;
- label-to-visual-slot mappings.

A Skill does not own a Dataset, a Project page, the global navigation, or the AnnotAgent brand. The current repository has one production Skill, `robocup`, plus the test-only `DummySkill` extension proof.

### Workflow

A Workflow belongs to a Project or exists as a reusable template. It is a typed graph of registered nodes and connections. A Workflow can be saved as a mutable Draft and published as an immutable version. Every Run must eventually pin an exact published Workflow version, Skill versions, and Model Bindings.

The intended status model is:

```text
Draft → Valid → Published → Archived
```

Publishing creates a new immutable version; it never mutates a version already referenced by a Run.

### Model

A Model Registry entry identifies a provider/model capability. A Project-level Model Binding gives a stable local name to that registry entry so Workflow nodes do not embed credentials or provider secrets. Credentials remain in the operating-system keychain or configured environment.

### Run and Review

A Run is an auditable execution pinned to Project, Workflow version, Skill versions, Model Bindings, input image, budget, and provider metadata. Review is a cross-project queue of candidates that require a human decision. Review revisions and correction records remain attached to their Project and Run.

## Current implementation truth

`ProjectSchema v1` currently has one `project.skill`, one `project.skill_version`, a top-level `tasks` list, and one Runtime execution path. The Runtime obtains its topological order from `DomainSkill::workflow()` and uses the Project task list for task configuration. It does not yet execute an arbitrary Project-owned multi-Skill graph.

This product-shell refactor therefore exposes an honest compatibility view:

- the configured Skill becomes one `EnabledSkill`;
- the real top-level Project task definitions become the nodes of one current Workflow view;
- that Workflow is marked `Published` because it is the fixed configuration used by current Runs;
- its static validation state is derived from the same Rust Project/Skill validation already required before execution;
- the configured provider/model becomes the current Model Binding;
- Workflow edit, LLM suggestion, dry run, publish, version selection at Run start, and arbitrary mixed-Skill execution remain disabled and are documented as roadmap work.

The compatibility view is not a claim that the missing Workflow Runtime exists.

## API and frontend DTO contract

The HTTP boundary exposes the following product concepts without changing the v1 execution contract:

```text
WorkflowStatus = draft | valid | published | archived

EnabledSkill {
  id
  display_name
  version
}

ModelBinding {
  id
  provider
  model
  role
}

WorkflowNodeSummary {
  id
  node_type
  depends_on[]
  model_binding?
  validators[]
  refiners[]
  human_review_gate
  fallback?
}

WorkflowVersion {
  workflow_id
  name
  version
  status
  validation_status
  is_default
  nodes[]
}

WorkflowSummary {
  id
  name
  current_version
  status
  validation_status
  is_default
  node_count
}
```

`ProjectSummary` includes its description, Dataset summary, Annotation Schema task/kind summary, enabled Skills, active Workflow, available Workflow versions, Model Bindings, image count, and recent Run. Legacy `skill_id` remains temporarily available for compatibility but is not the product model.

Run history currently stores a serialized Project schema, provider, model, and single Skill id. The Web Run view derives the compatibility Workflow and binding from those real persisted fields. Persisting first-class Workflow version and binding snapshots requires a storage migration and remains roadmap work.

## Workflow creation boundary

An LLM may only suggest a constrained Workflow Draft. It cannot directly execute a draft, introduce unknown nodes, generate arbitrary code, or invoke Shell commands.

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

Rust static validation must reject unknown nodes, invalid typed connections, unavailable Skill versions, missing Model Bindings, cycles, and policy violations before a draft can become publishable. Human editing and an explicit publish action remain required. LLM Workflow Suggestion, editing, dry run, and publishing are not implemented in this release.

## Visual and brand hierarchy

Runtime assets are layered as:

```text
web/public/brand/
├── core/
│   ├── AnnotAgent logo and mark
│   ├── app, favicon, PWA, and Open Graph assets
│   └── generic icons
└── skills/
    └── robocup/
        ├── Skill badge and example lockup
        └── label visual profile
```

The generic Annotation Canvas consumes `annotation-1` through `annotation-8`. A Project resolves visuals in deterministic priority order:

1. Project explicit override;
2. enabled Skill visual profile;
3. Annotation Schema mapping;
4. stable label hash fallback.

Array order is never used to assign colors. Domain labels are not compiled into the generic Canvas.

## RoboCup example

Recommended relationship:

```text
Skill
  id: robocup
  display_name: RoboCup Perception

Example Project
  id: robocup-demo
  name: RoboCup Demo Dataset
  enabled_skills:
    - robocup@1

Current compatibility Workflow
  id: accurate-hybrid
  version: 1
  status: published
```

RoboCup may appear in the example Project, enabled Skill list, Project-scoped Workflow nodes, Review labels, Skill catalog entry, and example documentation. It must not appear in the application name, global navigation, generic empty/error states, or no-Project TUI state.

## Migration sequence

1. Establish the generic AnnotAgent product shell and compatibility DTOs.
2. Persist Project-owned Workflow drafts separately from immutable published versions.
3. Add a Node Registry and typed-edge static validator independent of any one Skill.
4. Snapshot Workflow version, enabled Skill versions, and Model Bindings into each Run.
5. Add human editing, dry run, publish, and explicit Run version selection.
6. Only then enable arbitrary mixed-Skill Workflow execution.

Until those steps exist, UI actions for Suggest with LLM, Edit Workflow, Dry Run, and Publish stay disabled with an explicit explanation.
