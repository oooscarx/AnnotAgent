# Implementation Plan

## Repository baseline

The repository started as an empty directory on 2026-08-26. There was no existing source code or Git history to preserve. The implementation follows the course recommendation: land a working vertical slice first, then deepen the RoboCup-specific behavior and interfaces.

## Stages

1. Define the Rust workspace, checked geometry and annotation models, project schema, extension traits, registries, and a `DummySkill` integration test.
2. Run one image and one task through a real model/tool/validation/commit loop with a deterministic mock provider, persisted events, and usage.
3. Add the RoboCup task DAG, field containment, field-line pixel refinement, ball hard-negative detection, team-color evidence, and correction-memory influence.
4. Add durable history, revision records, budgets, run state transitions, pause/resume/cancel, and export formats.
5. Add the CLI and a compact Ratatui interface backed by the same application service.
6. Add the Axum API, SSE event stream, and React/TypeScript review GUI.
7. Generate an offline synthetic demo, update the design documentation, and execute every applicable acceptance command.

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

