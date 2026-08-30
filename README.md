# AnnotAgent

*Composable annotation workflows for vision data.*

AnnotAgent turns model proposals into typed, auditable annotations. A vision model proposes geometry, registered tools gather bounded image evidence, deterministic validators and refiners check it, and a review policy commits, retries, or sends the result to a human. Model calls, tool calls, revisions, validation issues, tokens, cost, and state transitions are persisted.

## Guided Project Workspace

The Web product is organized around one concrete Project journey:

`Create -> Data -> Labels -> Automation -> Test & Activate -> Run -> Review -> Export`

Start it from the repository root:

```bash
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

In the browser:

1. Open **Projects** and choose **New project**. The four-step wizard asks what to annotate, where the data is, which speed/accuracy priority matters, and which registered model connection to use. Internal IDs and generated YAML stay under Advanced.
2. In **Data**, add workspace-local images. In **Labels**, define annotation semantics such as classification or bounding box labels.
3. In **Automation**, preview a registry-bounded recommendation, apply it to the editable Draft, and adjust the readable Recipe or its node settings. The full typed graph remains in Expert mode.
4. In **Test & Activate**, run 1–10 real images in the sandbox. Inspect image outcomes, Crops, Review workload, duration, and cost; then activate the tested Draft as an immutable Workflow Version.
5. Return to the Project and choose the single server-recommended next action. Starting the Dataset creates real Runs; active work is restored from backend state and duplicate Start is locked.
6. Open a Run. **Results** shows annotations, confidence, linked Crops, and attention items; **Debug** reveals node inputs, outputs, configuration, usage, errors, and Replay. URL state preserves the exact Image, Node, and Artifact.
7. Use **Review** as a decision Inbox. Edit if needed, then Accept & next or Reject & next; source Run links are bidirectional and the final item leads to Export.
8. Open Project **Export**, resolve any readiness blocker, select a Schema-compatible format, and run the real exporter. The completion report and source fingerprint survive reload while the Project snapshot remains current.

Provider settings live under **Settings -> Provider & budgets**. Non-secret settings persist in the workspace; a GUI-entered key is write-only and stored in the workspace-private `.annotagent/credentials/provider-api-key` file, never in SQLite or a keychain. The offline Mock provider is the Release baseline and needs no key.

Start with [Guided Experience](docs/GUIDED_EXPERIENCE.md), [Project setup](docs/GUIDED_PROJECT_SETUP.md), [Run and Review UX](docs/RUN_AND_REVIEW_UX.md), or the [offline demo](docs/DEMO_GUIDED_EXPERIENCE.md). Acceptance screenshots are in [`docs/execution/screenshots`](docs/execution/screenshots), and the current Release Matrix is [`docs/execution/GUIDED_EXPERIENCE_ACCEPTANCE.md`](docs/execution/GUIDED_EXPERIENCE_ACCEPTANCE.md).

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

Model bindings connect Workflow nodes to configured providers and models. The Settings page offers a provider catalog for common vision providers plus optional local Detection Workers, persists non-secret configuration in the workspace, and stores the write-only key at `<workspace>/.annotagent/credentials/provider-api-key` with owner-only permissions. CLI environment-variable keys remain supported. Worker health and capabilities are discovered live; an unavailable Worker never blocks AnnotAgent startup.

## 5. Skill

A Skill contributes domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It does not own a Dataset or the application shell. Rust implementations are registered through `DomainSkill`; the generic canvas consumes stable `annotation-1` through `annotation-8` slots through a `SkillVisualProfile`.

The bundled `annotagent.open_vocabulary_grounding` Capability Skill finds objects from category
descriptions or referring phrases. Bind `mock-open-vocabulary` for an offline contract test or the
optional versioned LocateAnything Worker for local GPU inference. See [Open-vocabulary Detection](docs/OPEN_VOCABULARY_DETECTION.md)
and [LocateAnything Backend](docs/LOCATE_ANYTHING_BACKEND.md).

## 6. Review

Models select registered actions and may submit candidates or operate on stable Artifact references. Rust validation and review policy determine whether a candidate is committed, retried, completed empty, or queued. Human edits append revisions instead of overwriting history, and the trace exposes model/tool/Artifact events without hidden chain-of-thought.

## 7. Example Application: RoboCup Ball

The bundled `robocup` Pack and `robocup.ball` Domain Skill solve one annotation problem: football
bounding boxes. Robots, people, field geometry and penalty marks are visual context or hard
negatives; they are not output labels. Domain resources, checks, correction taxonomy and the ball
visual slot live outside Core.

The deterministic demo needs no GPU or API key:

```bash
cargo run -p annotagent -- demo generic-classification
cargo run -p annotagent -- demo generic-detection-crop
cargo run -p annotagent -- demo robocup-ball
```

The Generic demos have no RoboCup dependency. The Ball demo covers the clean fast path, white-shoe
rejection, penalty-mark review and a Correction Memory decision change entirely offline.

The Runtime extension test also registers an independent `DummySkill` without changing Runtime:

```bash
cargo test -p annotagent-runtime --test skill_extension
```

RoboCup exposes only two Ball starters: `robocup.ball.vlm-bootstrap` and
`robocup.ball.detector-first`. Both keep detector geometry as typed Artifacts and route only risky
ball candidates to Review.

For real SAM2.1 refinement, install the workspace-private worker once and start it before the GUI:

```bash
./scripts/setup-sam2.sh
./scripts/start-sam2-worker.sh
```

In a second terminal, start AnnotAgent normally. A RoboCup Ball Project configured with
`refiners: [sam_prompted_refiner]` runs VLM box proposal → local foreground prompt tightening →
SAM instance mask → tight bounding box. The mask and both boxes are persisted as separate Run
Artifacts. The tracked offline example keeps `ball_foreground_refiner`, so Mock acceptance never
depends on a model service.

Run the ground-truth-backed synthetic evaluation (no key or external weights required):

```bash
cargo run -p annotagent -- evaluate \
  --ground-truth examples/robocup/evaluation/ground-truth.synthetic.json \
  --predictions examples/robocup/evaluation/predictions.synthetic.json \
  --bbox-iou-threshold 0.5
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

See [Product hierarchy](docs/PRODUCT_HIERARCHY.md), [Project Guidance](docs/PROJECT_GUIDANCE.md), [Workflow model](docs/WORKFLOW_MODEL.md), [Workflow runtime](docs/WORKFLOW_RUNTIME.md), [Artifact model](docs/ARTIFACT_MODEL.md), [Batch coordinator](docs/BATCH_COORDINATOR.md), [Model backend protocol](docs/MODEL_BACKEND_PROTOCOL.md), [Open-vocabulary Detection](docs/OPEN_VOCABULARY_DETECTION.md), [Advisor](docs/WORKFLOW_ADVISOR.md), [Release acceptance](docs/RELEASE_ACCEPTANCE.md), and [Known limitations](docs/KNOWN_LIMITATIONS.md).

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
cargo run -p annotagent -- demo generic-classification
cargo run -p annotagent -- demo generic-detection-crop
cargo run -p annotagent -- demo robocup-ball
```

Security assumptions and disclosure guidance are in [SECURITY.md](SECURITY.md). The local server is designed for a trusted loopback workspace and has no authentication.
