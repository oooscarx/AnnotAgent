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
