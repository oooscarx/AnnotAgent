# RoboCup AnnotAgent

<img src="design/annotagent-visual-system/brand/logo/svg/robocup-annotagent-lockup-light.svg" alt="RoboCup AnnotAgent — AnnotAgent Core with the RoboCup Skill" width="560" />

A VLM-powered annotation and quality-control agent for RoboCup robot perception datasets, built on the extensible AnnotAgent Core.

RoboCup AnnotAgent turns model proposals into auditable annotations. A vision model proposes typed geometry, registered tools gather bounded image evidence, Rust validators and refiners check it, and a review policy either commits it, retries, or queues it for a human. This is deliberately more than a one-shot “image to JSON” script: every model call, tool call, validation issue, revision, token count, cost, and state transition is persisted.

Generic VLM annotation fails in predictable ways on robot soccer data: white shoes resemble balls, penalty marks resemble small white objects, field lines cross foreground robots, and team color evidence is often local. The bundled `robocup` Skill encodes those failure modes in Rust as well as task-scoped prompt resources.

## Architecture

```text
React Web GUI ─┐
Ratatui TUI ───┼─> AnnotAgent Application Service
CLI ───────────┘        │
                        ├─> Dataset Coordinator
                        └─> Image Agent
                              model → tool → validator/refiner
                                      → retry/review/commit
                                  │
              AnnotAgent Core <──┼──> RoboCup Skill
                                  │
                          SQLite history + SSE
```

`annotagent-core` only understands tasks, labels, geometry, attributes, relations, tools, validators, refiners, workflows, events, and usage. RoboCup names and algorithms live in `annotagent-skill-robocup`. The integration test in `crates/annotagent-runtime/tests/skill_extension.rs` registers a `DummySkill` and runs it without modifying Runtime.

The same boundary governs presentation: AnnotAgent owns the mark, interface tokens, status language, and generic annotation slots; the RoboCup Skill adds its product lockup, Skill badge, vocabulary, and label-to-slot mapping. Canonical assets and tokens live in `design/annotagent-visual-system/`.

## Install

Requirements: stable Rust (the repository pins a stable toolchain), Node.js 20+ for the Web UI, and npm.

```bash
cargo build --workspace --all-features
npm --prefix web install
npm --prefix web run build
```

No GPU or API key is needed for the deterministic demo.

## Five-minute offline demo

```bash
cargo run -p annotagent -- demo robocup
```

The generated image is copyright-safe and contains a green field, white lines, red/blue robots, white shoes, a penalty mark, and a ball. The Mock Provider first proposes a white shoe as a ball; `BallHardNegativeValidator` requests a retry and the corrected proposal is committed. A deliberately offset field line is moved onto the white pixel response by `RoboCupFieldLineRefiner`. Mock token and exact-decimal cost records are written to `.annotagent/history.db`.

Equivalent commands:

```bash
cargo run -p annotagent -- project validate examples/robocup/project.yaml
cargo run -p annotagent -- run \
  --project examples/robocup/project.yaml \
  --provider mock \
  --limit 1
```

Without `--limit`, the CLI enumerates the dataset and applies the project’s `max_parallel_images` bound.

## Real VLM

Copy the example and set the key only in the configured environment variable:

```bash
cp config/qwen3.7-flash.example.toml config/local.toml
export ANNOTAGENT_API_KEY='replace-me'
cargo run -p annotagent -- run \
  --project examples/robocup/project.yaml \
  --provider openai_compatible \
  --config config/local.toml \
  --limit 1
```

Endpoint, model, default run provider, protocol, timeout, output limit, reasoning mode, capability flags, custom headers, extra request fields, pricing, and budgets are configurable. The Settings page persists non-secret values to `<workspace>/.annotagent/settings.toml`; an API key entered there is write-only and stored in the operating system keychain, never in SQLite, the settings file, API responses, or trace output. Environment-variable keys remain supported by the CLI and as a server fallback.

## TUI and Web GUI

```bash
cargo run -p annotagent -- tui --project examples/robocup/project.yaml
```

The TUI starts the same application service, streams model/tool/validation/usage events, and supports pause, resume, cancellation, history inspection, and opening the GUI.

Build the frontend, initialize a workspace project, and start the local server:

```bash
npm --prefix web run build
cargo run -p annotagent -- init workspace/robocup-demo --skill robocup
cargo run -p annotagent -- serve --workspace ./workspace --open
```

Open `http://127.0.0.1:8787`. The GUI contains Dashboard, Project, Review, Skills, and Settings pages. Its SVG overlay supports zoom/pan and editing bbox, keypoint, polyline, polygon, and polygon-mask geometry. Every saved edit and review decision creates a revision; review decisions can create project-level correction memory.

For a real provider, open Settings once, select `OpenAI-compatible`, enter the endpoint, model, and API key, then save. These values survive server restarts, and `Start image run` uses the saved default provider automatically. Use `Clear saved key` to remove the workspace credential from the system keychain.

## Data and exports

Internal coordinates are checked normalized values in `[0,1]`. Supported values are classification, bounding box, keypoints, polyline, polygon, instance mask, attributes, and relations. Exports are explicit about incompatible annotations:

```bash
cargo run -p annotagent -- export \
  --project examples/robocup/project.yaml \
  --format coco \
  --output ./exports/coco
```

Available exporters: AnnotAgent Native JSON, COCO, YOLO Detection, YOLO Segmentation, and LabelMe. Folder import hashes images and skips duplicates.

## Quality checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
npm --prefix web run typecheck
npm --prefix web test -- --run
npm --prefix web run build
```

## Documentation

- `docs/ARCHITECTURE.md` — crate and runtime relationships.
- `docs/CORE_AND_SKILLS.md` — extension boundary and `DummySkill` proof.
- `docs/AGENT_LOOP.md` — loop, stopping, control, and context rules.
- `docs/ANNOTATION_SCHEMA.md` — checked data model and revision semantics.
- `docs/ROBOCUP_SKILL.md` — domain algorithms and test evidence.
- `docs/API.md` — CLI and HTTP/SSE surface.
- `docs/COURSE_REQUIREMENTS.md` — R1–R6 evidence and verification commands.
- `docs/KNOWN_LIMITATIONS.md` — exact remaining gaps.
- `docs/VISUAL_SYSTEM_INTEGRATION.md` — visual-system sources, GUI/TUI entry points, boundaries, and verification.

The implementation follows the course [requirements](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/requirements/), [quick start](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/quick-start/), and [agent architecture](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/agent-architecture/) guidance.

## Known limits

This release is local-first and single-user. It does not provide dynamic plugins, login, distributed queues, video annotation, a vector database, or a second production Skill. Dataset checkpoint/resume, native/COCO/LabelMe import, richer GUI authoring, and a shared global multi-image budget ledger remain documented gaps; see `docs/KNOWN_LIMITATIONS.md`.
