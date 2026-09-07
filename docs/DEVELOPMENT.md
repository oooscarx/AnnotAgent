# Development and architecture

This is the technical companion to the [product overview](../README.md). Run the commands below from the repository root. For everyday use, start with the README and [Guided Experience](GUIDED_EXPERIENCE.md).

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

A Workflow is a typed graph of model, tool, validator/refiner, review, and output steps. The Web Workflow page supports registry-bound suggestions, persisted Draft editing, static validation, selected-image Dry Run, and immutable publication. An exact Published Version can be selected for an image Run or Dataset Batch; the product executes that DAG, persists typed Artifacts and node trace, and stores its restart checkpoint. Formal Runs fail closed until an exact Published Workflow Version is selected.

## 4. Model

Model bindings connect Workflow nodes to reusable Provider and Model Profiles. Settings offers
Provider presets, capability-aware Model Profiles and installable Rust Expert Model Plugins.
The default GUI credential path is an owner-only file under the Git-ignored workspace so a saved
Provider survives restarts without using the OS Keychain. Environment-variable, session-only and
native system credential references remain available. Worker health and capabilities are
discovered live, and an unavailable Provider or Worker never blocks AnnotAgent startup.

Native expert models are installed through **Settings → Expert Model Plugins** as deterministic
`.annotplugin` code packages plus independent, data-only `.annotmodel` Bundles. AnnotAgent reviews
permissions and exact licenses, verifies Catalog-pinned checksums, binds generic model-file roles,
starts an isolated Rust process and requires ONNX Contract plus fixed smoke evidence before a
publishable model becomes Ready. Existing manually configured HTTP Vision v1 endpoints remain under
**Legacy HTTP** for historical compatibility, but new Workflows should use native plugins. See
[Rust Model Plugins](RUST_MODEL_PLUGINS.md), [Writing a Rust Plugin](WRITING_A_RUST_MODEL_PLUGIN.md)
and [Model Bundles](MODEL_BUNDLES.md). The repository ships no third-party weights in Git; the
built-in prompted-segmentation Fixture is non-publishable protocol evidence, not SAM or accuracy
evidence. The separately generated real EfficientSAM-Ti Bundle and its exact installation and
release identities are documented in [Real Model Release](REAL_MODEL_RELEASE.md).

## 5. Skill

A Skill contributes domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It does not own a Dataset or the application shell. Rust implementations are registered through `DomainSkill`; the generic canvas consumes stable `annotation-1` through `annotation-8` slots through a `SkillVisualProfile`.

The public Capability layer is deliberately small: `annotagent.classification`,
`annotagent.detection`, and `annotagent.segmentation`. Detection covers closed-set detection,
open-vocabulary detection, phrase grounding, and VLM grounding while each concrete implementation
is a Model Backend. Classification covers whole images, Crops, candidate verification, and
attributes. Segmentation declares semantic, prompted, and instance-mask contracts and remains
unavailable until a healthy compatible backend is configured.

OpenAI-compatible VLM, YOLO, RF-DETR, LocateAnything, PIDNet and SAM are Model Backends rather than
top-level Skills. Native backends live in Settings → Expert Model Plugins and remain unavailable
until exact package, checkpoint, contract and smoke evidence is complete. Pre-Lean Skill IDs remain hidden compatibility aliases so
stored Projects and immutable versions can still be loaded. See
[Open-vocabulary Detection](OPEN_VOCABULARY_DETECTION.md),
[Object Detection](OBJECT_DETECTION.md), [LocateAnything Rust Plugin](LOCATE_ANYTHING_RUST_PLUGIN.md),
and [RF-DETR Rust Plugin](RFDETR_RUST_PLUGIN.md).

Detector outputs can be joined with the generic `core.match_detection_sets` node and routed by
`core.evidence_gate`. The persisted decision report explains agreement, conflicts, missing scores,
domain issues and fallback requests without blending incomparable confidence. See
[Detection Evidence](DETECTION_EVIDENCE.md),
[Specialist Detection](SPECIALIST_DETECTION.md), and
[Hybrid Detection Workflows](HYBRID_DETECTION_WORKFLOWS.md).

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
cargo run -p annotagent -- demo lean-agent-robocup
```

The Generic demos have no RoboCup dependency. The Ball demo covers the clean fast path, white-shoe
rejection, penalty-mark review and a Correction Memory decision change entirely offline. The Lean
Agent demo runs an audited invalid-Draft repair and two three-image sandbox Dry Runs, adds Crop
Classification from measured Review evidence, and stops for human approval without publishing.

The Runtime extension test also registers an independent `DummySkill` without changing Runtime:

```bash
cargo test -p annotagent-runtime --test skill_extension
```

RoboCup exposes one default Ball starter: `robocup.ball.vlm-bootstrap`. It binds one ready Detection
backend, selects football candidates, applies Domain Validators, and routes through Decision to
Commit or Human Review. The explicit specialist/fallback template remains a compatibility and
advanced-deployment option, not a default recommendation.

SAM, RF-DETR, LocateAnything, PIDNet and YOLO remain Model Backends until their separate Rust plugin,
weights, health and capabilities are Ready. They are not RoboCup Skill actions and are never
injected into the default Draft. See the [Rust Plugin Alpha demo](DEMO_RUST_PLUGIN_ALPHA.md)
or the [five-minute Lean Agent demo](DEMO_LEAN_AGENT_ALPHA.md).

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

See [Product hierarchy](PRODUCT_HIERARCHY.md), [Project Guidance](PROJECT_GUIDANCE.md), [Workflow model](WORKFLOW_MODEL.md), [Workflow runtime](WORKFLOW_RUNTIME.md), [Artifact model](ARTIFACT_MODEL.md), [Batch coordinator](BATCH_COORDINATOR.md), [Model backend protocol](MODEL_BACKEND_PROTOCOL.md), [Real Model Release](REAL_MODEL_RELEASE.md), [Open-vocabulary Detection](OPEN_VOCABULARY_DETECTION.md), [Specialist Detection](SPECIALIST_DETECTION.md), [RF-DETR Backend](RFDETR_BACKEND.md), [Detection Evidence](DETECTION_EVIDENCE.md), [Model License Metadata](MODEL_LICENSE_METADATA.md), [Hybrid Detection Workflows](HYBRID_DETECTION_WORKFLOWS.md), [five-minute Lean Agent demo](DEMO_LEAN_AGENT_ALPHA.md), [Advisor](WORKFLOW_ADVISOR.md), [Release acceptance](RELEASE_ACCEPTANCE.md), and [Known limitations](KNOWN_LIMITATIONS.md).

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
cargo run -p annotagent -- demo lean-agent-robocup
```

Security assumptions and disclosure guidance are in [SECURITY.md](SECURITY.md). The local server is designed for a trusted loopback workspace and has no authentication.

## Geometry-safe detection and improvement

AnnotAgent separates a model's semantic/detection score from measured box quality. VLM boxes are
uncalibrated coarse proposals by default, so a high semantic score cannot directly authorize a
training bbox Commit. Safe Pipelines use Human Review, measured prompted refinement, or exact
Project/model/node calibration.

Run Results and Review show score meaning, box source and geometry verification separately. From
Project Overview, Run Results, Review or Automation, open **Improve Automation** to diagnose reviewed
evidence, create a focused Patch Draft, compare it on independent holdout Runs, and apply selected
changes. AnnotAgent never publishes the result automatically.

Start with [VLM Geometry Safety](VLM_GEOMETRY_SAFETY.md),
[Geometry Calibration](GEOMETRY_CALIBRATION.md), and
[Pipeline Self-Improvement](PIPELINE_SELF_IMPROVEMENT.md).
