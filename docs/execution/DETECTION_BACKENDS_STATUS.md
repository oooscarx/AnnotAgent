# Detection Backends Status

Updated: 2026-08-30

## Current Milestone

M0 — Baseline and design ledger (complete)

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

## In progress

- None. M0 is ready for its independent local commit.

## Next step

M1 — add domain-neutral open-vocabulary/phrase grounding capabilities and a complete versioned
Model Descriptor contract.

## Latest tests

| Area | Command | Result |
| --- | --- | --- |
| Rust | `cargo test --workspace --all-features` | PASS — 166 tests, 0 failed; doc tests pass |
| Web | `npm --prefix web test -- --run` | PASS — 12 files, 32 tests |
| Worker protocol | compile both tracked Python workers with Python 3.14 | PASS — syntax only; no model loaded |
| Browser | prior Guided Release browser evidence | PASS at 1024px and 720×450; new mixed-detector UI not implemented |

## Latest local commit

This document's containing M0 commit: `docs: establish mixed detection backend baseline`

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

- `Detection.confidence` is mandatory `f32`; score absence and semantics cannot be represented.
- There are no OpenVocabularyDetection/PhraseGrounding capabilities, DetectionEvidence,
  CandidateClusterSet, Candidate Match, or Evidence Gate contracts.
- `VisionModelDescriptor` lacks structured version/checkpoint/training-data/protocol metadata,
  input/output contracts, score semantics, license metadata, runtime requirements, label space,
  explicit availability, and measured latency/device state.
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

The initial matrix contains 24 `PASS`, 64 `OPEN`, and one `LIVE-CONDITIONAL` row. Counts must be
recalculated after each Milestone from `DETECTION_BACKENDS_ACCEPTANCE.md`.

## Live-conditional items

- LocateAnything-3B real smoke: no NVIDIA GPU or configured weights in the current Darwin arm64
  environment; official model terms limit use to non-commercial research/evaluation.
- RF-DETR real smoke: no configured checkpoint or training dataset version/hash; no weight will be
  downloaded automatically.
- Native browser 200% zoom remains manual; responsive browser automation is separate evidence.

## Real blockers

None for Mock, protocol, Core, Runtime, Storage, Web, TUI, or documentation work. External model
execution is live-conditional and cannot be represented as a passing offline result.
