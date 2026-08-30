# Detection Backends Status

Updated: 2026-08-30

## Current Milestone

M2 — Detection Artifact and Evidence (complete)

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

## In progress

- None. M2 is ready for its independent local commit.

## Next step

M3 — converge detection workers on one versioned health/capability/infer protocol with loopback
policy, response limits, cancellation, malformed-response handling, and contract tests.

## Latest tests

| Area | Command | Result |
| --- | --- | --- |
| Rust | `cargo test --workspace --all-features` | PASS — 178 tests, 0 failed; doc tests pass |
| Rust lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Web | `npm --prefix web test -- --run` | PASS — 12 files, 33 tests |
| Web types | `npm --prefix web run typecheck` | PASS |
| Worker protocol | compile both tracked Python workers with Python 3.14 | PASS — syntax only; no model loaded |
| Browser | prior Guided Release browser evidence | PASS at 1024px and 720×450; new mixed-detector UI not implemented |

## Latest local commit

This document's containing M2 commit: `feat(core): preserve detection evidence and score semantics`

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

The matrix contains 30 `PASS`, 58 `OPEN`, and one `LIVE-CONDITIONAL` row after M2.

## Live-conditional items

- LocateAnything-3B real smoke: no NVIDIA GPU or configured weights in the current Darwin arm64
  environment; official model terms limit use to non-commercial research/evaluation.
- RF-DETR real smoke: no configured checkpoint or training dataset version/hash; no weight will be
  downloaded automatically.
- Native browser 200% zoom remains manual; responsive browser automation is separate evidence.

## Real blockers

None for Mock, protocol, Core, Runtime, Storage, Web, TUI, or documentation work. External model
execution is live-conditional and cannot be represented as a passing offline result.
