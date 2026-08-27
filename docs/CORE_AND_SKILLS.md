# Core and Skills

## Product entities

- A **Project** owns a Dataset, Annotation Schema, selected Skills, Workflows, default Workflow, model bindings, review settings, and exports.
- A **Skill** supplies registered domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It owns neither a Dataset nor the app shell.
- A **Workflow** belongs to a Project or reusable template and connects typed registered nodes. Published versions are intended to be immutable and pinned by Runs.

The current schema supports one configured Skill and one compatibility Workflow. The broader DTO is a migration contract, not a claim that general multi-Skill graph execution already exists.

## Core boundary

AnnotAgent Core owns typed IDs, checked geometry, annotations, revisions, task DAGs, provider/tool/validator/refiner/review/export traits, the bounded model loop, budgets, events, registries, persistence interfaces, and frontend use cases. It contains no production domain labels.

Declarative Skill resources live under `skills/<id>`. A Project YAML supplies labels, task kinds, attributes, dependencies, validator/refiner selections, review thresholds, and exports. Schema errors have precise paths and unknown fields are rejected.

The layered registry distinguishes `Capability`, `Domain`, and `Pack` manifests. Every manifest has
an implementation version, dependencies, conflicts, capabilities, extension declarations,
templates, resources and taxonomy. The unified object-safe `Skill` trait exposes optional nodes,
tools, Validators, Refiners, templates, resources and correction taxonomy. The original
`DomainSkill` contract remains as an explicit compatibility adapter while bundled extensions
migrate.

Layered resolution validates exact Alpha versions, missing dependencies, conflicts and duplicate
registrations before a Workflow is authored or run. Resource requests accept only manifest-declared
relative names; absolute paths, traversal and undeclared resources are rejected before Skill code
runs.

Special algorithms implement object-safe Rust traits and are registered explicitly:

```rust
registry.register(Arc::new(MySkill::new()?))?;
```

`DomainSkill` returns task templates, DAG, tools, validators, refiners, prompt resources, taxonomy,
review policy, and an optional starter Project. `AgentTool::applicable_tasks` keeps domain tools out
of unrelated contexts.

## Visual profiles

The canvas only knows generic slots and patterns. A `SkillVisualProfile` maps domain labels into those slots. Multiple profiles are allowed in the frontend contract, and conflicts use Project override → Skill mapping sorted by stable Skill id → schema mapping → stable hash fallback. Array registration order never chooses a color.

Runtime proof remains `crates/annotagent-runtime/tests/skill_extension.rs`, which registers a `DummySkill` and executes normal Runtime without changing Core, Runtime, Server, or canvas code.

## Why no dynamic plugin loader

Compile-time registration makes compatibility and algorithm safety explicit. Signed WASM or dynamic components may be added later, but this release does not claim a package ecosystem.
