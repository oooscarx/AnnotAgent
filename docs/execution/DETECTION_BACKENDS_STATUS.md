# Detection Backends Status

Updated: 2026-08-30

## Current Milestone

M8 — Capability-bound RoboCup Ball hybrid Workflow (complete)

## Completed

- Read the 2,771-line master prompt and stored an exact repository copy.
- Verified `main` began clean and 25 commits ahead of `origin/main`.
- Audited Core, Runtime, Application, Provider, Server, Storage, Capability/Domain Skills, Web,
  TUI, examples, migrations, workers, and current execution evidence against code.
- Verified the existing offline baseline: 166 Rust tests and 32 Web tests pass.
- Verified both existing Python reference workers parse without executing or downloading models.
- Checked official LocateAnything and RF-DETR model/license sources and recorded variant-specific
  handling instead of a global commercial-use assertion.
- Created the Master Plan, Status, Decisions, Acceptance, Blockers, and Known Limitations ledgers.
- Confirmed the Core/generic Runtime production scan contains no model-brand or RoboCup branch;
  domain names occur only in the dedicated Runtime integration test/dev dependency.
- Added domain-neutral `OpenVocabularyDetection` and `PhraseGrounding` capabilities beside the
  existing `ObjectDetection`; no model-branded node kind was introduced.
- Expanded the existing Model Registry descriptor with provider/backend metadata, version,
  architecture, checkpoint SHA-256, training dataset version, protocol version, typed input/output
  contracts, score semantics, runtime requirements, label space, license metadata, and explicit
  availability.
- Registration now normalizes legacy descriptors, validates backend capability/kind, unique values,
  checkpoint hashes, contract consistency, official-license references, and executable status.
- Preserved JSON migration compatibility: older `http_json` values read as `http_vision`, older
  descriptor fields populate the structured contracts, and `VisionModelDescriptor` remains a
  source-compatible alias boundary for existing callers.
- Replaced mandatory Detection confidence with `DetectionScore { value: Option<f32>, semantics }`;
  finite range validation rejects NaN, infinity, and out-of-range values.
- Added source model/capability, query/model-vs-Project labels, independent `DetectionEvidence`,
  bounded raw-payload references, `CandidateClusterSet`, agreement/conflict types, and stable
  parent lineage to both input DetectionSets.
- Added Detection Artifact schema v2 and explicit JSON migration for v1 field names and persisted
  checkpoints. Migration synthesizes source evidence from the historical set/model identity but
  never invents a missing score; unsupported future schema versions fail closed.
- Migrated Mock YOLO, VLM detection, generic HTTP Pipeline, Core transforms, refiners, Review edits,
  Run materialization, Dry Run, and Web preview DTO parsing to the evidence-aware contract.
- Ordinary Confidence Gate now compares only calibrated/relative scores. Missing, ranking-only,
  and unknown scores route to Review instead of receiving a default; Filter retains such candidates
  for Evidence Gate or Human Review.
- Added typed Web API DTOs and legacy-compatible geometry/label/score parsing for existing history.
- Added one versioned Detection Worker v1 contract for health, capability discovery, inference,
  structured errors, timeouts, and cancellation, shared with the existing Pipeline protocol
  version constant so the two cannot drift.
- Added a generic HTTP Detection backend that converts validated normalized `xyxy` wire geometry
  to Core `xywh` Artifacts while preserving query/model/Project labels, source identity, optional
  score semantics, raw-response hash/size references, and valid empty results.
- Hardened all generic HTTP Vision/Pipeline transports: loopback-only by default, explicit HTTPS
  opt-in for remote hosts, no embedded credentials/query/fragment, redirects disabled, bounded
  requests/responses, bounded retries, connect/request timeouts, and no raw response logging.
- Added adversarial Worker contract coverage for malformed/oversized payloads, unknown fields and
  local paths, NaN/out-of-bounds/reversed coordinates, duplicate identities, undeclared labels,
  capability/model/version spoofing, redirect credential leaks, timeout, and forwarded cancel.
- Published the exact v1 JSON contract and security boundary in `docs/HTTP_VISION_PROTOCOL.md`.
- Added the domain-neutral `annotagent.open_vocabulary_grounding` Capability Skill with separate
  Open Vocabulary Detection and Phrase Grounding nodes, a strict text-query JSON Schema, Mock
  backend, valid-empty behavior, Query-ID-to-Project-Label mapping, and an editable
  Image → Grounding → Review → Commit Workflow template.
- Added a shared-registry HTTP adapter for both grounding capabilities. It discovers Worker facts,
  preserves optional scores and source/query evidence, validates normalized geometry, accepts an
  empty DetectionSet, and persists only a bounded raw-response hash reference.
- Added `examples/locate_anything_worker.py`, a loopback-only adapter over NVIDIA's official
  LocateAnything worker API. It loads only explicitly configured local code/model directories,
  never downloads weights, converts parsed pixel `xyxy` boxes to normalized coordinates, supports
  multiple text queries and cooperative cancellation, and never fabricates a confidence value.
- Added a disabled-by-default LocateAnything Detection Worker profile to local Settings. Disabled
  or unavailable Workers remain visible in the Model Registry without blocking application startup.
- Added live Worker Test support and Model metadata to the Models page, plus persistent Detection
  Worker endpoint/enable/remote-opt-in editing in Settings. Visual prompt is disabled with the
  exact unsupported-capability reason and static validation rejects hidden/manual use.
- Added Generic Project documentation and a runnable offline template test proving Draft → Dry Run
  → immutable publish → exact-version Run → persisted DetectionSet → Human Review without RoboCup.
- Documented local deployment, security, model/license restrictions, missing-score behavior, and
  the distinction between Mock/contract evidence and live GPU evidence.
- Added the backend-neutral `annotagent.object_detection` Capability Skill with the formal
  ObjectDetection request/options contract, strict JSON Schema, Model→Project class mapping,
  confidence/IoU/max post-processing, valid-empty Mock backend, and an editable specialist Review
  template. The Skill owns neither Crop nor a detector brand.
- Registered the generic Object Detection operation/model in Application Runtime, Draft/Dry Run,
  exact published execution and persisted Pipeline Artifact inspection. A Generic Project proves
  class mapping and finite score preservation without loading RoboCup.
- Extended Worker validation with an optional exact expected label space. Capability discovery
  rejects mismatched vocabularies and inference rejects model labels outside the configured space.
- Added `examples/rfdetr_vision_worker.py`, a loopback-only adapter using RF-DETR's official
  `from_checkpoint` and `predict` APIs. It verifies an explicit local checkpoint SHA-256, requires
  immutable architecture/model/dataset/label-space metadata and safe checkpoint loading, performs
  bounded class-aware NMS, and never downloads or fabricates inference.
- Added a disabled-by-default versioned specialist Worker profile. Enabling it is blocked until
  architecture, model version, checkpoint SHA-256, training dataset version, label space and exact
  weight-license metadata are present. Existing local Settings gain the profile additively without
  overwriting the saved LocateAnything profile.
- Expanded Settings to edit all specialist identity fields and Models to expose the profile,
  endpoint, score semantics and license summary through the same live Test Worker path.
- Added protocol and integration evidence for discovered label space, finite relative score,
  normalized coordinates, class mapping, no-object success, metadata persistence and unavailable
  Worker startup.
- Added generic `core.match_detection_sets` execution for exactly two same-image DetectionSets.
  Matching is stable and one-to-one by Project Label/IoU, retains unmatched candidates, and emits
  explicit MultiSourceAgreement, GeometryConflict, LabelConflict or SingleSource clusters.
- Extended Detection Evidence with migrated Project Label and source Capability facts so conflicts
  remain explainable without consulting a model brand. Candidate Cluster validation state now
  travels through Review/Commit and persisted Artifact lineage.
- Added generic `core.evidence_gate` with typed input/config/report contracts and exact `accept`,
  `fallback`, `review`, and `reject` routes. It consumes propagated validator issues and optional
  Correction Risk, treats missing/ranking/unknown scores as incomparable, and emits observable
  reasons rather than hidden reasoning.
- Added a Generic Project integration test for offline specialist + open-vocabulary fan-in through
  Draft, Dry Run, immutable publish, exact-version Run and persisted inspection. The scored and
  score-less contributions remain independent.
- Added inspection API route/metadata fields, Candidate Cluster bbox preview, and a responsive Run
  Debug Evidence Decision card showing decision, reasons, source models, candidates and domain
  issue count. Multi-source Annotation projection leaves aggregate confidence unset.
- Added a capability-driven deterministic Advisor path for detection-only Projects. It inspects
  Project Labels, exact specialist label space, Model availability and configured cost limits.
  Cold start produces Open Vocabulary → Crop Classification → Attach Result → Review when every
  binding exists; a compatible specialist produces specialist-first execution with a conditional
  Recovery node. Both results remain editable `Suggested` Drafts.
- Added versioned, domain-neutral Detection Recovery request/policy/report contracts with explicit
  query bindings, Agent budget, maximum fallback calls, estimated fallback cost, IoU policy,
  initial/final Evidence Gates, action and stop condition.
- Added the executable `agent.detection_recovery` Runtime node. It accepts Image + one primary
  DetectionSet, projects single-source evidence, and calls a Registry-bound Open Vocabulary
  backend only after an explicit `fallback` decision. A high specialist score completes with zero
  fallback calls.
- Empty specialist results, low comparable score, Validation Issues and Correction Risk can
  request fallback. User-disabled fallback, missing queries, insufficient step/tool/cost budget,
  Worker error or exhausted call limit preserve primary evidence and route to Human Review.
- Recovery makes at most one fallback request in the Alpha policy, matches its result without
  averaging scores, re-evaluates final evidence, and can change an initial `fallback` decision to
  `accept`. It never performs an unbounded model loop.
- Persisted Agent Trace contains only structured inputs, result counts, reason codes, timing,
  selected capability/model and stop facts. It excludes image bytes, query text and hidden
  reasoning. Application history persists the session beside the Run and accounts Recovery cost.
- Mock unit/integration tests cover the high-score fast path, empty-result fallback, domain-risk
  fallback, insufficient-cost Review, changed decision after agreement, one-call stop, persisted
  trace and exact published Runtime execution.
- Extended Skill manifests with explicit required and optional capability declarations. The
  RoboCup Ball Domain Skill requires only generic Crop/Human Review Core capabilities and advertises
  Object Detection, Open Vocabulary Detection, Classification, field and robot evidence as optional.
- Added Project-owned capability bindings. Draft creation resolves each model-backed template node
  from `capability.<name>` configuration on the enabled Domain Skill; the Skill template contains no
  concrete model identity or backend-brand branch.
- Replaced the former detector-first Ball recipe with the bounded
  `robocup.ball.specialist_with_open_vocab_fallback` Workflow. Accepted specialist evidence commits
  directly; empty, low-score, Domain-risk or Correction-risk evidence may call one fallback; unresolved
  evidence enters typed candidate projection, Crop and crop Classification verification.
- Added generic Candidate Cluster → DetectionSet projection and explicit candidate rejection nodes.
  Projection preserves every independent evidence record and source score semantics, while geometry
  or label conflict becomes a Validation Issue instead of a fabricated aggregate confidence.
- Domain Validator execution now reads Correction Memory from the exact Project/Skill/task/Label
  scope and attaches a structured Correction Risk fact. RoboCup Ball combines hard-negative and
  optional field-relation validation; lack of field evidence is not penalized when the Project does
  not declare field semantics.
- Crop Classification verification supports configured accept/reject labels and retains the exact
  crop subject/parent reference. `not_football` takes an explicit terminal reject route; uncertain
  classifications and conflicts remain reviewable.
- Added live and no-key Mock RoboCup Ball hybrid Projects. Concrete specialist, fallback and
  classifier model IDs exist only in Project configuration. The offline scenario manifest covers
  high specialist evidence, empty fallback, agreement, conflict, white-shoe rejection, insufficient
  budget and Worker crash.
- Exact published-Workflow integration proves the 0.92 specialist fast path skips fallback/Crop
  classification and commits, while an empty specialist result invokes fallback, projects evidence,
  creates a Crop, classifies it and ends in review without failing the Run. The same frozen graph
  proves `not_football` reaches only the explicit Reject terminal and an opted-in specialist Worker
  error remains structured, invokes fallback and finishes in Review rather than Failed.

## In progress

- None. M8 is ready for its independent local commit.

## Next step

M9 — expose Guided recommendations, Worker setup, mixed-evidence Results/Review explanations,
source-box choice, TUI model controls, URL restoration and responsive accessibility evidence.

## Latest tests

| Area | Command | Result |
| --- | --- | --- |
| Rust | `cargo test --workspace --all-features` | PASS — 219 tests, 0 failed; doc tests pass |
| Rust lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Rust build | `cargo build --workspace --all-features` | PASS |
| Web | `npm --prefix web test -- --run` | PASS — 12 files, 34 tests |
| Web types | `npm --prefix web run typecheck` | PASS |
| Web build | `npm --prefix web run build` | PASS |
| Worker | parse all tracked Python workers with Python 3.14 | PASS — syntax only; no model loaded |
| Locate worker | start without weights; request `/health` and `/v1/capabilities` | PASS — unavailable health is truthful; capabilities remain discoverable |
| RF-DETR worker | start without checkpoint; request `/health` and `/v1/capabilities` | PASS — immutable metadata requirement is reported; no fixture inference |
| Browser | prior Guided Release browser evidence | PASS at 1024px and 720×450; M6 evidence card is built/parser-tested, full mixed-evidence visual browser gate remains M9/M10 |

## Latest local commit

This document's containing M8 commit: `feat(robocup): add specialist and open-vocabulary ball workflow`

## Audited baseline

### Existing strengths

- `VisionCapability::ObjectDetection`, generic model/node registries, Mock and HTTP adapters exist.
- Label Pipelines already provide shared model stages, typed Detection/Crop/Classification sets,
  exact Crop parent references, immutable versions, Dry Run, durable checkpoints, cache-aware
  Replay, pause/resume/cancel, Run/Artifact inspection, Review, and export.
- Classification, VLM Detection, and YOLO crates are registered Capability Skills; RoboCup Ball is
  separated as a Domain Skill and its current templates are model-agnostic.
- HTTP workers already expose health/capability/infer concepts and avoid logging image bodies.
- Guided Experience presents a single Project journey and keeps technical graph details optional.

### Confirmed gaps

- Candidate Match and Evidence Gate execution nodes are not implemented yet; M2 only establishes
  their typed `CandidateClusterSet`/agreement and optional-score input contracts.
- Generic and Label-Pipeline HTTP protocols overlap instead of sharing one detection contract.
  Remote endpoints are currently accepted without explicit opt-in and response size is not
  consistently bounded.
- Settings returns one workspace VLM binding rather than arbitrary worker CRUD/discovery/testing.
- Advisor currently creates classification or a YOLO-named bounding-box recipe; it has no
  capability-driven cold-start/specialist/fallback strategy.
- Runtime supports static fallback branches but no evidence-aware Recovery Agent or per-image
  open-vocabulary budget.
- Results/Review do not expose independent detector evidence, missing-score language, agreement,
  or choose-a-source-box actions. TUI has no Models panel or `/models` commands.
- No LocateAnything/RF-DETR worker, mock scenarios, hybrid examples, or model-specific protocol
  tests exist.

## Release Blocking remaining

The matrix contains 78 `PASS`, 10 `OPEN`, and one `LIVE-CONDITIONAL` row after M8. Capability-bound
RoboCup hybrid policy, Correction Memory, hard-negative and Crop verification now pass;
cache-specific proof and Guided Results/Review remain scheduled work. Real five-image GPU smokes remain explicitly
live-conditional and are not represented by Mock fixtures.

## Live-conditional items

- LocateAnything-3B real smoke: no NVIDIA GPU or configured weights in the current Darwin arm64
  environment; official model terms limit use to non-commercial research/evaluation.
- RF-DETR real smoke: no configured checkpoint or training dataset version/hash; no weight will be
  downloaded automatically.
- Native browser 200% zoom remains manual; responsive browser automation is separate evidence.

## Real blockers

None for Mock, protocol, Core, Runtime, Storage, Web, TUI, or documentation work. External model
execution is live-conditional and cannot be represented as a passing offline result.
