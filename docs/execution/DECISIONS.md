# Workflow Alpha Architecture Decisions

## D-001 — Evolve the existing vertical loop through versioned boundaries

The current Agent Runtime remains a compatibility path while a published Workflow snapshot executor is introduced. It will not be renamed and presented as a DAG Runtime before it actually supports typed edges, branching, suspension, and replay.

Rejected: treating the existing linear `HybridWorkflowExecutor` as the finished DAG Runtime. It lacks most Milestone 3 semantics.

## D-002 — Typed Artifacts are the data plane

Specialist models, deterministic tools, refiners, and VLM nodes exchange immutable typed Artifact revisions. JSON is restricted to versioned API/backend boundaries. Runtime scheduling and commit policy operate on typed values and stable references.

Rejected: passing display summaries or asking a VLM to reconstruct coordinates.

## D-003 — Published Workflow snapshots are Run inputs

A Run must name and persist a complete immutable Workflow snapshot, including Skill, Model, prompt/resource, retry, fallback, and review policy versions. Draft IDs alone are insufficient for execution or history replay.

Rejected: resolving the latest mutable Project configuration during resume.

## D-004 — External vision models stay behind one versioned protocol

YOLO-, SAM-, and semantic-segmentation-class workers use the same health, capability, inference, error, usage, and timing contract. Rust owns orchestration, typing, budgets, cancellation, persistence, validation, and review.

Rejected: hard-coding model product names in Core enums or embedding all inference stacks in Rust.

## D-005 — Offline release evidence is mandatory; live evidence is conditional

Every release gate has an offline fixture or deterministic test where feasible. Real Qwen and local-weight inference are additional live-conditional checks and must never be reported as passing without an actual configured run.

## D-006 — Execution documentation is part of the implementation

The files in `docs/execution/` are updated at each vertical milestone and are the recovery source after context compaction. They record evidence and decisions, not aspirational completion claims.

## D-007 — Suspended review and completed-with-review are different states

`AwaitingReview` is a non-terminal Run suspension that remains active and can later resume. `CompletedWithReview` is terminal history indicating that image processing finished with review items. A one-time SQLite migration converts legacy `awaiting_review` terminal rows to `completed_with_review` and removes stale active reservations; subsequent real suspensions retain the new state.

## D-008 — History import preserves protocol identity as a graph

When a colliding Run import receives new IDs, all typed references are remapped together: assistant/tool messages, tool-call rows/events, Artifacts and lineage, annotation/revision relation endpoints, and TaskRun ownership. Remapping only the table primary keys would create an invalid replay and is rejected as incomplete history handling.

## D-009 — Compatibility Runs identify themselves honestly

Until Milestone 3 executes a published DAG, the existing Agent Runtime persists a complete `legacy_agent_runtime` snapshot containing the actual Skill manifest, task graph, Project schema, and model binding. It does not attach the latest published Workflow merely because one exists; that would make history claim execution semantics that did not occur.

## D-010 — Multi-Skill collisions are namespaced and visual precedence is explicit

For a multi-Skill Project, task/node, tool, Validator, and Refiner extension IDs are projected as `<skill>.<extension>`. Visual profiles merge in sorted Skill-ID order; the first owner wins and every ignored collision records both Skill IDs. Single-Skill legacy IDs remain readable for Schema v1 compatibility.

## D-011 — Suspension state is a complete replayable checkpoint

A DAG checkpoint owns node states, outputs, selected routes, activated fallbacks, review approvals, usage, and structured input/output traces and is bound to the published content hash. Resume rejects another Workflow hash and schedules only unfinished nodes. This serializable contract is the persistence boundary that the Dataset Coordinator will make durable in Milestone 5.

## D-012 — Commit and cache semantics are Runtime invariants

ImageInput, HumanReview, CandidateMerge, and Commit are built-ins; registering an operation with the same string cannot replace their safety behavior. Commit accepts only Valid Artifacts. Cache hits reuse immutable deterministic output, retain provenance, and add zero tokens/cost to the resumed execution.

## D-013 — Worker capability claims and real inference are distinct

The v1 HTTP contract is shared by detector-, prompted-segmentation-, and semantic-segmentation-class workers, but a worker may claim only capabilities backed by its configured model. The reference process reports degraded fixture health and `weights_unavailable` without weights; it reports a real model identity only after successfully loading a local model.

Rejected: returning plausible fixture geometry while labelling it as YOLO, SAM, PIDNet, or real local inference.

## D-014 — Registry secrets are references and health is observable state

Persisted model descriptors may contain only `env:` or `keychain:` secret references. Runtime adapters can hold transient credentials, but structured redaction and sanitized errors prevent those values from entering product traces. Health is a typed status with detail and check time and is exposed through the application/server DTO to the Models page.

Rejected: persisting provider keys in model configuration or inferring `healthy` merely because an external endpoint was configured.
