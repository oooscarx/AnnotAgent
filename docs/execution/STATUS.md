# AnnotAgent Label Pipeline Alpha Status

Last updated: 2026-09-02 CST

## Geometry-Safe Pipeline Builder M5 — 2026-09-02

- Pipeline Builder now has bounded read-only Tools for model geometry/score contracts, Project
  geometry policy, structured correction summaries, exact calibration and typed refiner
  availability. They are exposed during feasibility analysis without revealing credentials or
  consuming finalization reserve.
- The first bbox Draft is evidence-safe: Qwen VLM geometry is coarse regardless of semantic score;
  unavailable SAM remains an unapplied setup alternative; absent exact evidence produces mandatory
  Human Review rather than a fabricated auto-accept path.
- The full release suite passes 329 active Rust tests plus doc tests (one billable smoke ignored),
  strict all-target/all-feature Clippy, all-feature build, Rustfmt/diff checks, Web TypeScript, 41
  unit tests and production build. No remote was changed.

## Geometry-Safe Pipeline Builder M4 — 2026-09-02

- Added exact Project/model/node/prompt/preprocessing/schema/refiner/dataset geometry calibration,
  robust distribution metrics, six lifecycle states and immutable SQLite persistence.
- Project geometry thresholds and calibration history are available through HTTP APIs. Published
  Draft and formal-Run validation use the persisted exact-context state and mark relevant changes
  Stale; credential rotation is intentionally excluded.
- Passing historical calibration cannot make a semantic confidence gate safe. It must be consumed
  by an explicit geometry evaluation and decision boundary.
- Focused Core, Storage, Application and Server calibration tests pass; complete release checks are
  recorded in the geometry acceptance evidence. No remote was changed.

## Geometry-Safe Pipeline Builder M3 — 2026-09-02

- Human bbox edits now produce durable structured geometry evidence instead of only replacing the
  annotation value: typed reason, original/reference boxes, IoU, normalized/pixel center movement,
  area/width/height ratios, size bucket, Run/node and revisioned Model Profile lineage.
- SQLite migration 10 stores quality report plus correction evidence atomically. Bounded Project and
  Run APIs return the records and scale-aware aggregate summary; Dry Run summaries read the same
  source.
- Review uses the controlled common taxonomy plus registered Skill reasons; unknown codes fail
  before mutation. Legacy records without a frozen Model Profile remain visible but explicitly
  ineligible for calibration.
- Full workspace/all-feature Rust tests, strict all-target/all-feature Clippy, all-feature build,
  Web TypeScript, 41 Web unit tests and production build pass. No remote was changed.

## Run Preview Selection Focus Hotfix — 2026-09-02

- Removed the focusable wrapper around the Run result annotation viewer. Clicking a small bbox no
  longer applies the global blue focus outline to the entire preview component.
- The viewer remains keyboard-accessible through its semantic result-list buttons; arrow navigation
  still works there while the Zoom slider retains its native arrow-key behavior. The duplicate
  visual SVG overlay is explicitly non-focusable.
- Selected geometry now keeps its Label color with the same 1.25 px stroke before and after click,
  transparent fill, and no drop shadow that can visually enlarge very small boxes.
- The reported persisted Run was reproduced before the fix and visually verified after it. Web
  tests pass 41/41, and the production Web build, TypeScript and `git diff --check` pass.

## Dataset Run History Grouping Hotfix — 2026-09-02

- Runs now treats one full-dataset launch as one top-level Dataset Run instead of flattening every
  per-image child Run into apparently duplicated history rows.
- A Dataset Run row shows aggregate image progress, status, usage, Workflow Version and Batch ID.
  It expands on demand to the child image Runs that retain individual Artifacts, errors, Replay and
  Review history.
- `GET /api/batches` now includes persisted aggregate progress and ordered child Run identities, so
  grouping survives refresh and server restart without timestamp heuristics.
- The reported launch is confirmed as one completed Batch containing four image Runs. No additional
  model execution was triggered during diagnosis or verification.
- Server tests pass 17/17 and Web tests pass 41/41; production Web build, strict Server Clippy,
  TypeScript, Rustfmt and `git diff --check` pass.

## Full Dataset Run Entry Hotfix — 2026-09-02

- Test & Activate now exposes `Start full Run` beside the activated immutable Version, and Project
  Overview exposes the same action whenever no Batch or image Run is active.
- Starting a full Run submits a Dataset Batch with the exact Published Workflow `workflow_id` and
  `version`, then opens the Project-filtered Runs page for durable progress and controls.
- Project Guidance now counts terminal work only when its frozen Workflow snapshot matches the
  current Published Workflow Version. Completed Runs from older Versions no longer skip the user
  past `Ready to Run` or hide the launch action.
- The Application suite passes 53 tests (one opt-in billable smoke ignored), along with 17 Server
  tests and 41 Web tests; production Web build, focused strict Clippy, Rustfmt and TypeScript
  checks also pass.

## Persistent Sample Test Recovery Hotfix — 2026-09-02

- Added a read API for the latest SQLite-backed Sample Test associated with each editable Draft.
- Test & Activate now restores the saved report after refresh, route re-entry and Draft switching,
  including result galleries, diagnostics, usage and activation readiness.
- Activating automation no longer removes its Sample Test from the page. The immutable Version and
  its report remain selectable as read-only activated evidence after refresh.
- Reports older than the Draft's latest edit are retained as audit evidence but shown as out of
  date; they cannot silently enable activation for changed automation.
- Server tests pass 17/17, Web tests pass 41/41, the production Web build succeeds, and strict
  Server Clippy, Rustfmt and `git diff --check` pass.

## Pinned Qwen VLM Revision — 2026-09-02

- Updated the DashScope VLM default and Provider preset recommendation from the moving
  `qwen3.7-flash` alias to `qwen3.7-flash-2026-07-15`.
- The live Registry Model Profile now has immutable revision 2 with the exact dated remote model
  ID. Existing Published Workflow snapshots retain revision 1; editable Drafts resolve revision 2.
- The semantic Model Profile change is intentionally `unverified` until the user confirms a
  potentially billable active probe or executes an explicit Sample Test.
- Server tests pass 17/17, Web tests pass 40/40, the production Web build succeeds, and strict
  Server all-target/all-feature Clippy plus Rustfmt pass.

## Provider Registry Dry Run Credential Hotfix — 2026-09-02

- Fixed the legacy flat-Workflow Dry Run path so it receives the already-resolved Provider
  Registry credential, matching Label Pipeline Dry Run and published Run behavior.
- A persisted workspace-file credential no longer falls through to the legacy
  `ANNOTAGENT_API_KEY` environment lookup when a model node executes in the sandbox.
- Added a local OpenAI-compatible regression fixture that rejects every request except the exact
  injected bearer credential. The test passes without configuring the referenced environment
  variable and executes the Classification model node successfully.
- `cargo test --workspace --all-features` passes 308 tests with one opt-in billable smoke ignored;
  strict all-target/all-feature Clippy, Rustfmt and `git diff --check` also pass.

## Product Mock Session Cleanup Hotfix — 2026-09-02

- Confirmed the live Registry, all 11 RoboCup Drafts/versions and the active Workflow contain no
  Mock model binding. The remaining `Mock Detector (offline)` label came from eight persisted
  Pipeline Builder audit sessions created before the product Mock purge.
- Production startup now removes only Agent authoring sessions containing canonical Mock Provider
  or Model identities. Formal Runs, immutable published Workflows, annotations and ordinary Agent
  sessions—including explanatory text that merely mentions Mock—are preserved.
- Storage and Server regressions pass. The live database is backed up before applying the cleanup,
  so the removed legacy Agent-session records remain recoverable outside the active workspace.

## Pipeline Builder Provider Resilience Hotfix — 2026-09-01

- Confirmed the reported GLM-5.2 failure was a transient upstream `502 Bad Gateway`: the same
  persisted Builder session completed three model calls before its fourth call exhausted the
  Provider Profile's bounded retries. Credentials, model identity and the compatible API path had
  already succeeded in that session.
- OpenAI-compatible execution now applies the Provider Profile's minimum and maximum retry delays,
  honors bounded numeric `Retry-After` responses, and validates the delay range.
- Exhausted 5xx and rate-limit errors now report a safe, actionable retry-from-saved-Draft message
  with the number of attempts. HTML gateway bodies are not rendered in the product error surface.
- The affected local GLM Provider passed a non-billable compatibility check (26 discoverable
  models, 41 ms) and now uses four bounded retries with 1–5 second exponential backoff. Its model,
  endpoint and persisted credential reference were left unchanged.
- Focused Provider tests cover two transient 502 responses followed by success, exhausted 502
  behavior, hidden nginx HTML, retry-count metadata, and delay clamping.
- `cargo test --workspace`, strict all-target/all-feature Clippy, Rustfmt and `git diff --check`
  pass after the hotfix; the opt-in billable smoke remains ignored.

## Pipeline Builder Progress-Safety M6 — 2026-09-01

- Replaced the open-ended inspection loop with persisted Builder phases, phase budgets, a protected
  finalization reserve, compact context snapshots, deterministic feasibility, observation caching
  and duplicate suppression. Budget exhaustion now preserves a runnable or blocked editable Draft
  with a typed outcome and concrete next action.
- A missing model produces `ProviderSetupRequired` in four Tool Calls; retry starts with fresh
  budgets while retaining the same Draft. The reproduced 48-call loop now stops after eight calls
  and reduces fixture input tokens from 95,326 to 27,236 (71.4%).
- Structured VLM Detection now requires image input plus structured output or Tool Calls and no
  longer masquerades as native Object Detection. Qwen-style VLM compatibility, Classification,
  VLM Detection + Crop, parent lineage, Draft Diff/Undo and immutable publication are covered.
- The GUI restores persisted Agent progress, phase/budget counters, typed outcomes and setup/retry
  actions. It keeps one primary action and decodes Server errors without HTML entity artifacts.
- `scripts/acceptance.sh` passes 304 Rust tests, strict Clippy/Rustfmt, all-feature build, Web
  typecheck/40 tests/build, doctor and four offline demos. All 35 Chromium journeys pass through a
  deterministic local OpenAI-compatible protocol fixture.
- Offline status is `PASS`; the billable external Provider smoke is `LIVE-CONDITIONAL`. No
  conversation credential, push or remote mutation was used.

## Expert Vision SDK M8 — 2026-09-01

- Closed the offline Expert Vision release matrix: capability-bound RoboCup Ball, specialist-first
  execution, bounded open-vocabulary fallback, semantic-first hard-negative handling and
  evidence-gated Prompt→Mask→BBox refinement all pass without model brands in Core.
- Added a clearly labelled deterministic HTTP contract fixture and browser journeys for Generic,
  SAM, YOLO, RF-DETR and LocateAnything registration. The full 34-test Chromium suite also keeps
  Run, Review, Replay, Export, Provider Registry, responsive and Generic isolation paths green.
- Added the required Geometry Quality, Expert Model Onboarding, YOLO, PIDNet and Grounding DINO
  documentation and aligned the existing architecture, Agent, Guided Experience, RoboCup and
  limitations documents.
- `scripts/acceptance.sh` passes boundary/secret scans, 299 Rust tests, strict Clippy/Rustfmt,
  all-feature build, Web typecheck/40 tests/build, doctor and four offline demos. Python SDK/SAM
  tests pass 16/16. One billable Provider smoke is ignored by default.
- Offline release status is `PASS`. Real external-model accuracy remains `LIVE-CONDITIONAL`; no
  key, checkpoint, model download, push or remote mutation was used.

## Expert Vision SDK M7 — 2026-09-01

- Added a guided, six-step Expert Model setup for SAM and other capability-compatible Workers:
  endpoint trust, live discovery, immutable identity, selected-image sample evidence and explicit
  registration.
- Availability is now durable evidence rather than a toggle. Health, protocol, contracts, weights
  identity and typed sample conversion must all pass; unresolved credential references invalidate
  stale availability after restart without persisting the secret.
- The SAM reference Worker now consumes exact Box Prompt references and emits a generic MaskSet.
  No checkpoint was downloaded or claimed, so real SAM inference remains live-conditional.
- Settings → Vision Workers is directly reachable and the setup UI was verified at desktop and
  480 × 760 without horizontal overflow or browser console errors.
- Full verification passes 299 Rust tests, strict all-target/all-feature Clippy and Rustfmt, 40 Web
  tests, production Web build and 16 Python SDK/SAM tests; one billable smoke remains ignored.

## Expert Vision SDK M4 — 2026-09-01

- Added stable failure classes so Provider/Worker outages, no candidate, semantic mistakes, geometry
  errors, missing scores, domain risks, invalid Artifacts and budget limits cannot be conflated.
- Detection geometry semantics, per-candidate reports and Dry Run aggregates keep VLM confidence
  separate from bbox quality and expose real prompted-segmentation comparison metrics.
- Human bbox edits now return and persist center shift, relative area adjustment and IoU evidence;
  Pipeline Builder can read bounded quality statistics without receiving image bodies.
- Strict Clippy/Rustfmt, 294 Rust tests, Web typecheck and all 40 Web tests pass. One explicitly
  billable Provider smoke remains ignored.

## Expert Vision SDK M3 — 2026-09-01

- Added explicit Box/Point Prompt, Mask and Polygon Set Artifacts plus a capability-neutral
  Conversion Registry.
- The public SAM-compatible flow is now Detection → Box Prompt → Prompted Segmentation → Mask →
  Core Mask-to-BBox. Original box, prompt, mask and refined box retain exact item lineage.
- `capability.segment` runs with the offline mock or a protocol-v1 prompted-segmentation Worker;
  Worker identity, response scope and MaskSet contracts fail closed.
- Pipeline Builder gained `find_artifact_conversion_path`; it cannot claim the SAM refinement cycle
  when any executable node is missing.
- The Published Runtime offline test executes the full SAM-compatible chain and sends its refined
  box to real human review. All 291 Rust tests, strict Clippy/Rustfmt, Python SDK 14/14 tests, and
  Web typecheck/40 tests pass. Full M3 gates are recorded in `EXPERT_VISION_ACCEPTANCE.md`.

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

## Registry-only execution admission — 2026-08-31

- Legacy Run fallback removal: the GUI no longer exposes a singleton Provider Run card or writes it
  from the New Project wizard. Project task graphs without an actual published version are shown as
  unpublished and block Run admission. Formal Run/Batch requests now require an exact Published
  Workflow Version; model nodes must carry frozen Registry Model Profiles, while old Settings remain
  only an explicit migration source. Draft Dry Run uses the same fail-closed Registry resolution.
- A built-in offline Mock Provider and capability-specific Model Profiles bootstrap an empty
  workspace through the same Registry, frozen-snapshot, publication and exact-version path as live
  Providers; Mock is no longer a Settings fallback.
- Local dev/test profiles disable incremental compilation and use bounded debug symbols. The change
  removed 893,011 stale build files (158.0 GiB logical size) and prevents repeated all-feature runs
  from recreating unbounded incremental caches; SAM models and the active Project database remain.
- Strict workspace Clippy, all-feature Rust tests, 39 Web unit tests, production build and all 31
  Chromium E2E scenarios pass.

Registry-only execution admission status: `PASS`.

## Persistent non-Keychain Provider credentials — 2026-08-31

- Provider Registry now offers `Local workspace file` as its default credential source. The secret
  is written atomically below the Git-ignored `.annotagent/credentials` directory; Unix directory
  and file permissions are restricted to `0700` and `0600`.
- Workspace credential locators reject path separators and symlinks. Values remain absent from
  SQLite, Settings, API responses, logs and browser storage.
- Session-only credentials remain available for temporary use. After a restart, an expired session
  reference now reports why it expired and directs the user to Local workspace file instead of
  reporting an internal missing-reference error.
- Strict workspace Clippy, all-feature Rust/doc tests, 40 Web unit tests, production build, the full
  31-scenario Chromium suite and the focused five-scenario Registry suite pass.

Persistent non-Keychain Provider credentials status: `PASS`.

## Product Mock removal — 2026-09-01

- Product startup no longer bootstraps a Mock Provider or any Mock classifier, detector, grounding,
  or segmenter profile. Existing fixture Providers, their active Project bindings/global defaults,
  and Mock-backed unpublished Drafts are removed transactionally; immutable Run history remains.
- Settings → Providers, the Provider catalog, Expert Vision onboarding and Pipeline Builder no
  longer expose Mock choices. New Project recommendations and Workflow recommendations require an
  Available live text-generation Model Profile.
- The Pipeline Builder system contract forbids Mock, fixture and test-only fallbacks. The HTTP
  boundary rejects `advisor=mock`, rejects returned/saved/published Drafts containing Mock model
  bindings, and reports unresolved real bindings instead of substituting a fixture.
- Production runtime no longer converts missing classifier, detector, Grounding or prompted
  segmentation bindings into fake outputs. Missing real Provider/Worker configuration now fails
  closed with an actionable binding error.
- Internal deterministic test doubles remain reachable only through explicit test/demo paths;
  production Server startup cannot register or select them.
- Verification passes all 300 Rust tests (one explicit billable smoke ignored), all 40 Web unit
  tests, Web typecheck/build, the focused Chromium product-boundary regression, and live workspace
  API/SQLite inspection with zero Mock Provider, Model Profile or unpublished Draft entries.

Product Mock removal status: `PASS`.
