# Expert Vision Decisions

Updated: 2026-09-01

## D1 — Extend existing registries and protocol

The implementation will extend `ModelProfile`, `VisionModelDescriptor`, the current HTTP Vision
protocol and the current Pipeline Runtime. It will not create a parallel registry or second
inference transport.

## D2 — Model Profiles are selectable; connections are infrastructure

Provider and Worker endpoints remain infrastructure configuration. Workflows and the Builder bind
only immutable Model Profile identities/revisions and may not read credentials or change endpoints.

## D3 — Capabilities and typed contracts drive execution

Core and Runtime may branch on capability, Artifact contract and node configuration. They may not
branch on SAM, YOLO, RF-DETR, LocateAnything, PIDNet, Grounding DINO, Qwen or RoboCup identities.

## D4 — Prompted segmentation is an explicit chain

The new SAM-compatible path will be visible as DetectionSet → BoxPromptSet → MaskSet →
DetectionSet. A Worker returns masks. Mask-to-bbox remains a deterministic, inspectable Core node.
The legacy RoboCup refiner will be a compatibility path, not the public authoring model.

## D5 — Confidence and geometry are independent

Detection score semantics remain distinct from geometry semantics. VLM detection starts as
`CoarseHypothesis`; even a high semantic score does not imply a calibrated/tight box.

## D6 — Availability is a gate, not a label

`Available` requires configured identity, compatible protocol/contracts, health and a passing
sample conversion. Missing weights, unconfigured, disabled, unknown, unreachable, incompatible
and failed-smoke states remain explicit and non-selectable for publishable Drafts.

## D7 — Evidence controls Advisor revisions

The Builder will diagnose ProviderFailure, NoCandidate, SemanticError, GeometryError,
MissingScore, DomainRisk and related failures from structured Runtime/Dry Run data. Prompted
segmentation is legal only for geometry evidence with promptable candidates and an available
model.

## D8 — Offline tests prove contracts, not live accuracy

Mock workers and deterministic fixtures may prove registration, contract validation, typed
execution and Agent policy. They may not be described as evidence that a real checkpoint ran or
achieved an accuracy level.
