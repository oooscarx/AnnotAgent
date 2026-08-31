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

## D9 — Preserve the provider-profile storage boundary during migration

Existing provider-backed `ModelProfile` records remain source- and SQLite-compatible and expose a
derived `ProviderModel` connection. Worker-backed expert profiles are registered through the
versioned Manifest in the existing Model Registry. Persistent Worker setup will migrate onto this
boundary in M5/M7 instead of introducing a fake Provider ID or weakening current credential and
foreign-key guarantees.

## D10 — Legacy model names stop at the Application compatibility boundary

Existing editable `sam_prompted_refiner` Drafts are recognized only by the Application migration
layer and expanded to generic conversion and capability nodes. Core, Runtime and the public node
catalog never branch on SAM, YOLO, RF-DETR or LocateAnything identities. The original node id is
retained as Mask-to-BBox so downstream references remain stable.

## D11 — Workspace configuration is not availability evidence

An HTTP endpoint and expected capability list are enough to construct an adapter and validate a
Manifest contract, but not enough to report a model as `Available`. Default profiles without an
immutable checkpoint identity report `MissingWeights`; a configured profile remains `Unknown`
until discovery, health and explicit sample conversion produce the remaining evidence in M7.

## D12 — Prompt policy is backed by executable Agent gates

The system prompt explains VLM geometry limits and failure-specific actions, but Prompt text is not
the sole control. The Builder receives typed availability/contracts/quality data; Expert Model
binding rejects every state except `Available`; and the prompted-segmentation guided revision checks
candidate, failure, geometry, conversion-path and model evidence before changing a Draft.

## D13 — Discovery and sample evidence are durable, secrets are not

The guided Worker setup persists only model configuration, an optional environment credential
reference, and active availability evidence. It never stores the resolved bearer secret. Four
discovery resources and one user-selected image conversion are required before registration; a
Server restart re-evaluates an unresolved credential reference and cannot keep a stale Worker
`Available`.

## D14 — Browser fixtures prove integration, never model quality

Release E2E uses one deterministic multi-model HTTP Worker to exercise the real protocol,
discovery, Settings persistence and Artifact conversion without weights or third-party runtimes.
The fixture identifies itself in manifests, response metadata and warnings and is excluded from
accuracy claims. Real-model quality remains live-conditional even when every integration test
passes.
