# AnnotAgent

*Composable annotation workflows for vision data.*

AnnotAgent turns model proposals into typed, auditable annotations. A vision model proposes geometry, registered tools gather bounded image evidence, deterministic validators and refiners check it, and a review policy commits, retries, or sends the result to a human. Model calls, tool calls, revisions, validation issues, tokens, cost, and state transitions are persisted.

## Guided Project Workspace

The Web product is organized around one concrete Project journey:

`Data -> Labels -> Pipeline -> Test & Publish -> Run -> Inspect -> Review -> Export`

Start it from the repository root:

```bash
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

In the browser:

1. Open **Projects**, create or choose a Project, then use **Build**.
2. In **Data**, add workspace-local images. In **Labels**, define annotation semantics such as classification or bounding box labels.
3. In **Pipeline**, create a Draft, use the controlled Advisor or Node Catalog, configure model bindings and Core nodes, then save.
4. In **Test & Publish**, Dry Run 1–10 images. A valid report can be published as an immutable Workflow Version.
5. Start a single-image Run or Dataset Batch from the Project. Active work is restored from backend state and duplicate Start is locked.
6. Open **Runs** to inspect the exact image, node timeline, inputs, outputs, configuration, usage, errors, bbox/crop lineage, and sandbox Replay. Image, node, and Artifact context is preserved in the URL.
7. Open **Review** to edit and accept or reject queued annotations. Source Run links are bidirectional.
8. Return to the Project **Export** section and choose a format. The report shows the written files, exported count, skips, and warnings.

Provider settings live under **Settings -> Provider & budgets**. Non-secret settings persist in the workspace; secrets use the operating-system credential store. The offline Mock provider is suitable for product evaluation without a key.

Acceptance screenshots are in [`docs/execution/screenshots`](docs/execution/screenshots), and detailed milestone evidence is in [`docs/execution/UX_ACCEPTANCE_EVIDENCE.md`](docs/execution/UX_ACCEPTANCE_EVIDENCE.md).

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

A Project is one concrete annotation effort. It owns a Dataset and Annotation Schema and selects zero or more Skills, immutable Workflow versions, model bindings, review policy, Runs, imports, and exports. Generic Projects require no RoboCup Skill; multi-Skill extension IDs are namespaced and visual precedence is deterministic.

## 3. Workflow

A Workflow is a typed graph of model, tool, validator/refiner, review, and output steps. The Web Workflow page supports registry-bound suggestions, persisted Draft editing, static validation, selected-image Dry Run, and immutable publication. An exact Published Version can be selected for an image Run or Dataset Batch; the product executes that DAG, persists typed Artifacts and node trace, and stores its restart checkpoint. Legacy single-Skill Projects retain an explicitly labelled compatibility path when no version is selected.

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
cargo run -p annotagent -- demo generic-workflow
cargo run -p annotagent -- demo robocup-hybrid
```

The Generic demo has no RoboCup dependency. The RoboCup hybrid demo executes detector candidates, VLM semantic evidence, a real Skill Validator, Review Gate, and blocked Commit entirely offline.

The Runtime extension test also registers an independent `DummySkill` without changing Runtime:

```bash
cargo test -p annotagent-runtime --test skill_extension
```

RoboCup exposes three Workflow starters in the Workflow Designer: `vlm-bootstrap`, `detector-first`, and `accurate-hybrid`. The latter keeps specialist geometry as typed Artifacts and uses the VLM only for verification and attributes.

Run the ground-truth-backed synthetic evaluation (no key or external weights required):

```bash
cargo run -p annotagent -- evaluate \
  --ground-truth examples/robocup/evaluation/ground-truth.synthetic.json \
  --predictions examples/robocup/evaluation/predictions.synthetic.json \
  --bbox-iou-threshold 0.5 \
  --minimum-field-region-iou 0.7
```

Unlabelled real datasets are rejected as accuracy inputs; their run telemetry remains available separately.

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

See [Product hierarchy](docs/PRODUCT_HIERARCHY.md), [Workflow model](docs/WORKFLOW_MODEL.md), [Workflow runtime](docs/WORKFLOW_RUNTIME.md), [Artifact model](docs/ARTIFACT_MODEL.md), [Batch coordinator](docs/BATCH_COORDINATOR.md), [Model backend protocol](docs/MODEL_BACKEND_PROTOCOL.md), [Advisor](docs/WORKFLOW_ADVISOR.md), [Release acceptance](docs/RELEASE_ACCEPTANCE.md), and [Known limitations](docs/KNOWN_LIMITATIONS.md).

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
npm --prefix web run typecheck
npm --prefix web test -- --run
npm --prefix web run build
npm --prefix web run test:e2e
cargo run -p annotagent -- doctor
cargo run -p annotagent -- demo generic-workflow
cargo run -p annotagent -- demo robocup-hybrid
```

Security assumptions and disclosure guidance are in [SECURITY.md](SECURITY.md). The local server is designed for a trusted loopback workspace and has no authentication.
