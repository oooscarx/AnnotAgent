# Expert Vision Known Limitations

Updated: 2026-09-01

## Baseline limitations before M1

- Provider-backed `ModelProfile` records expose the shared Provider connection abstraction and
  Worker-backed profiles are represented by Expert Manifests in the existing Model Registry, but
  persistent Settings/SQLite still use the older Detection Worker projection until M5/M7.
- The complete availability enum and validation gate now exist, but live discovery/smoke evidence
  is not persisted through Settings until M2/M7.
- The generic protocol now includes model/contract discovery and optional warmup. Existing
  detection-specific adapters still emit their legacy capability shape until M5 migration.
- The reusable Python SDK and native/Python scaffold exist; generated adapters intentionally remain
  unavailable until a developer supplies identity, implementation, weights and sample evidence.
- The public prompted-segmentation path now uses explicit Prompt/Mask Artifacts and Core geometry
  conversion. The legacy RoboCup refiner remains readable for compatibility until M5 migration.
- Polygon and uncompressed COCO RLE support tight-box conversion. Compressed RLE must be decoded by
  the Worker; contour extraction from RLE for `core.mask_to_polygon` remains future work.
- The Builder can verify conversion paths, but M4/M6 still need failure-class and quality evidence
  before it should conditionally recommend the path.
- Dry Run reports counts, warnings, cost and duration but not structured failure classes or geometry
  quality/human-adjustment/refiner metrics.
- Settings can edit existing Detection Workers, but there is no generic discovery-driven Expert
  Model onboarding flow.

These limitations describe the pre-change baseline and will be removed or narrowed milestone by
milestone. Live model accuracy without configured legal weights remains explicitly conditional.
