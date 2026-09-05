# Small-Object Localization Recovery Status

Last updated: 2026-09-05 CST

## Milestone 2 — Bounded coarse-to-fine re-localization

- Added typed, serializable `RegionExpansionPolicy` variants for image-fraction,
  candidate-relative and direction-aware search. Candidate-relative expansion enforces minimum
  pixel dimensions, a maximum image fraction and edge-safe clamping.
- Added observable `TargetScaleProfile` evidence from candidate geometry and source-image pixels.
  Tiny/small/medium thresholds are node or Skill configuration rather than model-brand logic.
- Registered and implemented `core.expand_region`. It preserves the source evidence, records the
  original candidate, expansion policy and scale profile, and marks output as an intermediate
  localization search region.
- Extended `core.crop` to optionally emit executable image Artifacts as well as CropSet metadata.
  Each image keeps its parent and exact normalized root-image region.
- Bound detection runners now crop the untouched source image for one local search input, record the
  source-image Artifact and produce local coordinates for the existing
  `core.project_coordinates` transform. Multiple implicit local crops fail closed instead of using
  the wrong image.
- The M0 miss is recovered by a 96×96 candidate-relative search crop that fully contains the known
  football above the bad coarse box.

Milestone 2 status: `PASS`.

## Milestone 1 — Failure classification and Prompt Coverage

- Added the domain-neutral `LocalizationFailureClass` taxonomy. `prompt_outside_target` resolves to
  both `CoarseLocalizationMiss` and `PromptCoverageFailure`, never `LooseGeometry`.
- Added typed `PromptCoverageArtifact`, state, action and observable evidence contracts. A known
  state without evidence is invalid; missing evidence remains `Unknown` and has no fabricated
  confidence.
- Added executable `core.prompt_coverage_gate`. Independent/re-localized detections determine
  covered, partially covered or outside states by measured intersection; the source DetectionSet
  cannot validate its own prompt.
- Gate routes are bounded and explicit: `refine`, `relocalize`, `search_tiles` or `review`. Outside
  and unknown prompts do not enter the automatic refinement route.
- Static validation blocks an automatic prompted-segmentation-to-Commit path without a Prompt
  Coverage Gate while preserving explicitly human-reviewed debug paths.

Milestone 1 status: `PASS`.

## Milestone 0 — Regression baseline

- Added a deterministic 544×448 non-square image generator with a generator-owned 16 px football
  bounding box, green field, white line and white-shoe distractors.
- The fixture fixes a deliberately wrong coarse detection below the football. Its intersection with
  the football is exactly zero, and the legacy `0.02` full-image prompt padding still does not cover
  the target.
- The historical-style refined box also has zero intersection with the target and grows to
  4.6973686 times the coarse-box area, preserving the refiner-drift failure before any recovery
  implementation changes behavior.
- Preserved the current B-Human Sample Test as provisional evidence in
  `fixtures/small-object-localization/bhuman-provisional.json`. It contains no invented Ground
  Truth and explicitly requires human calibration before any real-image accuracy claim.
- Captured the backend projection defect: one coarse detection and one refined detection from the
  same lineage currently produce two Sample Test Results. The target terminal projection count is
  one; implementation is assigned to Milestone 6.

Milestone 0 status: `PASS`.
