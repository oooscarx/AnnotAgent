# AnnotAgent Agent + Skill Status

Last updated: 2026-08-28

Current product scope supersedes the earlier broad compatibility notes below: RoboCup now exposes
only the `objects[ball]` annotation task, one Ball Validator, Ball correction reasons and two Ball
Workflow templates. Field and robot roadmap resources were removed from the product package;
remaining algorithms are dormant regression utilities, not registered annotation capabilities.

## Current milestone

M9 — complete. The offline Agent + Skill Alpha Release Gate passes; only explicitly external live
checks remain conditional.

## Audited baseline

- The repository is on `main`, clean at the start of this effort, and 11 commits ahead of
  `origin/main`.
- Existing strengths: checked geometry, typed vision and pipeline Artifacts, persisted model/tool
  history, bounded annotation Runtime, pause/resume/cancel, immutable workflow versions, Dry Run,
  Replay, Classification/VLM Detection/YOLO crates, HTTP vision adapters, Web workflow authoring,
  and multi-Skill project configuration.
- `ModelMessage` already preserves assistant `tool_calls` and tool-result `tool_call_id`; Runtime
  validates one ordered result for every call.
- `SucceededEmpty` exists for task outcomes.

## Confirmed gaps against this master task

- `DomainSkill` is still the only production extension abstraction; manifests do not distinguish
  Capability, Domain and Pack or declare dependencies/conflicts/capabilities.
- Artifact data is split across `VisionArtifact` and pipeline envelopes rather than one strong,
  project/run/image/node-scoped envelope.
- the LLM Workflow Advisor performs one constrained submission, not an iterative inspect → validate
  → dry-run → revise → approval loop.
- correction memory exists as storage data, but no separate bounded Annotation Recovery Agent owns
  risky-candidate recovery.
- RoboCup is a broad Skill rather than a Pack containing `robocup.ball` with robot/field roadmap
  entries.
- TUI has `/skills` but not the complete Advisor and memory command set required here.

## Safety status

- No remote mutation or push is authorized.
- No conversation-provided key will be read, stored, or used.
- Real Qwen and real YOLO checks remain live-conditional; Mock and local HTTP protocol paths are
  release-blocking.

## M0 verification

- domain boundary scan: passed;
- ordered one-result-per-tool-call protocol baseline: passed;
- the complete master request is archived at `docs/execution/AGENT_SKILL_MASTER_PROMPT.md`.

## M1 delivered

- `SkillKind::{Capability, Domain, Pack}`, versioned dependencies and conflicts;
- unified object-safe `Skill` contract with optional node/tool/Validator/Refiner/template/resource
  and taxonomy contributions;
- layered registry with deterministic catalog, exact-version dependency resolution and conflict
  rejection;
- manifest-declared on-demand resources with traversal and undeclared-resource rejection;
- independent dummy Capability, Domain and Pack registration without Core/Runtime branching.

## M2 delivered

- versioned strong `ArtifactEnvelope` with Project/Run/image/node scope, typed payload, parent/item
  lineage, provenance, timestamp and deterministic cache key;
- Published DAG input/output trace envelopes with validation and Replay-compatible serialization;
- strict full-history validation for single/multiple calls, ordered one-result-per-id, duplicate
  IDs, missing IDs/results, unexpected/wrong IDs and nested calls;
- model-visible tool results expose stable Artifact references while full geometry remains in the
  persisted result;
- existing node timeout and provider/DAG cancellation tests are retained as protocol stop proofs.

## M3 delivered

- formal `classification` Capability Skill manifest, compact on-demand resource and
  `classification.whole-image` template;
- whole-image and CropSet subject handling with exact subject/parent references;
- configurable single-label and multi-label Mock output;
- OpenAI-compatible VLM and generic HTTP JSON model bindings through the versioned Pipeline Vision
  Protocol;
- deterministic `classification.verify` node with allow-list and confidence-to-review behavior;
- runtime/catalog registration for both classifier and verifier nodes.

## M4 delivered

- formal `vlm-detection` and `yolo-detection` Capability Skill manifests, bounded resources and
  templates;
- VLM Detection remains structured Image → DetectionSet and accepts a valid empty DetectionSet;
- YOLO remains detection-only with Mock/HTTP JSON backends, class mapping, confidence threshold and
  deterministic per-class NMS;
- Core owns Filter, Map Label, Crop, Attach Result/Attribute, Confidence Gate, Artifact Cache and
  Compute Image Statistics; built-in Image Input/Human Review/Commit remain Runtime-controlled;
- Crop output now records source and crop dimensions, padding, parent detection item, MIME/blob
  reference and an item cache key;
- both detection templates intentionally exclude Crop; the UI composition remains detector →
  filter → crop.

## M5 delivered

- shared serializable `AgentSession`, tool-step trace, status, usage and step/tool/token/cost budget
  contracts;
- iterative Workflow Advisor actions covering schema/Skill/capability/model/resource inspection,
  proposal, validation, revision, Dry Run, metrics and publish approval request;
- the offline policy deliberately produces an invalid binding, consumes the blocking report and
  revises to a valid Draft;
- default HTTP `mock`/`agent` Advisor now runs this loop and returns its trace alongside the
  compatible suggestion payload;
- SQLite migration v4 persists sessions by Project; cancellation and budget stops are explicit;
- terminal success waits for human publish approval and creates no Workflow Version.

## M6 delivered

- `robocup` Pack manifest plus independent `robocup.ball` Domain Skill manifest;
- Robot and Field are not product Skills or annotation tasks;
- `RoboCupBallHardNegativeValidator`, issue taxonomy and duplicate/geometry/footwear/point/line risk
  evidence;
- `RoboCupBallFieldRelationValidator` with inside/outside checks and safe missing-field warning;
- two model-agnostic Ball templates using generic VLM/YOLO capability operations and Core nodes;
- on-demand Ball summary and hard-negative resources with correction taxonomy;
- existing broad RoboCup Skill and all previous hybrid tests remain intact.

## M7 delivered

- strict Correction Memory lookup by exact Project UUID, Skill, Task and Label, with a bounded
  result count and no cross-Project fallback;
- a separate `RoboCupBallRecoveryAgent` that is entered only for Validator risk or matching
  Memory, while clean candidates bypass Agent Session creation entirely;
- observable bounded actions for resource loading, candidate inspection, Memory query, real crop
  statistics, evidence comparison and the final accept/reject/human-review decision;
- application composition that safely resolves an optional Project-local image, runs Recovery,
  rewrites the session to the product Project identity and persists its full trace;
- budget and cancellation stop states that route unresolved high-risk candidates to Human Review;
- an end-to-end two-run proof where the first uncertain candidate requires review and an exact
  scoped correction changes the second decision to rejection.

## M8 delivered

- one layered Skill catalog across Server and Web, grouped as Capability, Domain and Pack with
  versions, contributions, requirements, templates and Project usage;
- Project Build controls that persist enabled layered Skills, automatically include declared
  dependencies and keep legacy Pack Projects compatible during migration;
- visible persisted Workflow Advisor and Recovery Agent sessions, including observable tool input
  and result data, validation, Dry Run, token/cost usage, stop reason and scoped cancellation;
- Review-side Domain Skill selection from the Project's enabled correction taxonomies, so stored
  Memory carries the exact Skill identity instead of an implicit global default;
- Project-scoped Correction Memory display explaining how matching evidence can affect recovery;
- TUI `/skills`, `/skills show`, `/advisor`, `/advisor cancel`, `/memory` and `/history` commands on
  the same application/storage state as the GUI;
- the exact TUI product title `AnnotAgent / Composable Annotation Agent Runtime` and generic empty
  states that do not advertise a domain extension unless a Project enables it;
- browser-verified Advisor waiting/cancellation transitions: cancellation clears pending human
  action and never exposes hidden chain-of-thought.

## M9 delivered

- three real one-command offline demos: `generic-classification`, `generic-detection-crop` and
  `robocup-ball`;
- a four-case Ball demo covering fast-path Commit, white-shoe crop evidence, penalty-mark Review
  and a persisted Correction Memory decision change;
- a safer failed-Dry-Run Advisor revision: the Draft receives a bounded retry-policy change,
  returns to Editing and stops for human editing instead of requesting publication approval;
- a five-minute course walkthrough in `docs/DEMO_AGENT_SKILL.md` and updated R1–R6 mapping;
- the release script now runs the domain/secret scans, all-feature Rust/Web gates, doctor and all
  three Agent + Skill demos;
- 150 Rust tests, including the 100-image persistent pause/restart/resume batch, and 24 Web tests
  pass in the final release run;
- Release Matrix A–H passes offline. Real Qwen and a real out-of-process YOLO worker remain clearly
  `live-conditional` because no operator-owned credentials/service were supplied to this run.
