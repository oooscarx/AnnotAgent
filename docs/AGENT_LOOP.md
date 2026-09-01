# Agent Loop

## Per-image execution

`AgentRuntime::run_image` validates the Skill workflow, opens a durable run, builds the tool registry, and walks the Skill DAG. For each configured task:

1. Publish `TaskStarted` and compile task-scoped context.
2. Check pause/cancel and budget before a model request.
3. Call the provider with a bounded image copy and only applicable tools.
4. Persist actual or mock usage and publish model/usage events.
5. Normalize repeated-call signatures and validate the tool name, task scope, cancellation state, and JSON Schema.
6. Execute the bounded tool and put its visible summary back into context.
7. Parse submitted candidates into checked annotation types.
8. Run configured refiners, append before/after revisions, then run deterministic validators.
9. Ask the Skill review policy to auto-accept, retry, request human review, or reject.
10. Persist annotations/issues/events and move to the next task or terminal state.

```text
model
  → registered tool / typed submission
  → optional deterministic refiner
  → deterministic validators
  → policy: retry | human review | reject | commit
```

Runtime, not the model, controls commit. “I validated it” in model text has no effect.

## Detection Recovery Agent

Published detection Workflows can use the Rust-owned `agent.detection_recovery` node. It consumes
an Image plus primary DetectionSet, evaluates typed Evidence Gate rules and can invoke only the
registered Open Vocabulary capability. Step, tool, fallback-call and exact-decimal cost budgets are
checked before invocation. A high specialist score takes the zero-call fast path; empty, low-score,
domain-risk or correction-risk evidence can request one fallback. Failure, cancellation, missing
binding or insufficient budget preserves primary evidence and stops at Human Review.

The persisted Agent Session contains structured reason codes, selected model/capability, counts,
timing, decision and stop condition. It does not store hidden chain-of-thought, query text, image
bytes or raw Worker responses.

## Evidence-driven expert model selection

The Pipeline Builder inspects model availability, capability, typed contracts, conversion paths,
failure class, geometry quality and bounded Dry Run comparison before changing a Draft. Provider
failure, no candidate and semantic/domain risk cannot trigger prompted segmentation. A coarse-box
geometry failure may add an Available promptable refiner, but the resulting Draft still requires
static validation, Dry Run and explicit human approval.

## Real-provider hardening

- The submit schema receives a dynamic enum of labels allowed by the current task.
- Prompt examples define normalized geometry and `[x,y,width,height]` rectangles.
- Malformed arguments and invalid typed candidates become structured feedback and bounded retries.
- A task may perform one non-terminal evidence/refinement call, after which only submit/finish/review tools are exposed.
- Three identical normalized tool calls stop the task.
- Failed dependencies prevent unsafe downstream execution; issues explain the skip.
- Provider failures, maximum steps/retries, budgets, cancellation, and unrepairable candidates are explicit stopping conditions.

## Control

Pause completes the current small operation and blocks at `wait_until_runnable`; resume signals the waiter. Cancel propagates a `CancellationToken` to the provider and tool context and transitions to `Cancelled`, preserving trace and committed work. Illegal terminal transitions return errors.

## Usage and budgets

Each completed model request creates a `UsageRecord` with provider/model, safe endpoint summary, timestamps/duration, token source, images/requests, request ID, retry count, cost breakdown, and success. Exact decimal pricing is user configuration. Budget checks occur before starting another call.

## Events and history

Events have schema version, event ID, run/image/task IDs, UTC timestamp, typed kind and typed payload. SQLite persistence occurs before broadcast to the Runtime bus. `LocalApplication` relays the same events to the TUI and SSE. Hidden chain-of-thought is neither requested nor stored.

## Geometry-aware planning and improvement

For bbox objectives, feasibility must inspect the operation-scoped Model quality contract, Project
geometry policy, correction summary, exact calibration and available typed refinement paths. A VLM
score is semantic evidence, not localization quality. The Agent may create and validate a safe
Draft, run bounded Dry Runs and compare a persisted improvement session, but it cannot publish or
start a full dataset Run. Provider failures, no-candidate results and semantic errors are classified
before considering segmentation; SAM is only a candidate repair for geometry errors with a valid
prompt and healthy compatible backend.
