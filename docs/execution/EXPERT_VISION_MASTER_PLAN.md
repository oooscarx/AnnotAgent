# Expert Vision Integration Master Plan

Updated: 2026-09-01

## Outcome

AnnotAgent will connect provider-backed LLM/VLM models and worker-backed expert vision models
through one capability- and contract-driven Model Profile boundary. Core nodes remain model-brand
neutral. The Pipeline Builder may select only an available compatible profile, must construct a
typed Artifact path, and must use structured Dry Run evidence before proposing a specialist or
refiner.

## Fixed architecture

1. Provider Registry owns LLM/VLM API connectivity and credentials.
2. Expert Vision Worker Registry owns HTTP worker endpoints, discovery, model identity, health,
   weights, contracts and smoke-test state.
3. Model Profiles are the only model identities selectable by workflows and the Builder.
4. Capability Skills describe detection, classification and segmentation behavior.
5. Core nodes prepare images, convert Artifacts, validate evidence, decide, review and commit.
6. Domain Skills contribute knowledge and policies, never model brands.
7. Pipeline Builder inspects, constructs, validates, Dry Runs, diagnoses, revises and submits.
8. Deterministic Runtime executes only a frozen published Workflow Version.

## Milestones

### M0 — Baseline and inventory

- Audit Registry, Worker adapters, protocol, SAM/detection backends, Artifacts, nodes, Advisor,
  Dry Run and tests.
- Establish the execution ledgers and exact master prompt.
- Record capability states separately: adapter, worker, process, weights, health, smoke,
  registered path and Builder selection.

### M1 — Expert Model Manifest

- Add worker-backed `ModelConnection` alongside provider-backed and Mock connections.
- Add versioned Expert Model Manifest, Artifact/Prompt contracts, geometry semantics and complete
  availability states.
- Validate manifests and migrate existing descriptor facts without brand branches.

### M2 — Worker SDK and protocol

- Extend the existing versioned HTTP protocol with model/contract discovery, cancellation and
  optional warmup while retaining compatibility.
- Add a Python SDK, conformance suite, example worker and scaffold CLI/presets.
- Enforce bounded images, responses, masks, coordinates and identities at the Rust boundary.

### M3 — Typed Artifact conversions

- Add Box/Point Prompt and Mask Set Artifacts.
- Add a conversion registry and path finder.
- Implement explicit Detection → Box Prompt → Prompted Segmentation → Mask → BBox nodes.
- Preserve parent references, source geometry and auditable intermediate Artifacts.

### M4 — Quality and diagnosis

- Add structured failure classes and geometry semantics.
- Add per-artifact geometry reports and aggregate Dry Run metrics.
- Incorporate human adjustments and refiner use/success/fallback evidence.

### M5 — Existing backend migration

- Represent SAM, YOLO, RF-DETR, LocateAnything, PIDNet and Grounding DINO by capability and
  contract.
- Keep old Workflow reads compatible while emitting the new public chain.
- Never claim real availability without health, identity, contract and sample conversion evidence.

### M6 — Evidence-driven Pipeline Builder

- Add constrained model/worker/contract/conversion/quality inspection tools.
- Teach the Builder to separate provider, no-candidate, semantic and geometry failures.
- Allow prompted segmentation only for an existing promptable candidate and a healthy compatible
  profile; validate and Dry Run every proposed revision.

### M7 — Expert Model setup UX

- Add Settings → Vision Workers onboarding for presets, generic HTTP and Mock.
- Discover live facts, collect immutable identity, run an explicit sample smoke test and register
  only when the availability gate passes.

### M8 — RoboCup and release

- Keep `robocup.ball` capability-only.
- Exercise specialist-first, open-vocabulary fallback, semantic verification and conditional
  geometry refinement.
- Close the offline release matrix and identify true GPU/weights/provider items as
  live-conditional.

## Commit discipline

Each milestone updates status and acceptance evidence, runs proportionate Rust/Web/Worker tests,
fixes regressions, and receives its own local commit. No push, remote mutation, model download,
weight commit, API-key use, reset, rebase or amend is permitted.
