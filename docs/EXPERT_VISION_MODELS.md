# Expert Vision Models

AnnotAgent separates three identities:

- A Provider connects an LLM/VLM API and owns credentials, rate limits and token billing.
- A Vision Worker connects an expert inference process and owns endpoint, protocol, weights,
  accelerator, health and cancellation.
- A Model Profile/Expert Model Manifest is the immutable model identity that a Workflow may bind.

Core never adds an enum variant for a model brand. A Worker-backed model declares capabilities,
typed input/output and prompt contracts, score semantics, geometry semantics, label space,
checkpoint/runtime/license facts and availability. The same `capability.detect` node can bind a
YOLO-compatible, RF-DETR-compatible or previously unknown detector manifest.

`Available` is deliberately strict. It requires evidence for health, protocol compatibility,
contract validation, weight readiness and an explicit sample conversion. `Unconfigured`,
`MissingWeights`, `Disabled`, `Unknown`, `Unreachable`, `IncompatibleProtocol`, `InvalidContract`
and `FailedSmokeTest` remain visible but cannot enter a publishable Draft.

Score and geometry are independent. A model may report a high relative confidence while its box
is still only a coarse geometric hypothesis. Provider/VLM grounding defaults to coarse geometry;
ordinary trained detectors default to predicted geometry; prompted-segmentation masks are
mask-refined geometry. Human edits are human-verified, not model-calibrated.

The canonical types are in `crates/annotagent-core/src/expert_model.rs`. Existing generic Vision
descriptors migrate through this boundary without promoting an untested HTTP Worker to Available.

## Guided registration

Open **Settings → Vision Workers → Add expert model**. The six-step setup supports a known preset,
a generic HTTP Vision Protocol Worker, or the already-registered offline Mock. HTTP setup records
an endpoint, remote-access policy, timeout, and optional `env:VARIABLE_NAME` bearer-token
reference; secrets are resolved at request time and never serialized into Settings.

Discovery actively reads `/health`, `/v1/capabilities`, `/v1/models`, and `/v1/contracts`. A
selected-image sample then shows the input, bounded raw summary, converted typed Artifact,
normalized coordinates, score semantics, geometry semantics, duration, and warnings. The observed
evidence is persisted in the Git-ignored workspace Settings and survives Server restarts.

Registration is disabled unless health, protocol, contracts, immutable model identity/weights,
and sample conversion all pass. Discovery failure records the exact stage. Missing checkpoint
identity remains `MissingWeights`; stale or incomplete evidence can never be upgraded to
`Available` merely by enabling a checkbox.
