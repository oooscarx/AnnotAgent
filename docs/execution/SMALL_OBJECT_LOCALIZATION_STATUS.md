# Small-Object Localization Recovery Status

Last updated: 2026-09-05 CST

## Milestone 5 — Prompt-coverage-gated segmentation safety

- Prompted Segmentation now requires one valid, independently evidenced `Covered`
  `PromptCoverageArtifact` per prompt whenever the graph declares the coverage contract. Outside,
  partial, unknown, missing or mismatched coverage fails before the model backend is called.
- Automatic Prompted Segmentation → Commit paths must wire the typed Prompt Coverage output into
  the segment node. Historical explicitly reviewed paths remain readable and execute in a labeled
  `legacy_review_only` mode; they do not gain automatic publication authority.
- The RoboCup small-object recovery template routes the `refine` branch and the typed coverage
  evidence together into segmentation. Search/re-localization/review branches cannot accidentally
  invoke the Refiner.
- Mask-to-BBox and Geometry Evaluation retain the original, prompt, mask and refined lineage. A
  drifting Refiner result is rejected: Geometry Decision restores the coarse/local candidate,
  records the rejected geometry and `refiner_drift`, and routes the image to Review.
- Regression coverage includes a Covered prompt reaching the segmentation backend, an outside
  prompt being rejected before inference, and a fourfold mask-bbox expansion being rejected while
  coarse evidence survives.
- Focused Core (109), Segmentation (3), Runtime (29), RoboCup (17) and Application (69 runnable,
  one explicit billable ignore) tests pass. No Provider request, credential access, Published
  mutation, formal Run, push or remote operation was performed.

Milestone 5 status: `PASS`.

## Milestone 4 — Versioned Ball resources and evidence-driven Builder

- Added three `robocup.ball` Prompt Resources at version `2.0.0`: whole-image detection, local
  re-localization and crop verification. Every VLM node freezes the resource id, version, exact
  SHA-256 and prompt content; compatibility Projects expose the same resources without model-brand
  assumptions.
- Added `robocup.ball.small-object-recovery`: coarse VLM → candidate-relative scale/search region
  → untouched-original crop → local crop-coordinate re-localization → root projection → domain
  validation → independent crop verification → Prompt Coverage → prompted segmentation → mask
  bbox → geometry evaluation/decision → Review/Commit.
- The recovery graph is Registry-bound. Ready production Provider profiles bind VLM nodes; Ready,
  checkpoint-pinned, non-Fixture Expert Models can bind prompted segmentation. Missing refinement
  setup produces a reviewed Draft/Setup Alternative, never a mock or invented binding.
- Deterministic Candidate ranking rewards explicit expansion, coordinate projection and Prompt
  Coverage for Balanced/Accurate goals, while Low Cost continues to penalize additional model
  calls. `Improve Automation` detects coarse misses, prompt-coverage failures and Refiner drift and
  adds the recovery subgraph to the selected immutable baseline's editable clone.
- Dry Run diagnosis now rejects unconditional segmentation when evidence says the prompt is outside
  the target. Provider failure, no candidate, semantic false positive, coarse localization miss,
  loose geometry and Refiner drift retain different permitted repairs.
- Focused Core (108), Runtime (29), RoboCup (17) and Application (69 runnable, one explicit
  billable ignore) tests pass. No Provider request, credential access, Published mutation, formal
  Run, push or remote operation was performed.

Milestone 4 status: `PASS`.

## Milestone 3 — Bounded tile search and recovery budget

- Added `LocalizationRecoveryBudget`, usage and decision contracts for local re-localization,
  tile-search stages, prompted-segmentation calls, total model calls and exact Decimal cost.
  Exhaustion returns `HumanReview`.
- `core.tile` now emits at most the configured tile budget (default maximum 9), records the full
  planned count and whether search was truncated, instead of failing the Run because an unbounded
  grid was requested.
- Bound detection executes multiple local/tile images as separate, bounded model calls using their
  untouched root-image pixels. `maximum_model_calls` truncates work deterministically and exposes
  the truncation as evidence.
- Added `core.merge_tiles` with deterministic score ordering, label-aware IoU deduplication and
  preservation of evidence from overlapping detections.
- The existing Prompt Coverage gate routes outside prompts to `search_tiles`; a graph can route
  budget exhaustion to Review without classifying it as Provider failure.

Milestone 3 status: `PASS`.

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
