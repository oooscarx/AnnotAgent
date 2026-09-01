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

## Milestone 4 detection compatibility

Core now checks Node capability, input modality, and protocol requirements together. A structured
VLM Detection node accepts an available profile only when it declares `vision_language`, image
input, and either structured output or Tool Calls. The same profile remains incompatible with the
native detector nodes unless it separately declares `object_detection`.

The public Node Catalog now exposes the already executable VLM, specialist, and open-vocabulary
detection operations as distinct contracts. The Qwen-style fixture uses only `vision_language` and
`image_classification`, binds the structured VLM node, validates and Dry Runs the resulting RoboCup
ball Draft in seven Builder Tool Calls, and proves it cannot bind the native detector contract.
Unavailable prompted-segmentation expert models are not added to that runnable Draft.

## Milestone 5 prompt, API, UI, and retry

The live Builder prompt now mandates compact context first, deterministic feasibility second,
batch-only detail inspection, duplicate suppression, the Draft deadline, and protected finalization.
Every Provider turn receives a phase-specific `AvailableAgentActions` mask plus a compact
`BuilderContextDigest`; the full 59-tool catalog is no longer exposed on every turn. Once only the
finalization reserve remains, broad inspection tools disappear.

Persisted Agent Sessions and the HTTP API now expose phase, outcome, typed stop reason, model turns,
total/remaining/reserved Tool Calls, per-phase calls, cache hits, duplicate calls, Draft ID,
unresolved bindings, and the concrete next action. The web trace renders those values directly and
shows distinct outcome actions for review, Provider/Model setup, opening a blocked Draft, and retry.
Numeric HTML entities and trailing escape artifacts in error messages are normalized; reloading
saved state is no longer mislabeled as an Agent retry.

Retry creates a fresh Agent Session with reset budgets and duplicate/cache counters while retaining
the exact persisted editable Draft and unresolved requirements. A regression fixture retries the
four-call no-detector outcome in three calls and verifies that the Draft ID remains unchanged.

## Milestone 6 release validation

The complete product path now exercises Pipeline Builder through a deterministic local
OpenAI-compatible HTTP fixture rather than the removed product Mock Advisor. It covers Profile
creation and probing, Builder selection, phase-specific tool calls, editable Draft Diff/Undo,
Classification Dry Run, immutable publication, VLM Detection, Core Crop lineage, Run, Review,
Replay and recovery navigation. The fixture supplies protocol-shaped outputs only and is not used
as external-model accuracy evidence.

Release testing found and closed three integration gaps:

- legacy per-tool inspection now crosses `FeasibilityAnalysis → Drafting → Validating` before
  validating a created Draft, while the compact context path retains its stricter phase sequence;
- Registry Model Profile bindings are validated by Application and normalized to the compatible
  Runtime model identity only for execution snapshots, without weakening Profile availability,
  capability, modality or protocol checks;
- the VLM Detect + Crop template connects its CropSet preview to Commit, so static validation no
  longer reports a dangling node and the crop retains its detection parent reference.

The final gate passes `scripts/acceptance.sh`, strict all-target/all-feature Clippy, Rustfmt,
304 Rust tests plus doc tests, all-feature build, 40 Web tests, TypeScript, production Web build,
doctor, four offline demos and all 35 Chromium scenarios. One explicitly billable Provider smoke
remains ignored unless the operator opts in with a separately configured legal credential.

Offline Pipeline Builder Progress-Safety release status: `PASS`. External Provider behavior and
quality remain `LIVE-CONDITIONAL`; no conversation credential, push or remote mutation was used.
