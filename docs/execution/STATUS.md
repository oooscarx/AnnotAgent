# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-08-31 CST

## Provider Registry + Pipeline Builder Alpha M8 — 2026-08-31

- Added an explicit, transactional and idempotent compatibility import for the legacy Provider,
  model name and `default-vision` Project bindings. The preview/confirm flow stores only a secret
  reference, preserves existing user bindings and leaves Published Versions and Run history
  unchanged.
- Published Workflows now freeze exact Model Profile revisions and Provider execution semantics.
  New Run/Batch admission fails closed when the current Provider/Profile is disabled or unavailable,
  while history remains readable and immutable.
- Draft Dry Run uses the same Registry Profile snapshots and write-only SecretStore resolution as
  publication. Published Runtime routes same-operation nodes to distinct frozen Profiles under one
  Provider; multiple Provider credentials in one Workflow remain an explicit fail-closed limitation.
- Provider Settings includes the guided migration without competing with the page's primary action.
  Provider lifecycle E2E covers reference conflicts, disable/re-enable, session key rotation without
  echo, migration idempotency and 390 px layout.
- Final validation passes: formatting, strict workspace Clippy, all-feature build, 275 Rust tests
  plus doc tests (one explicitly billable smoke ignored), 38 Web tests, TypeScript, production build
  and all 31 Chromium journeys. Secret-pattern, browser-storage and diff scans pass.
- The Provider Builder release matrix is closed for offline Alpha. Real billable Provider calls and
  native desktop Keyring interaction remain `LIVE-CONDITIONAL`; no key, push or remote change was
  used.

## Open-Vocabulary + Specialist Detection M10 — 2026-08-31

- Detector Cache Keys now include canonical image content, model/version/checkpoint/protocol,
  model/node configuration, query, target Label mapping and enabled Skills. Tests prove identical
  specialist/open-vocabulary calls execute once, Gate-only edits preserve both caches, and only the
  affected detector reruns after query or immutable model/configuration edits.
- Exact hybrid sandbox Replay preserves specialist, validation and Recovery ancestors and leaves
  the committed Annotation count at one. The 100-image durable gate now runs the exact published
  hybrid Workflow through pause, application restart and resume with 100 unique child Runs pinned
  to one immutable content hash.
- The complete 24-scenario Chromium suite covers mixed evidence, agreement/conflict, both source-box
  revisions, unavailable/timeout Workers, Generic isolation, URL restoration, keyboard behavior and
  compact responsive layouts. Native browser 200% zoom remains an explicit `MANUAL` check.
- Generic Server Review now presents persisted Skill-owned Validation Issue messages without a
  domain-string branch; the production Agent + Skill boundary scan passes while Core's concrete
  test-fixture Labels remain available for generic contract tests.
- Added specialist, per-version license, hybrid Workflow and five-minute demo documentation. Full
  verification passes: 221 Rust tests and doc tests, strict Clippy/build/fmt, 35 Web tests,
  typecheck/build, 24 E2E tests, Worker syntax and no-weight health probes, doctor, three offline
  demos, boundary/secret scans and diff checks.
- The detection release matrix is 88 `PASS`, zero `OPEN`, one `LIVE-CONDITIONAL`. Real
  LocateAnything/RF-DETR GPU smokes remain conditional because this Darwin arm64 host has no NVIDIA
  runtime or configured legal weights/immutable checkpoint metadata. No API key, model weight,
  download, push or remote mutation was used.

## Open-Vocabulary + Specialist Detection M9 — 2026-08-31

- Guided New Project recommendations now use the real Model Registry: label-compatible specialists
  are recommended first, while cold-start Projects are offered description-based detection without
  a training-data claim or hidden live availability claim.
- Models and Settings expose an editable Detection Worker collection, immutable model/license facts,
  score semantics, per-request cost and real health/capability actions. Unsupported visual prompt is
  visibly disabled.
- Run/Dry Run summaries show fallback and cache counts. Results, Debug and Review preserve every
  independent source box and score semantic, explain missing confidence/agreement/conflict, and let
  reviewers adopt a source box through the normal revision/acceptance path.
- TUI adds model, endpoint-test, Artifact and Replay inspection. Real browser verification covered
  the core pages at 1024px/760px without horizontal overflow and confirmed exact Debug URL reload.
- Full verification passes: 220 Rust tests and doc tests, strict Clippy, 35 Web tests, Web
  typecheck/build, formatting and diff checks. M10 remains responsible for detector-cache proofs,
  complete failure-path browser fixtures, release documentation and live-conditional smoke records.

## Open-Vocabulary + Specialist Detection M8 — 2026-08-30

- RoboCup Ball now has a capability-bound specialist + open-vocabulary fallback Workflow. The
  reusable Skill contains no concrete detector/classifier ID; each Project maps generic
  capabilities to Registry models when creating an editable Draft.
- High specialist evidence commits without fallback. Empty, low-score, hard-negative,
  field-relation or scoped Correction Memory risk may make one bounded fallback call. Successful
  but unresolved evidence follows a published Candidate projection → Crop → Classification branch;
  budget/Worker errors preserve evidence and route Human Review.
- Candidate projection preserves every source box and score semantic. Crop classification retains
  subject/parent references and can explicitly reject `not_football`; no aggregate score is
  invented from mixed evidence.
- Added real-binding and no-key Mock Projects plus seven deterministic scenario definitions. Exact
  published Runtime tests cover the clean fast path and empty-result verification path; focused
  tests cover agreement, conflict, hard negatives, budget exhaustion and Worker errors.
- Full verification passes: 219 Rust tests and doc tests, strict Clippy, Rust build, 34 Web tests,
  Web typecheck/build, formatting, diff checks and the Core/Runtime model/domain boundary scan.
  M9 is the next active milestone; no push, API key, model download or remote mutation was used.

## Open-Vocabulary + Specialist Detection M7 — 2026-08-30

- Capability-driven Advisor recommendations now distinguish detection cold start from a
  label-compatible available specialist. Cold start adds open-vocabulary detection, bounded Crop
  verification and Review when every typed binding exists; specialist-first adds conditional
  Recovery. Suggestions remain editable Drafts and are never auto-published.
- The published Runtime now executes a domain-neutral Detection Recovery Agent. High-confidence
  specialist evidence skips fallback; empty, low-score, Domain Validation or correction-risk facts
  may invoke one registered Open Vocabulary backend call.
- Step/tool/cost budgets are checked before the call. Disabled policy, missing queries,
  insufficient budget or Worker failure preserve primary evidence and stop at Human Review.
- Candidate matching and final Evidence Gate can change the initial fallback decision without
  averaging independent scores. Persisted Agent Trace records reason codes, counts, timing,
  capability/model identity, budget and stop condition, never hidden reasoning or image bytes.
- Mock unit and exact published-Run integration tests cover the clean fast path, fallback path,
  changed decision, Domain risk, budget stop and durable trace. M8 builds on this foundation above.

## Current scope reset — RoboCup Ball only

The active RoboCup product surface is now deliberately narrow:

- one annotation task: `objects`, with one output label: `ball`;
- one Domain Skill: `robocup.ball`, plus generic VLM/YOLO detection capabilities;
- two templates: VLM bootstrap and specialist with open-vocabulary fallback;
- white footwear, penalty marks and line intersections are hard-negative evidence only;
- no field-region, field-line, penalty-mark, robot, person, team-color or robot-state annotation;
- the active local workspace contains one `robocup-ball` Project with five B-Human images and a
  fresh history database.

The previous `qwen-live` and `robocup-demo` Projects, previous B-Human exports, pre-reset history,
and `e2e-guided` test residue were removed from the active workspace and placed in the recoverable
`workspace/.annotagent/deleted-projects/2026-08-28/` archive.

## Previous Label Pipeline milestone

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

## Formal Annotation overlay in Run detail — 2026-08-28

- `GET /api/runs/{run}/annotations` now returns formal Run Annotations and the resolved Project
  image index independently of Pipeline checkpoint availability.
- Legacy compatibility Runs resolve their source image through the persisted image digest; Pipeline
  Runs reuse their checkpoint image index.
- Run Result preview merges committed bounding-box Annotations with Detection/Crop Artifacts,
  deduplicates exact matching boxes, shows annotation count, label, confidence, and the correct
  source image.
- Browser verification on Run `6df70d25-e1fe-4233-8ec1-cd4314f665ca` displayed
  `color_771292.png`, one `ball` box at 95% confidence, and a non-black label color despite the Run
  having no Pipeline checkpoint.
- Strict workspace Clippy, all 151 Rust tests and doc tests, Web typecheck, all 25 Web tests, and the
  production Web build pass; browser console inspection reported no warnings or errors.

Formal Annotation overlay status: `PASS`.

## RoboCup Ball foreground refinement — 2026-08-28

- The active Ball task now runs `ball_foreground_refiner` after VLM submission and before the
  existing hard-negative Validator and Review policy.
- The Refiner is Skill-owned and uses bounded local foreground segmentation against green-field
  pixels. It is not presented as SAM and introduces no RoboCup branch into Core.
- Painted field lines are rejected as thin axis support. Inconclusive evidence preserves the exact
  VLM box and emits `ball_foreground_refiner_fallback` for Human Review.
- Runtime persists both the original candidate and refined candidate as revision-linked Artifacts.
- Live Qwen Run `d1500707-18a8-4d30-87be-2a379e65e34f` executed two evidence calls, submission,
  refinement, validation, and commit. The initial calibration reduced the candidate area while
  retaining confidence and produced no validation issue.
- Later live Runs returned increasingly oversized/upward-biased coarse rects and safely selected
  fallback/Review. Replaying the widest exact candidate after final tuning maps
  `[0.438, 0.335, 0.065, 0.075]` to
  `[0.4375, 0.35714287, 0.036764707, 0.04017857]` at quality `0.544`; the horizontal field line and
  an unrelated robot foot no longer widen the result.
- Strict workspace Clippy and all 153 Rust unit/integration tests plus doc tests pass. Browser
  verification shows one colored `ball 95%` overlay on the successful live Run with no console
  warning or error.

RoboCup Ball foreground refinement status: `PASS` (deterministic local backend). A real SAM worker
remains an optional HTTP Vision Protocol backend and is not claimed by this milestone.

## RoboCup Ball SAM2 prompted refinement — 2026-08-28

- `sam_prompted_refiner` now executes asynchronously through HTTP Vision Protocol v1. It sends the
  image and a foreground-seeded VLM box to a real SAM2.1 worker and derives the final bounding box
  from the returned instance mask, not from worker metadata.
- `examples/sam2_vision_worker.py` loads the official SAM2.1 Hiera Tiny checkpoint in a
  workspace-private Python environment. `scripts/setup-sam2.sh` installs a pinned SAM2 revision and
  checkpoint; `scripts/start-sam2-worker.sh` starts the loopback-only worker on port 8790.
- Runtime now supports asynchronous, cancellable Refiners and persists auxiliary mask Artifacts.
  The Run lineage is VLM Candidate → SAM InstanceMask → Refined Candidate. If SAM is unavailable or
  returns unsafe geometry, the Skill emits `sam_prompted_refiner_unavailable`, uses the explicit
  local foreground fallback, and requires Human Review.
- The first box-only live probe exposed an unstable 46% mask for an upward-biased VLM prompt. The
  final hybrid prompt fixed that failure mode: live Run `acc947fa-48e9-4dc8-a412-799f723004b0`
  produced a 92% SAM mask and bbox `[0.44117647, 0.35714287, 0.03308824, 0.04464286]`, with zero
  validator issues. GUI verification shows `3 Artifacts`, `1 Annotations`, and `ball 92%`.
- `./scripts/acceptance.sh` passes end to end: domain/secret boundaries, formatting, strict Clippy,
  all workspace Rust tests and doc tests, Web typecheck/25 tests/build, doctor, and offline demos.

RoboCup Ball SAM2 prompted refinement status: `PASS` (real local SAM2.1 worker).

## Published VLM → SAM → editable Review → Commit — 2026-08-29

- The RoboCup Ball VLM starter is now a valid publishable typed Workflow:
  `Image → VLM DetectionSet → Filter → SAM Refiner → Validator → Confidence Gate → Human Review → Commit`.
- The generic `annotation_refiner` adapter refines every DetectionSet item while preserving its
  detection identity. SAM masks remain inspectable evidence Artifacts; only the final bounding box
  enters the annotation Review queue.
- DetectionSet validation now preserves typed output instead of dropping the Pipeline Artifact at
  the Validator boundary.
- Review resolves the exact source image, renders at its natural aspect ratio, and exposes drag plus
  four-corner resize controls. Unsaved edits are persisted before `Accept & commit`.
- Accept resumes only the frozen Review and Commit descendants from the immutable checkpoint; VLM,
  Filter, SAM, and Validator are not re-executed. Retry is idempotent and a failed resume restores a
  reviewable Run state.
- Live Published Run `11312a03-f9ba-402b-af0c-0e89252a4ec7` reached Commit after human acceptance;
  all eight nodes are `succeeded` and the Run is `completed` with an `annotation_committed` event.
- Live Published Run `52988688-84ba-4892-86e1-ef29f8c0195d` is intentionally left at Review for GUI
  inspection. It contains exactly one editable Ball bbox; Qwen detected the Ball and SAM2 refined it
  to `[0.44301471, 0.42857143, 0.02941176, 0.03571429]` at 76% confidence.
- `./scripts/acceptance.sh` passes: strict Clippy, all workspace Rust/doc tests, 25 Web tests,
  production build, doctor, and the three offline demos.

Published editable-review pipeline status: `PASS`.

## Multi-prompt SAM recovery for imprecise VLM boxes — 2026-08-29

- Review history confirmed that the four newest candidates had executed `sam_prompted_refiner`;
  the remaining error was an inaccurate VLM prompt box, not a missing SAM stage. A box-prompted
  segmenter cannot recover an object that lies outside its only prompt.
- The SAM HTTP worker now accepts a bounded `box_prompts` set, runs the image encoder once, and
  returns the multimask candidates for every prompt with prompt/mask identity and tight-box
  metadata.
- The RoboCup Ball Skill expands and shifts the coarse VLM box, evaluates all plausible SAM masks,
  and selects using SAM confidence, geometry, proximity, non-field pixels, and distinctive ball
  appearance. Core and the generic HTTP Vision Protocol remain domain-neutral.
- Live Published Run `1d20cd51-3c04-4d4b-912f-e43f83e31d6a` corrected the VLM box
  `[0.44, 0.41, 0.035, 0.04]` to the visible football at
  `[0.4375, 0.35714287, 0.03860294, 0.05133929]`. It selected one result from five plausible SAM
  candidates and routed exactly one editable bounding box to Review.
- Review now resolves Pipeline Artifact lineage as well as legacy Vision Artifact lineage. The
  inspector visibly reports `Source Node: refine_ball` and `Refinement: SAM 2.1 multi-prompt`,
  while a local fallback is explicitly labelled as having no SAM.
- `./scripts/acceptance.sh` passes the boundary check, formatting, strict workspace Clippy, all
  workspace Rust/doc tests, Web typecheck, all 25 Web tests, production build, doctor, and three
  offline demos. Browser verification confirmed 15 inspectable mask Artifacts, the final
  `ball 68%` overlay, four resize handles, and the explicit SAM refinement label.

Multi-prompt SAM recovery status: `PASS` (real local SAM2.1 worker; Human Review remains required
by the configured 0.99 confidence gate).

## Grid-assisted Qwen grounding experiment — 2026-08-29

- The VLM detector can now opt into a bounded `localization_grid`. It sends the untouched source as
  Image 1 and a same-size dashed magenta grid copy as Image 2. The prompt requires recognition from
  Image 1 and coordinate calibration only from Image 2, so SAM and saved source pixels remain clean.
- The adapter now honors the RoboCup template's existing `target_description` alias. Previously the
  precise football description was silently replaced by the detector's generic default.
- Qwen 3.7 models default to their native integer `0..1000` XYXY grounding convention. The adapter
  normalizes this at the Provider boundary, records the effective coordinate format and grid
  configuration, and keeps non-Qwen providers on normalized XYWH.
- A normalized-coordinate A/B run showed that the grid moved the VLM box from
  `[0.430, 0.375, 0.040, 0.040]` to `[0.440, 0.340, 0.040, 0.050]`, closer to the small football,
  but two-image normalized grounding was slower and still missed one of three batch images.
- The final Grid + native Qwen grounding batch localized all three inspected images correctly in
  2.36–3.74 seconds per VLM node. SAM produced final confidences of 82%, 93%, and 86%; all remained
  in Review because the experimental Workflow intentionally retains a 0.99 gate.
- The experiment exposed a separate SAM ranking defect: a 61.9% oversized mask could outrank a
  91.5% tight mask. Ranking now caps implausible area expansion, gives more weight to SAM confidence
  and overlap, and records the selection score for every plausible mask.
- Published Workflow `83c2af7b-9ae2-4a37-b91a-8e5c47795494@v1`, named
  `RoboCup Ball · Grid + Qwen Grounding + SAM`, is the Project default. Experimental Batches were
  cancelled after evidence collection so they do not block Start Run; their immutable child Runs
  and Review evidence remain inspectable.
- `./scripts/acceptance.sh` passes the domain boundary check, formatting, MSRV-aware strict Clippy,
  all workspace Rust/doc tests, production build, all 25 Web tests, doctor, and all offline demos.

Grid-assisted Qwen grounding status: `PASS` on the three-image exploratory batch. This is evidence
of improvement, not a dataset-wide accuracy claim.

## Review priority rendering and API latency — 2026-08-29

- Review startup was traced to backend aggregation rather than static assets or image delivery.
  Before the fix, `/api/projects` took 5.08–5.31 seconds and `/api/reviews` took about 5.17 seconds;
  the image list and 358 KB source image each took about 1 ms.
- Dashboard review count now uses one direct SQLite status-column count instead of materializing every Review
  Artifact. Initial SSE connection no longer duplicates the Dashboard request.
- Review aggregation filters pending annotations before loading Run evidence, reuses one SHA index
  per Project, hashes encoded bytes without decoding pixels, and avoids repeated full History scans.
- A routed Review fetches its 1.1 KB detail first while the complete queue loads in the background.
  On the live workspace, `/api/projects` is now 81–102 ms, routed Review detail is 95–105 ms, and
  the complete 16-item queue is 566–590 ms.
- In-app browser measurement on the restarted `127.0.0.1:8787` service reached the selected Review,
  decoded 544×448 source image, and editable overlay in 399 ms. The full queue then hydrated without
  replacing or shifting the selected Review.
- Storage and server tests, 30 Web unit tests, production build, and the full Web E2E suite pass
  (9 passed, 1 fixture-dependent test skipped).

Review priority-rendering status: `PASS`.
