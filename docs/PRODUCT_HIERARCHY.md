# AnnotAgent Product Hierarchy

AnnotAgent is the product: a local-first system for composing auditable annotation workflows for vision data.

```text
AnnotAgent
├── Projects
│   ├── Dataset
│   ├── Annotation Schema
│   ├── Enabled Skills
│   ├── Model Bindings
│   ├── Workflow Drafts and Published Versions
│   └── Runs, Review, revisions, imports and exports
├── Workflow and Model registries
└── reusable Skills
```

## Independent entities

A **Project** is one annotation effort. It owns its Dataset, Schema, enabled Skill references, Model Bindings, Workflows, review policy, Run history, and data interchange. Its screen reports `active_run`/`active_batch` separately from `last_run`; the Project itself is never “Running” or “Failed.”

A **Skill** is a reusable domain extension. It contributes namespaced nodes, Validators, Refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It does not own a Dataset, Project page, global navigation, or brand. Projects may enable zero, one, or multiple Skills; collision resolution and visual precedence are deterministic and tested.

A **Workflow** is a Project-owned strongly typed DAG. Drafts are mutable and can be invalid while edited. Publishing creates a content-addressed immutable version with frozen Skill, Model, and prompt/resource snapshots. Image Runs and Dataset Batches can select an exact version, execute it through the generic DAG Runtime, and persist the complete version plus checkpoint.

A **Model** descriptor declares backend identity, capability, typed inputs/outputs, cost, health, limits, endpoint/path metadata, and a secret reference. Workflow nodes bind to descriptor IDs; GUI-managed credentials use the native system credential store, while environment and session-only references remain available. Model product names and domain labels are not Core scheduling concepts.

A **Run** is one image execution pinned to Project, Workflow, model identity, input, budget, state, node trace, Artifacts, validation, and usage. A **Dataset Batch** coordinates many child Runs with a durable queue, global exact budget, leases, pause/resume/cancel, and restart checkpoint. **Review** appends human revisions and correction records instead of overwriting history.

## Product lifecycle

```text
Project Schema + Skills + Model/Node catalogs
→ constrained Advisor suggestion or blank/template Draft
→ human edit
→ Rust static validation
→ selected-image Dry Run
→ immutable Publish
→ image Run or Dataset Batch
→ Review
→ Native / COCO / YOLO / LabelMe export or import
```

The optional LLM Advisor may reference only registered components and cannot emit or execute arbitrary code, Shell, URLs, or unknown tools. Rust validation and explicit human Publish remain authoritative.

The default Web journey presents the same lifecycle in user language:

```text
Create Project → Data → Labels → Automation → Test & Activate
→ Run Dataset → Review results → Export dataset
```

Project Overview is the navigation hub. Its server-owned Guidance chooses one primary action and explains blockers; Build owns Data, Labels, Automation, and Sample Test; Run Detail owns Results and Debug; Review owns human decisions; Export owns readiness, format compatibility, and the terminal report. Workflows, Models, Skills, and Artifacts remain real independent entities but are not competing global destinations in the guided path.

Global navigation is limited to Home, Projects, Runs, Review, and Settings. Project-scoped routes carry the Project identity. Global Runs and Review are never silently filtered by a remembered Project; scope is explicit in the URL.

## Visual and domain boundary

The generic Canvas consumes stable visual slots resolved from Project override, enabled Skill profiles, Schema mapping, then label hash fallback. Domain labels are not compiled into the Canvas.

RoboCup is one ordinary `robocup` Skill plus an example Project. Its labels, pixel algorithms, hard-negative rules, prompt resources, templates, correction taxonomy, badge, and colors may appear only in that Skill/example context—not in the AnnotAgent identity, generic empty states, or domain-neutral Core.

## Compatibility boundary

Legacy schema-v1 single-Skill Projects may still start without explicitly selecting a published version; those Runs identify `legacy_agent_runtime` honestly. Zero- or multi-Skill Projects select a Published Workflow. See [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) for the remaining product boundaries.
