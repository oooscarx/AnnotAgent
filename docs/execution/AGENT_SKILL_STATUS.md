# AnnotAgent Agent + Skill Status

Last updated: 2026-08-27

## Current milestone

M5 — complete. M6 RoboCup Ball Domain Skill and Pack is next.

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
