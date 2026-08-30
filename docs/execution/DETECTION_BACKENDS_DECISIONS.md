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

## DB-010 — Registry migration is additive and normalized at registration

Published Workflow snapshots and API payloads already serialize `VisionModelDescriptor`, so M1
extends that existing type and exposes `ModelDescriptor` as the preferred domain-neutral name
rather than creating a competing registry. New fields use serde defaults. Registration copies old
`model_version`, input/output, endpoint, backend kind, and health facts into structured fields and
then validates one canonical representation. The serialized `http_json` backend spelling is read
as `HttpVision` and new snapshots emit `http_vision`. No SQL migration is required because Model
Descriptors are frozen inside existing JSON Workflow/Run snapshots rather than a relational model
table.

An explicitly Disabled, Misconfigured, IncompatibleProtocol, or MissingWeights descriptor remains
visible in the Registry but cannot be resolved for execution. Unreachable is retained as a health
fact for later Recovery policy rather than silently deleting the configured model.

## DB-011 — Detection Artifact v2 migrates at the typed JSON boundary

Pipeline Artifacts already live inside durable JSON checkpoints, so M2 uses an explicit
`DetectionSetArtifact` schema version and custom deserializer rather than adding an unrelated SQL
column. Historical field names are accepted once, source model/evidence is reconstructed from the
historical set binding, and current serialization emits only v2 names. Missing legacy confidence
becomes `NotProvided`; a historical numeric confidence becomes `Unknown` because its calibration
semantics were never recorded. Unsupported future versions are rejected rather than guessed.

Filtering, mapping, refinement, and Review may create derived DetectionSet identities while source
evidence continues to point to the original detector Artifact. Validation therefore requires the
primary source model to appear in evidence but does not rewrite evidence to claim a transform was
the detector.

## DB-012 — Ordinary confidence is narrower than a displayed detector score

Only `CalibratedProbability` and `RelativeConfidence` are eligible for ordinary threshold gates or
the legacy Annotation confidence field. `RankingScore`, `Unknown`, and `NotProvided` remain visible
inside the evidence-aware Artifact but cannot silently become a percentage, an auto-accept value,
or a detection/classification combined confidence. Score-less filtering preserves candidates for
Evidence Gate, another model, deterministic validation, or Human Review.

## DB-013 — Detection Worker v1 is a strict numeric-version envelope

Health, capabilities, infer, error, and cancel use one numeric protocol version sourced from Core.
Every inference response is checked against the request ID, model ID, capability, and declared
query/target-label scope before an error or Detection is attributed to the active Run. Unknown
wire fields fail closed so filesystem paths or future semantics cannot silently cross the boundary.

## DB-014 — Worker HTTP is an untrusted local boundary by default

Generic Worker URLs accept only HTTP(S), no embedded credentials, query, or fragment. Loopback is
the default trust scope; remote access is an explicit setting and requires HTTPS. Redirects are
never followed, bodies/retries/timeouts are bounded, and remote payload text is not copied into
errors. This policy also applies to the older generic Vision/Pipeline HTTP clients.

## DB-015 — Raw Worker payloads use controlled references

Inference sends bounded inline PNG/JPEG bytes rather than a host path. Successful raw response
evidence stores only media type, SHA-256, and byte size, never the JSON body or image. Cancellation
is cooperative through `/v1/cancel`; Runtime cancellation remains authoritative even if the
best-effort Worker acknowledgement fails.

## DB-016 — Open-vocabulary grounding is a Capability Skill, not a model node

`annotagent.open_vocabulary_grounding` owns the text-query contract, JSON Schema, Mock behavior,
Workflow template, and two capability-specific operations. LocateAnything is one registry backend
for those operations. Generic Workflow validation reasons only about capability and input contract;
it never branches on the Skill or model ID.

Rejected: `NodeKind::LocateAnything`, model-branded Core DTOs, or allowing a backend-specific
request shape to bypass the typed DetectionSet boundary.

## DB-017 — The tracked LocateAnything adapter is local-install only

The Python Worker loads an explicitly configured local model directory plus the official NVIDIA
worker source. It does not clone code, download a checkpoint, accept arbitrary image paths, or turn
missing files into fixture inference. Without both paths it remains reachable for health and
capability discovery and reports `unavailable`; Mock inference stays visibly separate.

The implementation was checked against NVIDIA's official `LocateAnything-3B` model card and the
official `NVlabs/Eagle/Embodied/locateanything_worker.py` interfaces (`detect`, `ground_multi`, and
`parse_boxes`). The registered model metadata retains the official non-commercial
research/evaluation restriction as informational metadata rather than a legal conclusion.

## DB-018 — Unsupported visual prompting fails before execution

Visual exemplar prompting is an explicit input-contract boolean discovered from the Worker. The
current LocateAnything profile reports false, the Models UI disables the action with a concrete
reason, and both flat Workflow and Label Pipeline static validators reject visual-prompt parameters.
The Worker also fails closed if such a request bypasses UI validation.

Rejected: an enabled decorative control that fails only after starting a Run, or frontend-authored
capability claims that disagree with Worker discovery.

## DB-019 — Trained detection is a generic Capability Skill

`annotagent.object_detection` owns the backend-neutral Object Detection request, options, strict
schema, Model-to-Project class mapping, post-processing and Review template. RF-DETR and legacy
YOLO integrations are Model Registry backends for that capability, not Skill identities or Core
node kinds. Crop remains a Core transform and is never advertised as detector behavior.

The product registry exposes this generic Skill for new Workflows. Existing YOLO operation and
descriptors remain readable for compatibility while later template migration removes new product
dependence on them.

## DB-020 — Specialist Workers require immutable identity before enablement

A disabled Worker profile may be incomplete so AnnotAgent can start and explain setup. Enabling a
versioned specialist Worker requires architecture, model version, checkpoint SHA-256, training
dataset version, non-empty label space and concrete weight-license metadata. Existing Settings are
merged additively with new curated profiles; user-edited profiles are never overwritten by a
default migration.

Rejected: inferring metadata from a checkpoint filename, marking an unknown license as permissive,
or hiding an unavailable Worker from Models.

## DB-021 — RF-DETR uses an explicit local official checkpoint path

The tracked adapter follows RF-DETR's official `from_checkpoint` and `predict` APIs with safe
checkpoint loading. It accepts only an explicitly configured existing local checkpoint whose
bytes match the configured SHA-256 and never installs packages or downloads weights. It reports
only Object Detection and relative-confidence semantics; segmentation, keypoints, training and
batch inference are not claimed.

RF-DETR licensing is recorded per concrete model variant because the official repository
distinguishes Apache-designated artifacts from PML-licensed detection variants. A real GPU run is
live-conditional until the exact checkpoint and its applicable terms are configured.

## DB-022 — Specialist label space is discovered and enforced exactly

The configured expected label space is compared as a set with Worker capability discovery before
inference. Every returned model label is then checked against that same set before it becomes a
typed Detection Artifact. Project label mapping is a separate Skill concern and cannot expand the
model's declared vocabulary.

Rejected: trusting only a frontend label list, accepting undeclared response labels, or silently
mapping an unknown class to a Project Label.

## DB-023 — Candidate matching uses Project Labels and deterministic geometry

`core.match_detection_sets` accepts two same-image DetectionSets and performs stable one-to-one
matching. Sufficient IoU between equal Project Labels is agreement; overlapping equal labels below
the threshold are Geometry Conflict; overlapping different Project Labels are Label Conflict.
Unmatched candidates remain SingleSource when configured. Model-native labels must be mapped before
this node because only Project Labels define annotation semantics.

Representative geometry is selected deterministically from one source contribution. All member
boxes remain inspectable, and no score or rectangle is averaged into fabricated evidence.

## DB-024 — Evidence Gate is a fact-based four-route policy

`core.evidence_gate` evaluates typed Candidate Clusters, upstream Validation Issues and optional
Correction Risk against explicit rule lists. Precedence is explicit Reject, Fallback, conflict
safety Review, configured Review, Accept, then safe-default Review. It emits one `accept`,
`fallback`, `review`, or `reject` route for the image and a structured report with stable reason
codes, messages, source IDs and metrics.

Only calibrated probability and relative confidence satisfy numeric score rules. Ranking, Unknown
and NotProvided scores are incomparable. Rejected: filling a missing score, averaging two model
scores, or using model brands as Core policy branches.

## DB-025 — Validator facts travel as DAG metadata

Active upstream node metadata is keyed by source node and supplied to deterministic runners.
Candidate Match propagates Validation Issues and Correction Risk; Evidence Gate validates and
consumes them. This keeps domain issues attached to the real execution graph without turning them
into fake annotation Artifacts or copying frontend-authored state into Runtime decisions.

The persisted `evidence_gate` report is observable decision evidence, not chain-of-thought. Run
inspection exposes output metadata and selected route; the UI renders only those structured facts.

## DB-026 — Multi-source Annotations have no aggregate confidence

An accepted/reviewed Candidate Cluster may project its deterministic representative rectangle into
an Annotation. A single-source candidate may carry that source's comparable confidence. A
multi-source candidate records all source model IDs but leaves Annotation confidence absent; the
complete Candidate Cluster remains the authoritative evidence Artifact.
