# Core and Skills

## Product entities

- A **Project** owns a Dataset, Annotation Schema, selected Skills, Workflows, default Workflow, model bindings, review settings, and exports.
- A **Skill** supplies registered domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It owns neither a Dataset nor the app shell.
- A **Workflow** belongs to a Project or reusable template and connects typed registered nodes. Published versions are intended to be immutable and pinned by Runs.

The current schema supports zero, one, or multiple enabled Skills and an exact selected Published
Workflow. Legacy one-Skill Projects retain a labelled compatibility path; general multi-Skill
execution uses the typed published DAG.

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

`annotagent.open_vocabulary_grounding` is a Capability Skill, not a domain pack. Its two registered
operations schedule `OpenVocabularyDetection` and `PhraseGrounding` against any compatible Model
Registry entry. `LocateAnything Local` is one optional HTTP Backend for those capabilities; its
brand does not appear in Core node kinds. Text-query validation, model execution, Core transforms,
and Project/domain policy remain separate layers.

`annotagent.object_detection` is the trained-detector Capability Skill. It accepts only an Image
plus target Labels, class mapping and bounded detection options, and produces a DetectionSet. YOLO,
RF-DETR or another trained detector can implement the same Model capability; none is a Core node
kind. The Skill does not own Crop, Review, Commit, or domain policy.

Core combines detector Artifacts through `core.match_detection_sets` and routes structured facts
through `core.evidence_gate`. Candidate Clusters retain each model contribution independently;
validation issues travel as upstream node metadata, and the Gate persists an explainable decision
report. See [Detection Evidence](DETECTION_EVIDENCE.md).

Expert model brands remain outside both Core nodes and Domain Skills. A versioned Manifest and the
Worker SDK register capabilities, typed Artifact contracts and availability evidence. The generic
Conversion Registry composes Detection → Box Prompt → Prompted Segmentation → Mask → Bounding Box
without a SAM-specific Core branch.

Model bindings belong to Projects and published Workflow snapshots. A Domain Skill can require or
optionally use capabilities without naming a Backend. This lets three Label Pipelines reference one
shared specialist node while a conditional Recovery node binds a separate open-vocabulary model.

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

## Geometry remains Core policy

Skills and model adapters may declare detection or prompted-segmentation capabilities, but Core owns
score semantics, geometry semantics, calibration keys, Project geometry policy, static safety,
quality reports, correction evidence, typed conversion lineage and improvement comparison. No Core
branch names YOLO, SAM, RoboCup or a concrete Label to decide whether geometry is accepted. Domain
Skills may add validators and Review reasons; they cannot reinterpret semantic confidence as IoU or
bypass the Project policy.
