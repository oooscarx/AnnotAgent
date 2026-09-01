# Geometry Safety Status

Last updated: 2026-09-02

## Current milestone

Milestone 7 — evidence-driven Improve Automation complete.

## Completed

- Read the master prompt to EOF and saved it in the repository.
- Verified `main` is eight commits ahead of `origin/main` before geometry-safety work began.
- Confirmed the active RoboCup Ball Workflow is `RoboCup Ball · VLM bootstrap@v1`.
- Confirmed its executable path is VLM Detection → label filtering → static domain validation →
  generic Confidence Gate → Commit, with Review only on the low-score route.
- Added a deterministic Core fixture containing semantic score `0.99`, coarse geometry, a loose
  predicted box and a tighter human reference.
- Reproduced the legacy behavior: the existing static validator accepts that unsafe graph because
  any upstream Validator currently satisfies `validate_commit_safety`.
- Added explicit semantic-confidence and detection-confidence score semantics.
- Added separate geometry calibration state, validation state, quality-score source and geometry
  quality reference without inventing localization scores.
- Added operation-scoped `ModelCapabilityQualityContract` with conservative VLM, specialist and
  prompted-segmentation defaults.
- Frozen Model Profile snapshots now include effective quality contracts; legacy profile JSON
  without the field migrates conservatively.
- New OpenAI-compatible VLM Detection artifacts now preserve `VisionLanguage`,
  `SemanticConfidence` and `CoarseHypothesis` instead of masquerading as specialist detections.
- Added the read-only Model Profile quality-contract API and user-declared override input; server
  binds overrides to the actual Model Profile revision and marks their source truthfully.
- Added conservative Project Geometry Policy defaults: bounding-box annotation requires training
  quality and uses Refiner-or-Review acceptance.
- Geometry-aware Core validation now follows every candidate-source-to-Commit path and rejects
  score-only acceptance of coarse or uncalibrated predicted geometry.
- Mandatory Human Review and an available prompted-refinement plus mask-to-bbox, geometry
  evaluation and geometry decision chain are legal; a mask alone is not a trust boundary. Missing
  and stale calibration have distinct blocking codes.
- `allow_unvalidated_commit` cannot bypass geometry policy.
- New bbox suggestions and RoboCup Ball templates route uncalibrated geometry through mandatory
  Review; classification behavior remains unchanged.
- Published snapshots record safety compatibility. Legacy snapshots remain immutable, are
  re-assessed before a new formal Run, and unsafe versions fail with `unsafe_legacy_workflow`.
- Added `Create geometry-safe Draft`, which clones a historical version and inserts mandatory
  Review boundaries without editing the original version.
- Split ephemeral Dry Run candidate observations from durable, Project-scoped
  `GeometryQualityReport` records.
- Bounding-box edits now persist typed correction reasons, original/reference geometry, IoU,
  normalized and pixel center shift, predicted/reference area, area/width/height ratios, target
  size bucket, Run/node lineage and exact Model Profile revision when available.
- Added small/medium/large correction summaries so small-object failures cannot disappear inside a
  single average.
- Added transactional SQLite persistence and Project/Run read APIs; Dry Run summaries consume the
  new records while retaining a compatibility read of legacy correction memory.
- Review now offers the controlled common reason taxonomy, plus enabled Skill-specific domain
  reasons. Unknown reason codes fail before the annotation is mutated.
- Added immutable `GeometryCalibrationReport` records keyed by Project, task/Label, Model Profile
  and revision, node definition/configuration, prompt, preprocessing, Label Schema, refiner chain
  and dataset profile revision. Credential values and API-key rotation are deliberately outside the
  key.
- Calibration evaluates median/p10 IoU, median/p90 center shift, median area-ratio error, manual
  adjustment rate, loose/tight rates and a separate small-object count against Project-owned
  thresholds.
- Added all six lifecycle states: Uncalibrated, Collecting Evidence, Provisional, Passed, Failed
  and effective Stale. Relevant model, prompt, pipeline, schema, refiner or dataset changes stale a
  historical report without mutating it.
- Project policy and immutable calibration reports persist through SQLite migration 11 and are
  available through Project-scoped policy, calibration list/create and exact report APIs.
- Publication and formal-Run validation now hydrate exact persisted calibration state. Passing
  calibration alone cannot turn a semantic confidence threshold into geometry evidence; an
  explicit Geometry Quality Evaluation → Geometry Decision boundary must consume it.
- Added five bounded, read-only Pipeline Builder Tools for exact model quality contracts, Project
  geometry policy, structured correction aggregates, calibration state and typed refinement-path
  availability.
- Bounding-box Builder sessions expose those Tools only during feasibility analysis and reserve
  final Tool Calls for Draft persistence/finalization rather than late catalog exploration.
- The Builder system contract now requires geometry evidence inspection before feasibility,
  rejects semantic/relative confidence as localization quality, treats grid/resize/prompt changes
  as uncalibrated, and preserves every proposal as an editable Draft.
- Refinement discovery filters out Mock connections and Mock Providers. Missing-weights or
  otherwise unavailable prompted segmenters are returned only as unapplied setup alternatives;
  the current runnable fallback is mandatory Human Review.
- The Qwen-only regression proves the first saved Draft retains a Human Review boundary, does not
  add `capability.segment`, and reports no passing calibration or runnable real segmenter.
- Added strict, versioned `GeometryRefinementTrace` lineage linking the exact source Detection,
  Box Prompt, Mask and refined Detection items, together with the original/refined boxes and mask
  score semantics.
- Added deterministic coarse/refined Geometry Evaluation for IoU, normalized center movement and
  area/width/height ratios. Large conflict, movement, tightening, expansion or explicitly required
  weak mask evidence becomes a typed geometry issue rather than a new confidence score.
- Added executable `core.geometry_quality_evaluation` and `core.geometry_decision` nodes to the
  public catalog and both Published Runtime executor paths. A decision accepts only a non-empty set
  in which every detection has a valid stable evaluation; all incomplete/unstable cases route to
  Review and record that semantic score was not used.
- Evidence-backed prompted-segmentation revisions now replace the raw semantic Confidence Gate with
  Mask-to-BBox → Geometry Evaluation → Geometry Decision and retain mandatory Human Review as the
  conservative first publication boundary.
- Static validation now rejects Refiner → Mask-to-BBox → Commit without evaluation/decision. A
  returned SAM mask is evidence, not proof of geometry quality.
- Added persistent Improvement sessions that bind a Published baseline, diagnosis Runs,
  independent evaluation Runs, two editable Drafts, a structured Patch diff, validation,
  before/after comparison and human-selected application state.
- Diagnosis classifies Provider/infrastructure, no-candidate, semantic, geometry, domain,
  missing-score, invalid-Artifact, budget and insufficient-evidence failures before selecting a
  repair. SAM is considered only for geometry errors with an existing candidate.
- Baseline and candidate now run on the same non-committing holdout and compare semantic recall and
  precision, robust geometry metrics, manual adjustment, review, cost, latency, failures and
  small/medium/large object buckets.
- Diagnosis and holdout Run sets must be disjoint, and comparison also rejects overlapping Project
  image indices. Four images never recommend; five to nine are provisional under default policy.
- Added Project-scoped persistence and the four requested Improvement REST operations. Applying
  explicit selected changes produces an editable Draft and never publishes.
- Added bounded read-only Builder Tool `compare_pipeline_geometry`; the registry now contains 65
  Tools and still exposes no publish, full-Run, credential, shell, download or arbitrary-URL
  escape hatch.

## In progress

- None within M7; the next implementation milestone is M8.

## Next

- Milestone 8: expose the completed safety and improvement contracts coherently in GUI/TUI, add
  release E2E coverage, finish user documentation and run the complete release verification.

## Recent verification

- Rust: `cargo test -p annotagent-core -p annotagent-skill-robocup -p annotagent-server` — passed
  (83 Core, 17 RoboCup including integration tests, 17 Server tests).
- Application: all 54 active tests passed; the billable provider smoke test remains explicitly
  ignored.
- The 100-image pause/restart/resume regression passed after moving that control-plane fixture to
  a non-geometric classification Project.
- Workspace: full Rust tests, all-feature build, all-target/all-feature Clippy with warnings denied,
  formatting and diff checks passed.
- Web: TypeScript, 41 unit tests, and production build passed.
- M3 focused: 85 Core tests, 14 Storage unit tests plus Storage integration suites, and the HTTP
  Review revision flow passed; Web TypeScript passed.
- M3 release: full workspace/all-feature Rust and doc tests passed (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy passed, all-feature build passed, and Web
  TypeScript, 41 unit tests and production build passed.
- M4 focused: 89 Core tests and 15 Storage tests passed; exact service-level calibration creation,
  stale-on-node-change and the Project calibration HTTP surface passed.
- M4 release: all 329 active Rust tests and doc tests passed (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy and all-feature build passed, and Web TypeScript,
  41 unit tests and production build passed.
- M5 focused: the Qwen-only conservative Draft, system policy, phased Tool visibility and complete
  64-Tool registry tests passed.
- M5 release: all 329 active Rust tests and doc tests passed (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt and diff checks passed;
  Web TypeScript, 41 unit tests and production build passed.
- M6 focused: all 92 Core, 25 Runtime and 55 active Application tests passed; the Application's one
  billable provider smoke remains explicitly ignored. The offline Published Runtime test executes
  the complete prompt/mask/refinement/evaluation/decision chain.
- M6 release: all 332 active Rust tests and doc tests passed (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt and diff checks passed;
  Web TypeScript, 41 unit tests and production build passed.
- M7 focused: four Core improvement-policy tests, the SQLite session round-trip, the exact 65-Tool
  registry, the Application immutable-baseline Patch regression and the Project-scoped HTTP surface
  passed.
- M7 release: all 339 active Rust tests and doc tests passed (one explicitly billable smoke
  ignored), strict all-target/all-feature Clippy, all-feature build, Rustfmt and diff checks passed;
  Web TypeScript, all 41 unit tests and the production build passed.
- Web: not run for M0; no Web behavior changed.
- E2E: not run for M0; the baseline is a Core static-validation fixture.
- Browser: 2026-09-02 read-only inspection of all four current RoboCup Ball Run Results confirmed
  correct semantic targets with imperfect box tightness and no Crop artifact.

## Recent local commit

- `93882e7 test(geometry): reproduce unsafe vlm bbox auto-acceptance` (M0).
- `291f20c feat(models): separate semantic confidence from geometry quality` (M1).
- `0cb775c feat(workflow): block uncalibrated geometry from score-only commit` (M2).
- `77c5dea feat(review): capture structured bbox correction evidence` (M3).
- `ee4a159 feat(evaluation): calibrate geometry quality by model and project` (M4).
- `d07b180 feat(agent): build geometry-safe pipelines from the first draft` (M5).
- `7ea44f8 feat(workflow): add auditable prompted geometry refinement` (M6).
- `feat(agent): improve pipelines from review and geometry evidence` (M7, this milestone commit).

## Release-blocking remainder

- M8 remains open. M7 implementation and full release-command verification are complete.

## Live-conditional items

- Real Qwen geometry comparison.
- Real SAM Worker inference.
- Specialist detector inference and weights.

## Real blockers

- No healthy prompted-segmentation Model Profile is currently registered.
- The four B-Human predictions do not yet have enough independent reviewed references to pass the
  default 30-sample calibration threshold.
