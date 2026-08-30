# Detection Backends Acceptance Matrix

Updated: 2026-08-30

Status vocabulary: `PASS` has executable repository evidence; `OPEN` requires implementation;
`LIVE-CONDITIONAL` needs an external legal model/runtime; `MANUAL` cannot be truthfully automated
in the current browser environment. Existing platform behavior is marked PASS only when the M0
baseline directly exercised it.

## M1 foundation evidence

The following prerequisites are implemented and tested but do not replace the model-specific rows
below:

- `OpenVocabularyDetection`, `PhraseGrounding`, and `ObjectDetection` are distinct capabilities.
- Model Descriptor preserves backend kind/protocol/endpoint, architecture/version/checkpoint,
  training data, input/output contracts, score semantics, runtime requirements, label space,
  license, availability, health, limits, pricing, and secret reference.
- Registry validation rejects capability/backend mismatches, duplicate capabilities, inconsistent
  legacy/structured contracts, invalid checkpoint hashes, invalid verified-license sources, and
  disabled execution.
- Old descriptor JSON and the former `http_json` backend spelling migrate deterministically.
- `cargo test --workspace --all-features` passes 170 tests, including four new Model Registry
  migration/validation tests; strict workspace Clippy, Web typecheck, and all 32 Web tests pass.

## M2 Artifact and Evidence evidence

- Detection schema v2 carries optional score plus explicit semantics, source model/capability,
  query/model/Project label separation, independent evidence, and controlled raw-payload refs.
- Historical `id`/`class_id`/`label`/`rect`/`confidence` JSON migrates when a persisted checkpoint
  is read. Missing scores become `NotProvided`; future unsupported versions are rejected.
- Candidate Clusters serialize each model contribution independently and their Artifact Envelope
  retains both source DetectionSets as parents. No score averaging exists.
- Ordinary Confidence Gate cannot compare absent, ranking-only, or unknown scores; these route to
  Review, while Filter retains them for evidence-aware handling.
- `cargo test --workspace --all-features` passes 178 tests, strict workspace Clippy passes, Web
  typecheck passes, and all 33 Web tests pass.

## M3 Worker protocol evidence

- Detection Worker v1 has strict health, capabilities, infer, structured error, and cancel DTOs;
  the shared protocol version is the same constant used by Pipeline Model requests.
- The generic adapter validates model/capability/request scope, optional score semantics, query and
  target-label declarations, unique identities, finite normalized geometry, and valid empty sets
  before creating a Core Artifact.
- The shared HTTP boundary is loopback-only by default. Remote Workers require explicit opt-in and
  HTTPS; URLs with credentials/query/fragment, redirects, oversized requests/responses, excessive
  retry counts, zero timeouts, and malformed bodies fail closed.
- Contract tests cover capability spoofing, model/version mismatch, unknown local paths, duplicate
  IDs, undeclared labels, NaN/out-of-bounds/reversed boxes, oversized bodies, redirect credential
  isolation, distinct timeout/cancellation errors, and forwarding a bounded cancel request.
- Raw image/response bytes are not logged or persisted. Accepted raw responses retain only a
  controlled SHA-256/size reference in Detection evidence.
- `cargo test --workspace --all-features` passes 188 tests, strict workspace Clippy and build pass,
  Web typecheck/build pass, and all 33 Web tests pass. Model-specific C/D rows remain OPEN until M4
  and M5 provide real Worker implementations rather than inferring acceptance from the transport.

## M4 LocateAnything grounding evidence

- `annotagent.open_vocabulary_grounding` owns registry-bounded Open Vocabulary Detection and Phrase
  Grounding nodes; neither model identity nor grounding query semantics were added to Core node kinds.
- The Mock backend covers multiple queries, Query-ID-to-Project-Label mapping, score-less results,
  valid empty DetectionSets, both capabilities, and explicit rejection of visual prompts.
- The tracked Python adapter uses the official LocateAnything `detect`, `ground_multi`, and
  `parse_boxes` interfaces, accepts no host file path, normalizes coordinates, implements health,
  discovery, inference and cooperative cancellation, and loads only explicitly configured local
  code/model directories.
- The HTTP contract test proves multiple queries, empty results, normalized geometry, optional
  score preservation, capability discovery, and no fabricated confidence through a live local
  Worker fixture. Generic timeout/cancel tests exercise the same final adapter/client.
- The Generic Project integration test performs Draft, Dry Run, publish and exact-version offline
  execution and reads the persisted score-less DetectionSet from the Run checkpoint.
- An actual no-weight Worker process returned truthful unavailable health and discoverable
  capabilities. Real LocateAnything model inference remains `LIVE-CONDITIONAL`.
- `cargo test --workspace --all-features` passes 196 tests; strict Clippy/build, all 33 Web tests,
  Web typecheck/build, and Python syntax validation pass.

## A. Architecture

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| A01 | LocateAnything is not a Core node type | PASS | no such type exists; M4 must preserve this |
| A02 | RF-DETR is not a Core node type | PASS | no such type exists; M5 must preserve this |
| A03 | YOLO and RF-DETR share Object Detection Capability | OPEN | YOLO uses it; RF-DETR absent |
| A04 | LocateAnything implements open-vocabulary Capability | PASS | Capability Skill + Worker adapter implement OpenVocabularyDetection and PhraseGrounding |
| A05 | `robocup.ball` references no concrete model ID | PASS | current templates are model-agnostic; rescan at M8/M10 |
| A06 | Generic Project can use LocateAnything | PASS | generic template integration test publishes and runs without RoboCup |
| A07 | Generic Project can use RF-DETR | OPEN | backend/template absent |
| A08 | Core contains no model-brand branch | PASS | M0 scan; enforce each Milestone |

## B. Detection Artifact

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| B01 | Detection score supports `None` | PASS | schema v2 + missing-score serialization/storage tests |
| B02 | Score semantics are persisted | PASS | typed semantics round-trip in Artifact/checkpoint JSON |
| B03 | LocateAnything confidence is never fabricated | PASS | Mock, Worker and contract test preserve `NotProvided`/`None` |
| B04 | Every candidate stores source model | PASS | Detection validation requires source model and matching evidence |
| B05 | Every candidate stores query or model label | PASS | validation rejects Detection/Evidence with neither identity |
| B06 | Multi-model candidates retain independent evidence | PASS | CandidateCluster round-trip preserves scored and score-less members |
| B07 | Artifact lineage is traceable | PASS | cluster envelope test retains both source DetectionSet parents |
| B08 | Valid empty DetectionSet is not failure | PASS | empty Pipeline Artifact validates; new workers need tests |

## C. LocateAnything

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| C01 | Health can be checked | PASS | no-weight Python Worker live probe + Models Test Worker action |
| C02 | Capabilities can be discovered | PASS | Python Worker and Rust contract return both implemented capabilities |
| C03 | Open-vocabulary detection works | PASS | Mock/runtime and live local contract fixture return typed DetectionSet |
| C04 | Phrase grounding works | PASS | same Skill/adapter path is exercised with PhraseGrounding capability |
| C05 | Multiple queries work | PASS | Mock and HTTP contract tests preserve both query identities/mappings |
| C06 | No-object result works | PASS | Mock and HTTP Worker tests accept a valid empty DetectionSet |
| C07 | Coordinates normalize correctly | PASS | Worker converts parsed pixels; Rust contract validates normalized xyxy→xywh |
| C08 | Missing score remains `None` | PASS | Worker sends null/NotProvided and Core Artifact retains it |
| C09 | Unsupported visual prompt is blocked | PASS | Worker reports false, UI disables action, both Workflow validators block it |
| C10 | Timeout and cancellation work | PASS | shared final adapter/client contract test + Worker cooperative cancel loop |
| C11 | Model license metadata is visible | PASS | restricted official-source metadata is exposed on Models page |

## D. RF-DETR

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| D01 | Health can be checked | OPEN | worker absent |
| D02 | Capabilities can be discovered | OPEN | worker absent |
| D03 | Object detection works | OPEN | worker absent |
| D04 | Label space is reported and validated | OPEN | descriptor field absent |
| D05 | Class mapping works | OPEN | generic options exist but RF adapter absent |
| D06 | Real finite score is preserved | OPEN | adapter absent |
| D07 | Confidence threshold is supported | OPEN | adapter absent |
| D08 | Checkpoint SHA-256 is saved | OPEN | descriptor supports it; RF-DETR adapter absent |
| D09 | Training dataset version is saved | OPEN | descriptor supports it; RF-DETR adapter absent |
| D10 | Timeout and cancellation work | OPEN | model-specific contract absent |
| D11 | Model license metadata is visible | OPEN | Registry/UI metadata absent |

## E. Multi-model evidence

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| E01 | IoU matching | OPEN | node absent |
| E02 | Unmatched candidates retained | OPEN | node absent |
| E03 | Geometry conflict | OPEN | typed state exists; Candidate Match behavior absent |
| E04 | Label conflict | OPEN | typed state exists; Candidate Match behavior absent |
| E05 | Incomparable scores are not averaged | PASS | Cluster and fan-in tests retain/omit incomparable values without blending |
| E06 | Evidence Gate emits explainable reason | OPEN | node absent |
| E07 | High specialist score can skip fallback | OPEN | recovery absent |
| E08 | Empty specialist result can trigger fallback | OPEN | recovery absent |
| E09 | Domain risk can trigger fallback | OPEN | recovery absent |
| E10 | Insufficient budget routes to Review | OPEN | mixed-detection budget absent |

## F. Agent

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| F01 | Advisor recommends cold-start Pipeline | OPEN | no open-vocabulary strategy |
| F02 | Advisor recommends specialist-first Pipeline | OPEN | no specialist registry strategy |
| F03 | Advisor output is Draft-only | PASS | existing constrained Advisor never publishes |
| F04 | Recovery chooses fallback from evidence | OPEN | absent |
| F05 | Recovery changes decision after fallback | OPEN | absent |
| F06 | Agent has tools, budget, and stop conditions | PASS | bounded Agent Runtime baseline; mixed policy open |
| F07 | Trace explains fallback invocation | OPEN | absent |
| F08 | Trace exposes no hidden chain-of-thought | PASS | structured visible action/event baseline |

## G. Cache and Replay

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| G01 | Identical RF-DETR input executes once | OPEN | backend/cache key absent |
| G02 | Identical LocateAnything query executes once | OPEN | backend/cache key absent |
| G03 | Gate-only edit does not rerun detectors | OPEN | detector-specific proof absent |
| G04 | Query edit invalidates only LocateAnything | OPEN | query-aware cache absent |
| G05 | Cache key includes model version and config | OPEN | existing node cache lacks required detection fields |
| G06 | Replay preserves lineage | PASS | generic checkpoint Replay baseline; mixed evidence proof open |
| G07 | Replay does not duplicate Annotation commit | PASS | generic DAG Replay baseline; mixed path proof open |

## H. Product

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| H01 | Global brand remains AnnotAgent | PASS | product identity tests/baseline |
| H02 | Guided Mode uses user language | PASS | current Guided Workspace; new recommendations open |
| H03 | Expert Mode shows real model and evidence | OPEN | mixed evidence DTO/UI absent |
| H04 | Results shows source model | OPEN | mixed source evidence absent |
| H05 | Missing score says confidence not provided | OPEN | score cannot be absent |
| H06 | Agreement is visible | OPEN | clusters absent |
| H07 | Review explains why it was queued | OPEN | generic reasons exist; mixed reasons absent |
| H08 | Review can choose either model box | OPEN | source choices absent |
| H09 | Settings can test a Worker | PASS | Models `Test Worker` invokes real health/capability discovery endpoint |
| H10 | Unavailable Worker does not block AnnotAgent startup | PASS | disabled default descriptor + offline startup/catalog integration test |

## I. RoboCup

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| I01 | `robocup.ball` uses Capability Binding | OPEN | model-agnostic today, hybrid bindings absent |
| I02 | White-shoe risk triggers fallback or Crop Verify | OPEN | current validator/recovery exists; hybrid path absent |
| I03 | Penalty-mark risk never directly auto-accepts | PASS | current deterministic Ball validator test |
| I04 | Correction Memory affects Recovery | PASS | current Application/Skill tests; hybrid policy open |
| I05 | Generic Project does not load RoboCup | PASS | existing generic project/runtime tests |
| I06 | RoboCup appears only for enabled Skill | PASS | layered registry and browser baseline |

## J. Course/product requirements

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| J01 | Rust Runtime owns the Agent loop | PASS | architecture and workspace tests |
| J02 | TUI can inspect and cancel | PASS | current commands/tests; Models commands open |
| J03 | GUI can inspect and cancel | PASS | current Run Debug/control E2E |
| J04 | Models, endpoints, and cost are configurable | OPEN | arbitrary worker collection absent |
| J05 | Real-time progress is visible | PASS | SSE/TUI event baseline |
| J06 | Run history and Artifacts are inspectable | PASS | SQLite/API/Run Debug baseline |
| J07 | Every model call records usage and latency | OPEN | token/cost exists; worker call timing contract incomplete |
| J08 | RoboCup customization is real | PASS | validators/refiners/recovery/correction tests |
| J09 | Mock demo needs no key | PASS | three offline demos and acceptance baseline |
| J10 | Live smoke executes or is explicitly conditional | LIVE-CONDITIONAL | no GPU/weights configured; exact blockers recorded |

## Browser release checks

All twenty mixed-detection browser scenarios remain `OPEN` until M9/M10. Native 200% zoom is
`MANUAL`; 1024px/reflow checks will be automated independently and cannot substitute for it.
