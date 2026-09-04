# Workflow Model

AnnotAgent models a Workflow as a strongly typed directed acyclic graph owned by a Project. A mutable `WorkflowDraft` contains generic nodes, typed input/output ports, explicit edges, resource requirements, bounded retry and fallback policies, review gates, and registered Model/Validator/Refiner references.

The lifecycle is:

```text
Suggested or blank Draft
→ static validation
→ human editing
→ immutable selected-image Sample Test bound to exact Draft revision/hash
→ immutable PublishedWorkflowVersion
→ image Run or Dataset Batch
```

Static validation rejects cycles, unreachable terminal paths, unknown registry IDs, incompatible Artifact ports, missing capabilities, invalid retry/fallback graphs, and unsafe Commit paths. Errors include the exact node or port path. A Draft can be saved while invalid, but cannot be published.

Every Draft carries a server-owned revision and semantic content hash. Saves use optimistic
concurrency; a stale tab receives 409 and can compare, reload, or preserve its work as a new Draft
without overwriting the newer revision. Sample Tests are append-only and bind the exact Draft
revision/hash, stable image-set hash, and resolved model-snapshot hash. Publishing is rejected
unless the current exact Draft has passing or explicitly human-approved evidence.

Publishing freezes the Draft, enabled Skill versions, referenced model descriptors, and prompt/resource versions. The stable content hash excludes lifecycle timestamps but includes semantic graph content. Published versions cannot be edited; clone creates a new Draft. Every selected-version Run stores the full `PublishedWorkflowVersion` and a post-execution checkpoint, so later Draft changes cannot alter history.

Generic node kinds include image input, transform, vision model, vision-language model, deterministic tool, candidate merge, validator, refiner, gate, human review, commit, and export. Core knows Artifact types and scheduling semantics, not domain labels or model product names.

The Web Workflow page implements blank/template or Registry-bounded LLM Draft creation,
human-readable Recipe editing, parameters, bindings, retry/fallback/review configuration,
validation, selected-image Sample Test, publish, clone, archive, comparison, and exact version
selection for Image Runs and Dataset Runs. The technical graph is a read-only projection until
typed port editing, cycle prevention, safe deletion, and undo can ship as one complete contract.
Production suggestions and formal Runs never substitute Mock Providers for missing live capability.
