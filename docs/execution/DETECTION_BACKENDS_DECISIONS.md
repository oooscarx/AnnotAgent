# Detection Backends Decisions

Updated: 2026-08-30

## DB-001 — Capabilities are the scheduling boundary

Core and generic Runtime will add `OpenVocabularyDetection` and `PhraseGrounding` beside the
existing `ObjectDetection`. LocateAnything, RF-DETR, YOLO, and Qwen remain opaque Registry model
identities resolved through `ModelBinding`; model brands do not become node kinds.

## DB-002 — Unknown score is a first-class fact

Detection score becomes an optional finite value paired with explicit `ScoreSemantics`. No default
is substituted. Confidence Gate may only compare eligible scores; missing or incomparable scores
go through Evidence Gate, deterministic validation, another model, or Human Review.

## DB-003 — Evidence is preserved, not numerically blended

Candidate matching will cluster per-source DetectionEvidence by Project Label and geometry. It
will report single source, agreement, geometry conflict, or label conflict. The representative
geometry is not a fabricated confidence and all original boxes/scores remain addressable.

## DB-004 — One versioned HTTP detection contract

The existing generic Vision and Label-Pipeline HTTP adapters will converge on shared v1 health,
capability, infer, validation, error, limit, and cancellation types. Backend adapters own raw model
coordinate conversion; Core receives normalized rectangles only.

## DB-005 — Workers are loopback-only unless explicitly allowed

Worker registration accepts `127.0.0.1` and `localhost` by default. A distinct user-controlled
remote-worker option is required for any other host. Embedded credentials, arbitrary paths,
redirect-based host escapes, unbounded bodies, and capability mismatches are rejected.

## DB-006 — Persist compatibility before specialization

Version/checkpoint/license data belongs to Model Descriptors and immutable Workflow/Run snapshots.
Detection evidence belongs to typed Artifacts and checkpoints. JSON migrations must read older
Artifacts by explicit defaults; new persisted data must never infer a score that was absent.

## DB-007 — License metadata is per model version

Official sources checked on 2026-08-30:

- [NVIDIA LocateAnything-3B model card](https://huggingface.co/nvidia/LocateAnything-3B) and its
  [LICENSE](https://huggingface.co/nvidia/LocateAnything-3B/blob/main/LICENSE) describe the released
  model as non-commercial research/evaluation use with redistribution conditions. AnnotAgent will
  display `Restricted`, not transform this into legal advice.
- [Roboflow RF-DETR official repository](https://github.com/roboflow/rf-detr) and
  [LICENSE](https://github.com/roboflow/rf-detr/blob/develop/LICENSE) distinguish the Apache 2.0
  package/Apache-designated models from PML 1.0 Plus components and XL/2XL detection models.

Therefore no global “LocateAnything license” or “RF-DETR license” string is sufficient. Code and
weight licenses, source URL, permissions, notes, and verification state travel with each concrete
Model Descriptor.

## DB-008 — Real workers never download weights on server startup

Tracked Python workers load only an explicitly configured local checkpoint/model reference and
surface `MissingWeights`/unavailable health otherwise. Model installation remains an external,
user-authorized setup step and real smokes remain live-conditional until executed.

## DB-009 — Guided language and Expert evidence share product truth

Guided UI will describe user intent (“Find objects by description”, “Use trained detector”, “Check
uncertain results”). Expert/Debug views will expose capabilities, model/version/license, individual
evidence, score semantics, match metrics, decisions, cache state, and lineage from the same DTOs.
