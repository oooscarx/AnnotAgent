# Detection Backends Acceptance Matrix

Updated: 2026-08-30

Status vocabulary: `PASS` has executable repository evidence; `OPEN` requires implementation;
`LIVE-CONDITIONAL` needs an external legal model/runtime; `MANUAL` cannot be truthfully automated
in the current browser environment. Existing platform behavior is marked PASS only when the M0
baseline directly exercised it.

## A. Architecture

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| A01 | LocateAnything is not a Core node type | PASS | no such type exists; M4 must preserve this |
| A02 | RF-DETR is not a Core node type | PASS | no such type exists; M5 must preserve this |
| A03 | YOLO and RF-DETR share Object Detection Capability | OPEN | YOLO uses it; RF-DETR absent |
| A04 | LocateAnything implements open-vocabulary Capability | OPEN | capability/backend absent |
| A05 | `robocup.ball` references no concrete model ID | PASS | current templates are model-agnostic; rescan at M8/M10 |
| A06 | Generic Project can use LocateAnything | OPEN | backend/template absent |
| A07 | Generic Project can use RF-DETR | OPEN | backend/template absent |
| A08 | Core contains no model-brand branch | PASS | M0 scan; enforce each Milestone |

## B. Detection Artifact

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| B01 | Detection score supports `None` | OPEN | mandatory `f32` today |
| B02 | Score semantics are persisted | OPEN | type absent |
| B03 | LocateAnything confidence is never fabricated | OPEN | backend absent |
| B04 | Every candidate stores source model | OPEN | only set-level model binding today |
| B05 | Every candidate stores query or model label | OPEN | current detection has only class/Project label |
| B06 | Multi-model candidates retain independent evidence | OPEN | evidence type absent |
| B07 | Artifact lineage is traceable | PASS | typed refs/checkpoints/storage baseline; detection evidence extension open |
| B08 | Valid empty DetectionSet is not failure | PASS | empty Pipeline Artifact validates; new workers need tests |

## C. LocateAnything

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| C01 | Health can be checked | OPEN | LocateAnything worker absent |
| C02 | Capabilities can be discovered | OPEN | LocateAnything worker absent |
| C03 | Open-vocabulary detection works | OPEN | absent |
| C04 | Phrase grounding works | OPEN | absent |
| C05 | Multiple queries work | OPEN | absent |
| C06 | No-object result works | OPEN | absent |
| C07 | Coordinates normalize correctly | OPEN | absent |
| C08 | Missing score remains `None` | OPEN | Artifact cannot represent it |
| C09 | Unsupported visual prompt is blocked | OPEN | capability/validator absent |
| C10 | Timeout and cancellation work | OPEN | model-specific contract absent |
| C11 | Model license metadata is visible | OPEN | Registry/UI metadata absent |

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
| D08 | Checkpoint SHA-256 is saved | OPEN | descriptor field absent |
| D09 | Training dataset version is saved | OPEN | descriptor field absent |
| D10 | Timeout and cancellation work | OPEN | model-specific contract absent |
| D11 | Model license metadata is visible | OPEN | Registry/UI metadata absent |

## E. Multi-model evidence

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| E01 | IoU matching | OPEN | node absent |
| E02 | Unmatched candidates retained | OPEN | node absent |
| E03 | Geometry conflict | OPEN | type absent |
| E04 | Label conflict | OPEN | type absent |
| E05 | Incomparable scores are not averaged | OPEN | mixed evidence absent; policy recorded |
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
| H09 | Settings can test a Worker | OPEN | one workspace Provider setting only |
| H10 | Unavailable Worker does not block AnnotAgent startup | OPEN | must prove with registered workers |

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
