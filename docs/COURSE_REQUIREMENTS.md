# Course Requirements Matrix

This is the initial traceability matrix. File links and exact verification commands are updated as implementation stages land.

## R1 — Rust core logic

Planned evidence: domain-neutral data model and contracts in `annotagent-core`; orchestration and state transitions in `annotagent-runtime`; deterministic RoboCup algorithms in `annotagent-skill-robocup`; SQLite repositories and Axum application service in Rust.

## R2 — User interface

Planned evidence: a Ratatui TUI and a React/Vite Web GUI, both invoking the same Rust application service and displaying results and trace events.

## R3 — Configurable model

Planned evidence: configurable endpoint, API-key environment variable, model, context/output limits, reasoning mode, timeout, capability flags, and pricing. Secrets are never persisted.

## R4 — Live progress and interruption

Planned evidence: versioned event bus shared by TUI and SSE, plus centrally enforced pause, resume, and cancellation.

## R5 — Context and history

Planned evidence: SQLite run/event/tool/model-call history, annotation revisions, task-focused context construction, and versioned history import/export.

## R6 — Usage and pricing

Planned evidence: per-call input/output tokens and additional usage, exact decimal cost, aggregate budgets, automatic stop, and TUI/GUI presentation.

