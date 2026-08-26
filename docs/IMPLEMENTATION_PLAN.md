# Implementation Plan

## Repository baseline

The repository started as an empty directory on 2026-08-26. There was no existing source code or Git history to preserve. The implementation follows the course recommendation: land a working vertical slice first, then deepen the RoboCup-specific behavior and interfaces.

## Delivered stages

1. Defined the Rust workspace, checked geometry and annotation models, project schema, extension traits, registries, and a `DummySkill` integration test.
2. Ran the first vertical model/tool/validation/commit loop with deterministic Mock, persisted events, SQLite and usage.
3. Added the RoboCup DAG, field containment, field-line pixel refinement, ball hard negatives, team-color evidence, correction-memory influence, and two vertical integration tests.
4. Added durable history/revisions, budgets, state control, history round-trip and five explicit dataset export formats.
5. Added the CLI and Ratatui interface over a shared application service.
6. Added the Axum API, live SSE, durable workspace settings, system-keychain-backed provider keys, and React/TypeScript review GUI.
7. Added bounded multi-image CLI coordination, an offline two-case demo, course documentation, HTTP vertical tests, real-provider smoke evidence, and full acceptance checks.

## Architectural decisions

- Internal geometry uses checked normalized coordinates; invalid values cannot be constructed through the public API.
- `annotagent-core` contains only domain-neutral types and contracts. The binary composition root registers `annotagent-skill-robocup`; Core never branches on a skill ID or RoboCup label.
- SQLite stores structured history and relative image paths, never image blobs or API keys.
- The runtime treats provider output as untrusted, validates tool names and arguments, and enforces stopping and budget rules itself.
- The first release uses compile-time Rust skill registration plus YAML resources. Dynamic libraries, WASM plugins, and a package marketplace are deliberately outside scope.

## Assumptions

- The remote URL is configured exactly as requested (`git@github.com/AnnotAgent.git`). Git accepts it as a remote string; GitHub may still require an owner-qualified path before pushing.
- The mock provider is the default for tests and classroom demos so no API key, GPU, or external service is needed.
- A workspace directory is a trust boundary. External images are copied into it through controlled import rather than served from arbitrary paths.
