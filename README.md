# AnnotAgent

*Composable annotation workflows for vision data.*

AnnotAgent turns model proposals into typed, auditable annotations. A vision model proposes geometry, registered tools gather bounded image evidence, deterministic validators and refiners check it, and a review policy commits, retries, or sends the result to a human. Model calls, tool calls, revisions, validation issues, tokens, cost, and state transitions are persisted.

## 1. AnnotAgent Core

Core owns domain-neutral task types, checked geometry, the model/tool/validation loop, budgets, events, registries, persistence contracts, and frontend application use cases. It does not contain domain labels. CLI, TUI, and HTTP all call the same `LocalApplication` service.

```text
React Web GUI ─┐
Ratatui TUI ───┼─> Application Service ─> Runtime ─> Review/Commit
CLI ───────────┘             │                │
                             ├─ Project       ├─ Model
                             ├─ Workflow      └─ registered nodes
                             └─ SQLite history
```

## 2. Project

A Project is one concrete annotation effort. It owns a Dataset and Annotation Schema and selects Skills, Workflow versions, model bindings, review policy, and exports. The current schema supports one configured Skill and one compatibility Workflow derived from its task graph; the product DTO is intentionally broader so Project, Skill, and Workflow are no longer conflated.

## 3. Workflow

A Workflow is a typed graph of model, tool, validator/refiner, review, and output steps. The Web Workflow page supports registry-bound suggestions, persisted Draft editing, static Dry Run validation, and immutable published snapshots. The generic hybrid executor can run registered model backends, validators, review gates, and commits; the existing Project task graph remains the default production Run definition until a published Draft is explicitly selected in a future version.

## 4. Model

Model bindings connect Workflow nodes to configured providers and models. The Settings page offers a provider catalog for common vision providers, persists non-secret configuration in the workspace, and stores keys in the operating-system keychain. CLI environment-variable keys remain supported.

## 5. Skill

A Skill contributes domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It does not own a Dataset or the application shell. Rust implementations are registered through `DomainSkill`; the generic canvas consumes stable `annotation-1` through `annotation-8` slots through a `SkillVisualProfile`.

## 6. Review

Models select registered actions and may submit candidates or operate on stable Artifact references. Rust validation and review policy determine whether a candidate is committed, retried, completed empty, or queued. Human edits append revisions instead of overwriting history, and the trace exposes model/tool/Artifact events without hidden chain-of-thought.

## 7. Example Application: RoboCup Perception

The bundled `robocup` Skill and `examples/robocup/project.yaml` demonstrate the extension boundary on robot-soccer perception. Domain labels, prompt resources, hard-negative checks, pixel refiners, correction taxonomy, badge, and label colors live in the Skill or example—not in Core or the global product shell.

The deterministic demo needs no GPU or API key:

```bash
cargo run -p annotagent -- demo robocup
```

The Runtime extension test also registers an independent `DummySkill` without changing Runtime:

```bash
cargo test -p annotagent-runtime --test skill_extension
```

## Install and start

Requirements are stable Rust, Node.js 20+, and npm.

```bash
cargo build --workspace --all-features
npm --prefix web install
npm --prefix web run build
```

Start the product shell with an empty workspace:

```bash
cargo run -p annotagent -- serve --workspace ./workspace --open
```

Open the TUI with or without an initial Project:

```bash
cargo run -p annotagent -- tui
cargo run -p annotagent -- tui --project examples/robocup/project.yaml
```

Create and run a Project:

```bash
cargo run -p annotagent -- init workspace/my-project --skill robocup
cargo run -p annotagent -- run \
  --project workspace/my-project/project.yaml \
  --provider mock \
  --limit 1
```

For a real compatible provider, copy an example configuration, enter the provider and model in Settings or set the configured environment variable, then select that saved binding for the run. Never commit local keys.

## Repository guide

- `crates/annotagent-core`: domain-neutral contracts and checked types.
- `crates/annotagent-runtime`: bounded agent loop and Workflow execution compatibility layer.
- `crates/annotagent-application`: Project/Workflow/Model DTOs and use cases.
- `crates/annotagent-server`: local HTTP/SSE boundary.
- `web`: product shell and review interface.
- `skills/<id>` and `crates/annotagent-skill-*`: Skill resources and implementations.
- `examples`: concrete Project examples.
- `design/annotagent-visual-system`: canonical Core and Skill visual sources.

See [Product hierarchy](docs/PRODUCT_HIERARCHY.md), [Design](docs/DESIGN.md), [Core and Skills](docs/CORE_AND_SKILLS.md), [Hybrid vision execution](docs/HYBRID_VISION.md), [Vision worker protocol](docs/VISION_WORKER_PROTOCOL.md), [API](docs/API.md), and [Known limitations](docs/KNOWN_LIMITATIONS.md).

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
npm --prefix web run typecheck
npm --prefix web test -- --run
npm --prefix web run build
```

Security assumptions and disclosure guidance are in [SECURITY.md](SECURITY.md). The local server is designed for a trusted loopback workspace and has no authentication.
