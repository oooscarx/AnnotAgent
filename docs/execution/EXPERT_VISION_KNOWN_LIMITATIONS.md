# Expert Vision Known Limitations

Updated: 2026-09-01

## Current limitations after M8

- Provider-backed `ModelProfile` records expose the shared Provider connection abstraction and
  Worker-backed profiles are represented by Expert Manifests in the existing Model Registry.
  Workspace Settings still call the persisted collection `detection_workers` for compatibility,
  even when a Worker provides segmentation rather than detection.
- Discovery and selected-image smoke evidence are persisted through Settings. Authentication
  references currently support environment bearer tokens; workspace-file Worker credentials are a
  future extension of the shared SecretStore boundary.
- The generic protocol includes model/contract discovery and optional warmup. Legacy Detection
  Worker endpoints remain readable for compatibility, but new setup validates the complete generic
  protocol before registration.
- The reusable Python SDK and native/Python scaffold exist; generated adapters intentionally remain
  unavailable until a developer supplies identity, implementation, weights and sample evidence.
- The public prompted-segmentation path uses explicit Prompt/Mask Artifacts and Core geometry
  conversion. Legacy RoboCup refiner Drafts are migrated to that public chain when loaded.
- Polygon and uncompressed COCO RLE support tight-box conversion. Compressed RLE must be decoded by
  the Worker; contour extraction from RLE for `core.mask_to_polygon` remains future work.
- Dry Run and the Builder expose structured failure, geometry quality, human-adjustment and refiner
  metrics. A dedicated visual comparison of original and refined masks remains future product work.
- The guided setup supports presets and generic HTTP Workers. “Use mock Worker” selects the already
  registered deterministic offline model rather than creating duplicate mock configuration.
- Real SAM quality remains live-conditional because this repository contains no downloaded
  checkpoint and the current host has no verified SAM weight identity.

Live model accuracy without configured legal weights remains explicitly conditional.
