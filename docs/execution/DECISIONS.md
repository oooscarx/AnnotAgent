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

## D-015 — Batch and child Run identities remain separate

A Dataset batch owns queue order, global budget, worker lease, progress events, and recovery checkpoint under a `BatchId`. Every image execution retains its own `RunId` and full audit history. A completed Batch image is never reclaimed; a failed image returns to Pending only through an explicit retry transition.

Rejected: treating a list of unrelated process-local image Runs as a durable batch or using one Run ID for all image histories.

## D-016 — Budget reservation precedes concurrent work

The Batch ledger distinguishes exact consumed and reserved usage. Claiming an image, checking the combined ledger against token/request/image/cost/deadline limits, and recording its reservation occur in one SQLite transaction. Completion releases the reservation and adds actual usage atomically. Worker ownership is a renewable lease; a new server owner recovers orphaned leases and requeues unfinished work.

Rejected: checking a shared counter before a request without reserving capacity, which lets concurrent workers oversell the same remaining budget.

## D-017 — Advisor creativity is limited to registered choices

The offline Advisor creates a deterministic registry-bound Draft. The optional workspace-LLM
Advisor receives the same bounded catalog and one strict submission action; it may rename the
Draft and adjust only registered model bindings and review gates on a safe base graph. Static
validation remains authoritative and every suggestion stays a Draft until an operator publishes it.

Rejected: allowing an Advisor to emit arbitrary code, Shell commands, URLs, unknown tools, or an
already-published/running Workflow.

## D-018 — Run selection and execution attribution are recorded separately

An image Run may select an immutable Workflow Version and history presents that exact name and
version. Until the product image path is wired to the generic DAG executor, the same snapshot also
records `legacy_agent_runtime`, the actual Skill graph/model binding, and an explicit compatibility
note. Selection is auditable without falsely claiming that the generic graph controlled execution.

Rejected: replacing the actual compatibility snapshot with the selected Draft merely to make the
UI appear fully integrated.

## D-019 — Domain templates are Skill-owned and evaluation requires labelled truth

Core defines only the typed `WorkflowTemplate` contract. Enabled Skills contribute concrete
templates, and the application refuses a template ID that does not belong to the Project's enabled
Skills. RoboCup specialist templates preserve model geometry as Artifacts; VLM nodes emit semantic
verification or attributes and cannot silently replace detector coordinates.

Accuracy evaluation accepts a separate schema-v1 ground-truth document only when it explicitly
declares `labeled: true`. Operational telemetry can describe unlabeled data, but accuracy values and
quality-gate claims cannot be inferred from it.

Rejected: global RoboCup template IDs in Core, arbitrary template selection across Projects, or
plausible-looking accuracy reports derived from predictions without human-labelled truth.

## D-020 — Annotation import is report-first and Review-bound

Every importer returns valid annotations, revisions where the source can express them, per-record
issues, and format-level compatibility warnings before persistence. Dry-run uses the same parser and
mapping path without writes. Product import then maps images to persisted child Runs and commits only
valid records into `NeedsReview`; it never assigns an unrelated Run merely to make an import succeed.
Native keeps provenance and revision chains, while lossy interchange formats say exactly what they
cannot represent.

Rejected: aborting a whole dataset on one malformed record, silently dropping unsupported fields,
or importing annotations directly as accepted ground truth.
