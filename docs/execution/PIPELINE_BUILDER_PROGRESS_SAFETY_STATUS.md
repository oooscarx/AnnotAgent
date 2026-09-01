# Pipeline Builder Progress-Safety Status

## Milestone 0 baseline

The regression fixture `pipeline_builder_baseline_reproduces_repeated_inspection_budget_exhaustion`
reproduces the production failure without making a network request:

- seven scripted Provider turns;
- 48 successful read-only Tool Calls;
- repeated `inspect_node_definition` and `inspect_model_profile` calls;
- 95,326 reported input tokens;
- no persisted Workflow Draft;
- terminal `BudgetExceeded` with `step or tool-call budget exhausted`.

This is intentionally the pre-fix assertion. Milestones 1–5 replace the generic exhaustion outcome
with phased progress enforcement, cached observations, deterministic feasibility, a runnable or
blocked Draft, and a concrete next action.

No credential, remote Provider request, formal Run, Publish operation, or repository remote change
is part of this fixture.

## Milestone 1 phased progress state

Core now persists a `PipelineBuilderPhase`, concrete `PipelineBuilderOutcome`, typed
`BuilderStopReason`, phased `PipelineBuilderBudget`, and `BuilderProgressInvariant` on every live
Builder Agent Session. Defaults cap discovery at 10 calls, preserve six total calls for
finalization, cap one Provider response to four tool requests, and define context, feasibility, and
Draft deadlines at calls 6, 10, and 12.

Phase transitions are centralized and reject regression back into catalog loading. The session API
serialization now carries phase counters, model turns, Draft identity, unresolved binding summaries,
cache/duplicate counters, and the next action while remaining backward compatible with historical
session JSON through Serde defaults.

## Milestone 2 compact context and observation reuse

The registered Builder tool surface now includes one revisioned `get_pipeline_builder_context`
snapshot plus bounded Node, Model, and contract batch inspection. The snapshot includes the Project
and target Label, enabled Skills, typed Node summaries, credential-safe available Model summaries,
existing Drafts, templates, a capability matrix, and explicit unavailable capabilities.

Read-only observations are keyed by tool name, canonical JSON arguments, context revision, and Draft
revision. The first duplicate returns an observation reference; later duplicates return
`repeated_inspection_blocked`. The production-loop regression now proves that its 48 attempted reads
perform only two full underlying resource reads, with two cache hits and 47 duplicate requests
recorded for diagnosis. Mutation, validation, and Dry Run operations are never cached.

## Milestone 3 deterministic feasibility and blocked Drafts

Rust now resolves `Runnable`, `RunnableWithDegradedQuality`, `BlockedByBindings`, or `Unsupported`
from the compact context instead of asking the Builder model to enumerate the catalog. A typed
`UnresolvedModelRequirement` carries capabilities, modalities, protocol features, reason, compatible
profiles, and concrete setup actions on the affected Draft node.

The Runtime enforces a six-call recovery deadline for blocked/unsupported requests and a ten-call
deadline for runnable requests. Recovery itself records feasibility and Draft creation while using
the reserved finalization budget. The original inspection-loop fixture now stops after two model
turns and eight total Tool Calls, persists one blocked Draft, and reports `ProviderSetupRequired`
instead of generic exhaustion. Its scripted input usage falls from 95,326 to 27,236 tokens (71.4%).

A separate no-detection-model fixture completes the intended context → feasibility → blocked Draft
→ setup-requirements path in four Tool Calls. Static validation emits the blocking
`unresolved_model_binding` issue; Dry Run and Publish remain unavailable until the binding is fixed.
