# AnnotAgent

*Composable annotation workflows for vision data.*

AnnotAgent turns model proposals into typed, auditable annotations. A vision model proposes geometry, registered tools gather bounded image evidence, deterministic validators and refiners check it, and a review policy commits, retries, or sends the result to a human. Model calls, tool calls, revisions, validation issues, tokens, cost, and state transitions are persisted.

AnnotAgent can combine open-vocabulary models, specialist detectors, domain validators, and human
review into versioned annotation pipelines.

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

Provider and Model Profiles live under **Settings → Providers / Models**. Non-secret settings persist
in SQLite. The default GUI path uses an environment-variable reference or a process-only session
value, so a key is never written to the workspace, browser storage, SQLite, or macOS Keychain.
Native credential storage remains an explicit opt-in source in advanced Provider settings. An
existing `.annotagent/credentials/provider-api-key` remains readable only as an explicitly labelled
legacy source and is never copied or deleted automatically. The offline Mock Provider is the Release
baseline and needs no key.

Start with [Guided Experience](docs/GUIDED_EXPERIENCE.md), [Project setup](docs/GUIDED_PROJECT_SETUP.md), [Run and Review UX](docs/RUN_AND_REVIEW_UX.md), the [offline demo](docs/DEMO_GUIDED_EXPERIENCE.md), or the [Provider Builder demo](docs/DEMO_PROVIDER_BUILDER.md). Acceptance screenshots are in [`docs/execution/screenshots`](docs/execution/screenshots), and the current Release Matrix is [`docs/execution/GUIDED_EXPERIENCE_ACCEPTANCE.md`](docs/execution/GUIDED_EXPERIENCE_ACCEPTANCE.md).

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

Model bindings connect Workflow nodes to reusable Provider and Model Profiles. Settings offers
Provider presets, capability-aware Model Profiles and separate optional local Vision Workers.
Environment-variable and session-only references are the normal credential paths; native system
storage is opt-in. Worker health and capabilities are discovered live, and an unavailable Provider
or Worker never blocks AnnotAgent startup.

## 5. Skill

A Skill contributes domain nodes, validators, refiners, prompt resources, Workflow templates, correction taxonomy, and label visual mappings. It does not own a Dataset or the application shell. Rust implementations are registered through `DomainSkill`; the generic canvas consumes stable `annotation-1` through `annotation-8` slots through a `SkillVisualProfile`.

The public Capability layer is deliberately small: `annotagent.classification`,
`annotagent.detection`, and `annotagent.segmentation`. Detection covers closed-set detection,
open-vocabulary detection, phrase grounding, and VLM grounding while each concrete implementation
is a Model Backend. Classification covers whole images, Crops, candidate verification, and
attributes. Segmentation declares semantic, prompted, and instance-mask contracts and remains
unavailable until a healthy compatible backend is configured.

Mock, OpenAI-compatible VLM, YOLO, RF-DETR, LocateAnything, and SAM are Model Backends rather than
top-level Skills. The optional local Workers live in Settings → Models under Experimental / Labs
until explicitly configured and healthy. Pre-Lean Skill IDs remain hidden compatibility aliases so
stored Projects and immutable versions can still be loaded. See
[Open-vocabulary Detection](docs/OPEN_VOCABULARY_DETECTION.md),
[Object Detection](docs/OBJECT_DETECTION.md), [LocateAnything Backend](docs/LOCATE_ANYTHING_BACKEND.md),
and [RF-DETR Backend](docs/RFDETR_BACKEND.md).

Detector outputs can be joined with the generic `core.match_detection_sets` node and routed by
`core.evidence_gate`. The persisted decision report explains agreement, conflicts, missing scores,
domain issues and fallback requests without blending incomparable confidence. See
[Detection Evidence](docs/DETECTION_EVIDENCE.md),
[Specialist Detection](docs/SPECIALIST_DETECTION.md), and
[Hybrid Detection Workflows](docs/HYBRID_DETECTION_WORKFLOWS.md).

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

SAM, RF-DETR, LocateAnything and YOLO remain Model Backends in Labs until their separate Worker,
weights, health and capabilities are configured. They are not RoboCup Skill actions and are never
injected into the default Draft. See the [five-minute Lean Agent demo](docs/DEMO_LEAN_AGENT_ALPHA.md)
for the current course path.

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

See [Product hierarchy](docs/PRODUCT_HIERARCHY.md), [Project Guidance](docs/PROJECT_GUIDANCE.md), [Workflow model](docs/WORKFLOW_MODEL.md), [Workflow runtime](docs/WORKFLOW_RUNTIME.md), [Artifact model](docs/ARTIFACT_MODEL.md), [Batch coordinator](docs/BATCH_COORDINATOR.md), [Model backend protocol](docs/MODEL_BACKEND_PROTOCOL.md), [Open-vocabulary Detection](docs/OPEN_VOCABULARY_DETECTION.md), [Specialist Detection](docs/SPECIALIST_DETECTION.md), [RF-DETR Backend](docs/RFDETR_BACKEND.md), [Detection Evidence](docs/DETECTION_EVIDENCE.md), [Model License Metadata](docs/MODEL_LICENSE_METADATA.md), [Hybrid Detection Workflows](docs/HYBRID_DETECTION_WORKFLOWS.md), [five-minute Lean Agent demo](docs/DEMO_LEAN_AGENT_ALPHA.md), [Advisor](docs/WORKFLOW_ADVISOR.md), [Release acceptance](docs/RELEASE_ACCEPTANCE.md), and [Known limitations](docs/KNOWN_LIMITATIONS.md).

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
