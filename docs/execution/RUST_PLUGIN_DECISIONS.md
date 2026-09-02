# Rust Expert Model Plugin Alpha — Decisions

## D001 — Process boundary

Official plugins are separate Rust executables. The host extends the existing loopback HTTP Vision
Protocol instead of loading a Rust dynamic library or defining another inference wire format.

## D002 — Core remains capability-oriented

Plugin API may reuse domain-neutral Core capabilities and typed artifacts. Core receives a generic
plugin-backed model identity; model brands stay in plugin packages and product metadata.

## D003 — Truthful model readiness

`Ready` requires runtime discovery, contract parity, a configured immutable checkpoint identity and
a passed smoke/conformance test. Contract-only packages remain `NeedsWeights`, `UnsupportedPlatform`
or `FailedSmokeTest`; no fixture promotes a production model.

## D004 — Alpha isolation claim

The Alpha promises process isolation, loopback/token authentication, environment and filesystem
minimization, bounded logs/responses, cancellation and crash containment. It does not claim an OS
security sandbox on every target.

## D005 — Deterministic packages

`.annotplugin` uses a deterministic ZIP profile: sorted paths, fixed timestamps, normalized modes,
manifest/checksum validation and traversal rejection. Large weights are provisioned separately.

## D006 — Shared runtime stays model-neutral

Common image/geometry operations and native ONNX session mechanics are reusable crates. Tensor-name
selection, model-family preprocessing/postprocessing, class semantics and Artifact construction
remain in each plugin. Explicit accelerators fail setup if unavailable; the Registry must not
describe an unverified CPU fallback as CUDA or TensorRT.

## D007 — YOLO is a family, not one implicit tensor shape

The first official detector declares the exact YOLOX Nano 416×416 COCO-80 input, output and
postprocessing contract. Another YOLO export is rejected unless a distinct model contract and
version describe its preprocessing, heads, label space and score semantics. Core still sees only
ObjectDetection and DetectionSet.

## D008 — Multi-file weights are named components

A model may declare zero or more named weight components. Zero preserves the v1 single-file
contract. A declared component fixes model ownership and controlled filename; provisioning records
the original filename and exact digest. Every component is required before setup, and publication
freezes a deterministic ordered aggregate digest. A partial SAM encoder/decoder set is not an
installed runnable checkpoint.

## D009 — Prompted and semantic masks are different Artifacts

Prompted segmentation produces a `MaskSet` whose items retain prompt parents and independent
scores. Semantic segmentation produces a dense `SemanticMask` with one lossless model class ID per
source pixel and optional Project mapping. Conflating them would lose either prompt lineage or
multiclass semantics and is rejected.

## D010 — Contract-complete is not Ready

SAM and PIDNet include real native ONNX execution code and opt-in process tests, but no accepted
checkpoint is committed. Offline tensor tests establish deterministic implementation behavior;
only a user-provisioned, hash-bound sample inference can produce smoke evidence and `Ready` status.

## D011 — Implementation status is immutable package truth

Manifest `implementation_status` is part of the package digest. `Runnable` preserves existing
behavior; `LiveConditional` allows a complete Rust path to become Ready only after real external
evidence; `Unsupported` is installed disabled as `UnsupportedPlatform` and cannot be promoted by a
fixture, enable action or fabricated smoke report.

## D012 — RF-DETR follows the official exported postprocessor

The RF-DETR plugin reads the exact fixed NCHW input and `dets`/`labels` outputs documented by the
official exporter, performs antialias-free half-pixel resize, per-class sigmoid and flattened top-k,
and does not add NMS because the official model-specific postprocessor does not use it. Dataset
version is a required frozen node parameter; checkpoint identity stays Registry-owned.

## D013 — LocateAnything remains unsupported without a verified Rust runtime

The official 2026 release uses a custom MoonViT/Qwen/PBD Python inference stack and publishes no
complete ONNX, Candle, Burn or stable Rust runtime. A third-party C++ port is not silently adopted.
The production package fails closed; a separately named Rust fixture proves only protocol transport
and is excluded from the `.annotplugin` package and readiness evidence.
