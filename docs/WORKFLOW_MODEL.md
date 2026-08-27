# Workflow Model

AnnotAgent models a Workflow as a strongly typed directed acyclic graph owned by a Project. A mutable `WorkflowDraft` contains generic nodes, typed input/output ports, explicit edges, resource requirements, bounded retry and fallback policies, review gates, and registered Model/Validator/Refiner references.

The lifecycle is:

```text
Suggested or blank Draft
→ static validation
→ human editing
→ selected-image Dry Run
→ immutable PublishedWorkflowVersion
→ image Run or Dataset Batch
```

Static validation rejects cycles, unreachable terminal paths, unknown registry IDs, incompatible Artifact ports, missing capabilities, invalid retry/fallback graphs, and unsafe Commit paths. Errors include the exact node or port path. A Draft can be saved while invalid, but cannot be published.

Publishing freezes the Draft, enabled Skill versions, referenced model descriptors, and prompt/resource versions. The stable content hash excludes lifecycle timestamps but includes semantic graph content. Published versions cannot be edited; clone creates a new Draft. Every selected-version Run stores the full `PublishedWorkflowVersion` and a post-execution checkpoint, so later Draft changes cannot alter history.

Generic node kinds include image input, transform, vision model, vision-language model, deterministic tool, candidate merge, validator, refiner, gate, human review, commit, and export. Core knows Artifact types and scheduling semantics, not domain labels or model product names.

The Web Workflow page implements Draft creation, registry-bounded Mock or optional LLM suggestion, node and edge editing, parameters, bindings, retry, fallback and review configuration, validation, selected-image Dry Run, publish, clone, archive, comparison, and exact version selection for image Runs and Dataset Batches.

