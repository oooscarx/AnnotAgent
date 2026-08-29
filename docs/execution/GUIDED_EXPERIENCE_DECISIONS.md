# Guided Experience Alpha Decisions

Updated: 2026-08-30

## GE-001 — Guidance is an Application projection, not a new persisted lifecycle

`ProjectStage` and `ProjectGuidance` are derived deterministically from persisted Project, Dataset, Workflow, Model, Run, Review, and Export state. They are not a second mutable status column. This prevents drift and makes refresh/restart recovery automatic.

## GE-002 — The server chooses one primary action

The Guidance Engine returns exactly one `primary_action` plus optional secondary/repair actions. React and TUI consume the same action DTO. The client may choose layout but may not reorder business priority.

## GE-003 — Guided and Expert modes share one definition

Automation Recipe, label lanes, and Expert Graph are projections/editors over the same `WorkflowDraft`. No conversion copy or separately persisted guided graph is allowed.

## GE-004 — Journey completion needs persisted evidence

Sample testing and activation cannot be inferred from a valid Draft alone. Guidance will use actual Dry Run and published-version evidence available from storage. A successful Dry Run remains a sandbox action and never writes formal Annotations.

## GE-005 — Global pages are not silently scoped

Global Runs and Review default to all Projects. Project context may be an explicit filter or deep link, but active Project local state cannot silently hide global records.

## GE-006 — Results precede execution internals

Dry Run and Run Detail first answer what was annotated, what was missed, what needs review, time, and cost. Node state, payloads, IDs, and trace remain available in Debug/Inspector and remain URL-addressable.

## GE-007 — Existing working capabilities are evolved in place

The typed DAG, immutable versions, Artifact checkpoint, Replay, geometry editor, provider presets, and export protocol remain authoritative. Guided Experience wraps these capabilities in task-oriented APIs and presentations rather than creating disabled or simulated substitutes.

## GE-008 — Offline evidence is the release baseline

Mock and deterministic backends must complete the full journey. Live VLM/SAM/YOLO configuration is conditional evidence and is never required to prove product-state correctness.

## GE-009 — API keys stay outside product DTOs and repository history

Provider settings may expose only whether a workspace-local credential exists. The API never returns the secret; logs, Guidance, Run summaries, evidence documents, and commits contain no credential material.

