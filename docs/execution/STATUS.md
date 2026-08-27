# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-28 CST

## Current scope reset — RoboCup Ball only

The active RoboCup product surface is now deliberately narrow:

- one annotation task: `objects`, with one output label: `ball`;
- one Domain Skill: `robocup.ball`, plus generic VLM/YOLO detection capabilities;
- two templates: VLM bootstrap and detector first;
- white footwear, penalty marks and line intersections are hard-negative evidence only;
- no field-region, field-line, penalty-mark, robot, person, team-color or robot-state annotation;
- the active local workspace contains one `robocup-ball` Project with five B-Human images and a
  fresh history database.

The previous `qwen-live` and `robocup-demo` Projects, previous B-Human exports, pre-reset history,
and `e2e-guided` test residue were removed from the active workspace and placed in the recoverable
`workspace/.annotagent/deleted-projects/2026-08-28/` archive.

## Current milestone

Milestone LP5 — complete: Project Label authoring, Shared Stage/per-Label Pipeline GUI, controlled
Node Catalog editing, Detect & Crop composition, bbox/crop preview, Inspector, Replay, and the full
Rust/Web/browser release gate.

## Product objective

The active release target is **AnnotAgent Label Pipeline Alpha**:

- Project Schema owns annotation semantics and Labels.
- Workflow owns how each Label is produced.
- multiple Label Pipelines may fan out from one shared upstream node;
- a shared node has one compiled identity and executes once per image/configuration;
- Advisor output is always a registry-bounded editable Draft;
- Dry Run, immutable publish, exact-version execution, Artifact inspection, and Replay are real
  Runtime capabilities rather than UI placeholders.

RoboCup Ball is the only current domain example. Earlier broad RoboCup algorithms remain internal
regression fixtures where useful, but are not registered as product tasks, templates or resources.

## Completed

- Workflow Alpha M0–M9 remains the tested foundation: immutable Workflow versions, typed flat DAG,
  cache/checkpoint/Replay traces, Review, batch recovery, Model Registry, controlled Advisor, and
  security boundaries.
- LP1 added `LabelPipeline`, `SharedWorkflowStage`, `PipelineSource`, `PipelineStep`, `ArtifactRef`,
  `DetectionSetArtifact`, `CropSetArtifact`, `ClassificationSetArtifact`,
  `AnnotationCandidateSet`, `ModelBinding`, and `SkillBinding`.
- LP1 compiles one Image Input plus all shared and per-Label steps into the existing flat Workflow
  graph; three Label Pipelines referencing one shared detector compile to one detector node with
  three outgoing edges.
- LP1 implements explicit Image + DetectionSet → CropSet fan-out and DetectionSet +
  ClassificationSet → AnnotationCandidateSet fan-in. Crop and Classification records retain exact
  parent/subject item references.
- LP1 static validation blocks unknown Labels/tasks/nodes/models/Skills, capability mismatches,
  broken shared-stage ownership, missing sources, and Artifact type mismatches.
- Published snapshot content hashing now includes the optional Label Pipeline authoring projection;
  existing Workflow/RoboCup snapshots remain compatible through a defaulted optional field.
- LP2 extends the existing DAG checkpoint, Trace, content-addressed cache, and Replay engine with
  typed Pipeline Artifacts; it does not introduce a parallel Runtime.
- LP2 adds executable Core Crop, Filter, Map Label, Attach Result, Attach Attribute, and Confidence
  Gate nodes. Image Input, Human Review, Commit, Artifact Cache, and Replay remain generic built-ins.
- LP2 adds separate Classification and YOLO Detection Skill crates. The Detection Skill accepts an
  Image and produces only `DetectionSetArtifact`; Crop exists only in Core.
- Classification supports mock, registry-bounded OpenAI-compatible VLM, and generic HTTP JSON
  bindings. Detection supports mock and generic HTTP JSON bindings over protocol v1.
- `replay_from` resets one node and its descendants while retaining completed upstream outputs.
  The crop-classification gate proves classifier Replay does not call the detector again.
- LP3 connects typed Pipeline execution to application-owned published Runs. The selected image is
  materialized as an `ImageArtifact`; node outputs are persisted in the Run checkpoint and committed
  Pipeline candidates become formal stored annotations.
- The application Model/Node Catalog now exposes real mock classifier and detector bindings plus the
  formal Skill and Core node descriptors used by publication validation.
- Three generic example Project Schemas cover whole-image classification, detection, and shared
  detector → Crop → Classification composition without enabling RoboCup.
- A 100-image synthetic Dataset gate executes the exact immutable published whole-image
  Classification Workflow and persists one committed annotation per child Run.
- LP4 constrains both mock and LLM Advisor paths to an exact Project task/Label pair. The LLM may
  only adjust registered bindings and review gates on a safe composition-backed Draft.
- Saving a Label Pipeline Draft recompiles its authoring projection into the one flat Runtime DAG;
  static Label type/Registry errors remain editable but block publish.
- Label Pipeline Dry Run calls the same typed DAG runners used by Published Runs, accepts at most 10
  selected images, and creates neither a durable Run nor a formal annotation.
- Run Inspector exposes each node's configuration, typed inputs/outputs, status, attempts, cache,
  usage, latency, and structured error directly from the persisted checkpoint.
- Replay starts at one exact node in a sandbox, keeps byte-for-byte-equal upstream checkpoint
  outputs, and never recovers credentials from Run history.
- LP5 adds validated Project Schema Label creation without coupling Label semantics to Runtime
  methods. Existing published versions remain immutable.
- The Workflow GUI renders Shared Stages separately from per-Label lanes, exposes typed sources,
  Model Binding, threshold, padding, class mapping, fallback, parameters, Save, Dry Run, and publish.
- The optional Detect & Crop template is visibly and internally detector → filter → Core Crop →
  Classification → Attach Result → Confidence Gate → Commit; Crop is never placed in the detector.
- The Node Artifact Inspector renders the original image, Detection bbox overlays, Crop previews,
  typed JSON inputs/outputs, full configuration, timing/error/usage, and real Replay results.
- A formal `vlm_detection.detect` Skill now provides registry-bounded Image → DetectionSet visual
  grounding without detector weights. Its OpenAI-compatible adapter keeps the image and prompt in
  one multimodal message, supports tool-call and constrained-JSON responses, parses content parts,
  and normalizes Qwen's native 0–1000 xyxy coordinates at the provider boundary.
- The product template `VLM Football Detect & Crop` composes the VLM detector → Core Filter → Core
  Crop/Artifact Cache plus Confidence Gate → Commit. The B-Human demo Project contains five local
  sample images and defaults to the most recently published immutable Workflow.

## LP5 verification

- In-app browser gate passes Project Label creation → target-Label Draft → human-visible Pipeline
  editor → real Dry Run → immutable publish → exact-version Run → Inspector → classifier Replay.
- Browser Replay reports `scene.day.classifier`, Gate, and Commit re-executed while
  `core.image_input` remains preserved. The inspected image and configuration render without layout
  overlap at the tested desktop viewport.
- `cargo test --workspace --all-features`: 126 Rust tests passed, 0 failed; doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Web typecheck/build passed; 8 files and 15 tests passed.
- Live Qwen grounding on `color_771292.png` completed with one football Detection at confidence
  0.98 and normalized rect `[0.432, 0.356, 0.046, 0.046]`; Core produced one parent-linked Crop.
  Formal Run `367e9a0e-5fea-485a-adf7-b437502c2727` completed and its node artifacts were read back
  through the product Inspector API.
- Current full regression: 148 Rust tests passed with all doc tests; strict workspace Clippy passed;
  Web typecheck, production build, and all 24 tests in 10 Web test files passed.
- `./scripts/acceptance.sh`: passed end to end, including domain/secret scans, doctor, and offline
  generic plus RoboCup Ball demos.
- Final in-app browser smoke showed exactly one Project with five B-Human images, only the
  `objects` / `ball` Schema, two RoboCup Ball templates, and the `ball_hard_negative` Validator.
- Core domain scan for RoboCup/YOLO/domain Labels: clean.
- No conversation credential was read, restored, logged, or used.

## Release status

All 20 Label Pipeline Alpha Release Blocking gates have direct offline evidence. Live
OpenAI-compatible inference and configured external HTTP detector quality remain optional deployment
conditions, not blockers for the mock/offline Alpha contract. RoboCup remains regression-tested and
on the Roadmap; it is not the primary acceptance path.

## OpenAI-compatible action recovery and local credentials — 2026-08-28

- Native-tool requests no longer also send a conflicting JSON-schema response format.
- When an OpenAI-compatible model returns a registered `{name, arguments}` action in message
  content instead of `tool_calls`, the adapter promotes it through the same registry validation
  path. Unregistered or malformed content is not promoted.
- The Settings API now stores its write-only API key at
  `<workspace>/.annotagent/credentials/provider-api-key`, with directory mode `0700` and file mode
  `0600` on Unix. Startup migrates and deletes the matching legacy keychain entry; new writes never
  target the keychain.
- Live Qwen Run `76e0ed20-771c-4e53-ab97-b682070b38e6` completed on B-Human
  `color_771292.png`, committed one validated ball annotation, and reported one recognized tool
  call for every model response. Usage was 20,792 tokens across five requests at `$0.032276`.
- Strict workspace Clippy, all 149 Rust tests and doc tests, Web typecheck, all 24 Web tests, and the
  production Web build pass.

OpenAI-compatible action recovery and local credential status: `PASS`.

## Bounded auxiliary-tool convergence — 2026-08-28

- Diagnosed failed Run `709bae51-d2d8-45d7-b713-89b1c8dfdc33`: all eight Qwen responses were
  valid tool calls, but every call selected `evaluate_ball_hard_negative`; no submission action was
  selected before the configured turn budget ended.
- Runtime now detects two consecutive successful auxiliary evidence calls and reserves exactly one
  bounded convergence turn exposing only terminal actions. A failed terminal candidate returns to
  the normal recovery protocol; auxiliary tools are not permanently disabled.
- The final configured model turn is also terminal-only. No task/model/tool budget was increased.
- Live Run `6df70d25-e1fe-4233-8ec1-cd4314f665ca` completed on the same B-Human image with tool
  sequence `evaluate_ball_hard_negative → evaluate_ball_hard_negative →
  submit_annotation_candidates`, zero validation issues, and one committed Ball annotation. Usage
  was 15,641 tokens across three requests at `$0.034535`.
- Strict workspace Clippy, all 151 Rust tests and doc tests, Web typecheck, all 24 Web tests, and the
  production Web build pass.

Bounded auxiliary-tool convergence status: `PASS`.
