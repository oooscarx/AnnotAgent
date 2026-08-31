# Expert Model Onboarding

Open **Settings → Vision Workers → Add expert model**. The guided flow supports SAM, YOLO,
RF-DETR, LocateAnything, PIDNet, Grounding DINO, a generic HTTP Worker and the existing offline
Mock.

## Registration flow

1. Choose a preset or generic Vision Protocol v1 Worker.
2. Configure a loopback endpoint, or explicitly allow remote HTTPS. Optional authentication uses
   an `env:VARIABLE_NAME` reference; the resolved secret is never stored in Settings.
3. Discover health, capabilities, model identities and complete Artifact contracts.
4. Confirm the live model version, checkpoint SHA-256, dataset/label space and license. Discovery
   fills missing identity and rejects conflicting local identity.
5. Run a selected Project image. Inspect input, bounded raw summary, converted typed Artifact,
   normalized coordinates, score/geometry semantics, duration and warnings.
6. Register only after health, protocol, contracts, weights identity and sample conversion all
   pass. The same evidence gate protects the advanced Enabled checkbox.

Evidence is saved in the Git-ignored workspace Settings and survives Server restarts. Discovery is
repeated before sampling, and an unresolved credential reference invalidates stale availability.

## SAM example

The SAM preset expects Image plus BoxPromptSet or PointPromptSet and returns MaskSet. Bounding-box
refinement remains explicit Core composition:

```text
DetectionSet → BoxPromptSet → Prompted Segmentation → MaskSet → Bounding Box
```

The built-in Mock proves orchestration only. A preset or scaffold without legal configured weights
stays `MissingWeights`; it is never shown as real model quality or made selectable by the Advisor.
