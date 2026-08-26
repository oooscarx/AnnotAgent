# Core and Skills

## Boundary

AnnotAgent Core owns domain-neutral nouns and mechanics:

- typed IDs, checked normalized geometry, annotations, revisions, projects and task DAGs;
- provider, tool, validator, refiner, review-policy, exporter and Skill traits;
- the model/tool/validation loop, state control, budgets, events and registries;
- persistence interfaces and frontend application use cases.

The RoboCup Skill owns its labels, task graph, prompt resources, correction taxonomy, evidence tools, field/ball/robot validators, pixel refiner, and review policy. Searches for RoboCup label names in `annotagent-core`, `annotagent-runtime`, `annotagent-server`, and the generic GUI source return no matches. The GUI obtains the starter YAML and correction taxonomy from `DomainSkill` through `/api/skills`.

## Two extension levels

Declarative resources live under `skills/<id>`: the manifest describes identity, resources, task-to-resource routing, and correction taxonomy. A project YAML supplies labels, task kinds, attributes, dependencies, validators/refiners, review thresholds, and export preferences. Schema errors have precise paths and unknown fields are rejected.

Special algorithms implement object-safe Rust traits:

```rust
registry.register(Arc::new(MySkill::new()?))?;
```

`DomainSkill` returns its task templates, DAG, tools, validators, refiners, prompt resources, correction taxonomy, review policy, and optional starter project. `AgentTool::applicable_tasks` keeps domain tools out of unrelated model contexts.

## Proof that Runtime is not RoboCup-specific

`crates/annotagent-runtime/tests/skill_extension.rs` defines `DummySkill`, a validator, resource, review policy, and simple classification workflow entirely in test code. It registers the Skill in `SkillRegistry` and executes the normal Runtime without changing Runtime, Server, or GUI DTOs.

Run the proof:

```bash
cargo test -p annotagent-runtime --test skill_extension
```

## Resource loading

`ContextManager` starts with Core rules, a one-line Skill summary, project/task/image metadata, allowed task tools, remaining steps, and current usage. It then asks the Skill only for resources relevant to the current task. Corrections and issue summaries are compact context; full trace stays in SQLite.

## Why no dynamic plugin loader

Compile-time registration makes trait compatibility, algorithm safety, and classroom setup explicit. A future host could add signed WASM or dynamic components, but this release does not invent a package ecosystem before there is a deployment requirement.
