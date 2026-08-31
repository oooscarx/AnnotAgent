# Prompted Segmentation (SAM-compatible) Pipeline

SAM is one possible backend for the generic `PromptedSegmentation` capability. It is not a Core
node, Label, or RoboCup-specific branch.

## Legal flow

```text
Image ───────────────────────────────┐
                                     v
DetectionSet → BoxPromptSet → capability.segment → MaskSet
       │             │                         │        │
       │             └ exact detection ref    │        └ exact prompt ref
       │                                       v
       └ original detector evidence ─── core.mask_to_bbox → refined DetectionSet
```

The prompted-segmentation Worker must return a `MaskSet`; it must not silently replace the mask
with a box. `core.mask_to_bbox` owns the visible, replayable geometry conversion.

## When it helps

Use prompted segmentation when a detector or VLM already found the correct semantic object but the
box is loose or geometrically inaccurate. The resulting Detection keeps the original box and model
evidence alongside the prompt, mask, and tight box.

Do not add SAM when:

- the Provider timed out or rejected credentials;
- no candidate exists, because there is no box/point prompt;
- the candidate is the wrong semantic object;
- no healthy prompted-segmentation Model is registered;
- any conversion node in the typed path is unavailable.

Those cases require infrastructure recovery, another detector, crop classification/domain
validation, or human Review rather than boundary refinement.

## Runtime bindings

- Offline tests use `mock-prompted-segmenter` and deterministic polygon masks.
- A real backend binds `capability.segment` to any enabled Vision Worker model declaring
  `PromptedSegmentation` and serving protocol v1 `/v1/infer`.
- Worker responses are checked for protocol, request, image, node, model identity, Artifact kind,
  prompt lineage, normalized geometry, and bounded response size.
- Generated `sam2` Worker scaffolds remain `missing_weights` until the user supplies legal local
  weights, implements inference, passes health/contracts/sample conversion, and explicitly enables
  the model.

## Replay and inspection

Each node output is a normal Pipeline Artifact in the DAG checkpoint. Replaying from
`capability.segment` preserves the upstream detector and prompt conversion. The Inspector can show
box-prompt and polygon-mask bounds, while the JSON detail exposes the exact prompt and evidence
references.
