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
