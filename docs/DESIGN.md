# Design Decisions

## Product scope

The formal product is RoboCup annotation. Extensibility exists so the architecture stays honest, not to market a universal annotation platform. Consequently there is one production Skill and one synthetic RoboCup demo.

## Checked geometry

Coordinates use private fields and checked constructors. This prevents invalid normalized values from leaking into validators, exporters, and the GUI revision path. Geometry is converted to pixels only at image-tool or export boundaries.

## Model proposes; Rust decides

The model cannot commit directly. It submits typed candidates. Runtime parses them, deterministic Skill code refines and validates them, and a review policy selects accept, retry, review, or reject. The trace stores visible model content and structured actions, never hidden reasoning.

## Task-scoped tools and context

The Runtime exposes only generic tools plus Skill tools applicable to the current task. A task may spend one evidence/refinement tool call before it must submit, preventing large tool-call fan-out observed with real compatible providers. Prompt resources are loaded per task instead of injecting the full Skill corpus.

## Persistence

SQLite was chosen for local durability, transactions, easy classroom setup, and exportable audit history. Revision records append before/after snapshots rather than overwriting human changes. Money uses `rust_decimal::Decimal`.

## Frontends

CLI/TUI and HTTP use `LocalApplication`; neither duplicates the Agent Loop. React only renders DTOs and sends review/control requests. The server owns validation, state transitions, correction records, export, and settings validation.

## Assumptions

- A configured workspace is the local security boundary.
- Folder import is controlled copying, while arbitrary external reads are not exposed over HTTP.
- OpenAI-compatible Chat Completions is the production network protocol in this release.
- Mock mode is the authoritative offline demo and CI fallback.
