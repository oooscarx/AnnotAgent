# Builder Plan Preservation Alpha Status

## Milestone 0 — regression fixture (2026-09-05)

Status: complete.

- Saved the governing task as `BUILDER_PLAN_PRESERVATION_MASTER_PROMPT.md`.
- Confirmed the live failure with the configured GLM Builder: the persisted
  `find_geometry_refinement_path` observation reported a runnable prompted-segmentation path, but
  the final Draft remained the VLM bootstrap path.
- Added the deterministic scripted regression
  `discovered_prompted_segmentation_path_is_materialized_at_discovery_limit`.
- Before the fix, its explicit ignored run fails after proving `runnable=true`; the recovered Draft
  contains only `core.image_input`, `vlm_detection.detect`, `core.filter`, `static_validator`,
  `core.confidence_gate`, `review_gate`, and `commit`.
- The loss occurs in the discovery-deadline branch of `run_workflow_advisor_loop`: it calls
  `materialize_feasibility_draft` with the precomputed safe suggestion and does not consume the
  successful conversion-path observation.
- The regression is ignored only while Milestones 1–4 introduce durable candidates and salvage. It
  must be enabled and pass before release.

### Baseline

- `cargo fmt --all --check`: pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass.
- `cargo test --workspace --all-features`: pass; five external/billable tests remain explicitly
  ignored.
- `cargo build --workspace --all-features`: pass.
- `npm --prefix web run typecheck`: pass.
- `npm --prefix web test`: pass, 61 tests.
- `npm --prefix web run build`: pass with the pre-existing bundle-size warning.
- `npm --prefix web run test:e2e`: pass, 43 Chromium scenarios.

No remote, credential, model asset, Published Workflow, historical Run, or formal annotation was
modified by this milestone.

## Remaining milestones

All six milestones are complete.

## Milestone 1 — durable working plans (2026-09-05)

Status: complete.

- Added explicit `PipelineBuildMode` variants for FromScratch, ImproveExisting, RepairDraft and
  ResolveBindings.
- Every fresh Builder Session creates and persists an empty, project-scoped Working Draft before
  model discovery. Repair sessions explicitly retain their selected editable Draft.
- Added durable `PipelineFragment`, `PipelinePlanCandidate`, binding, score, sufficiency, geometry
  safety, Working Memory and planning-event structures.
- Agent Session JSON now exposes `build_mode`, `working_draft`, `working_memory`, `plan_candidates`,
  `selected_candidate_id`, `discovered_conversion_paths`, `planning_events`, and
  `salvage_outcome`; the existing SQLite Session store persists them without a second source of
  truth.
- Draft creation and deterministic recovery adopt the already-persisted Working Draft identity
  instead of creating an unrelated Draft at the end of discovery.
- Core serialization, SQLite round-trip, Application multi-turn editing and focused strict Clippy
  pass. Conversion observations are converted into fragments in Milestone 2.

## Milestone 2 — conversion path materialization (2026-09-05)

Status: complete.

- `PipelineFragment::from_conversion_path` converts every registered conversion step into a
  concrete node/edge blueprint and rejects missing nodes, incompatible ports and absent declared
  auxiliary inputs.
- `PipelineFragment::materialize_into` attaches the primary Artifact and required auxiliary
  Artifacts to an editable Draft, updates dependencies and returns the exact typed output endpoint.
- Successful artifact and geometry path Tool Results are captured immediately after the auditable
  Tool step. A context-derived deterministic Fragment ID prevents duplicate discovery from
  multiplying equivalent plans.
- Every captured Fragment creates a persisted partial Plan Candidate with its Observation source and
  unresolved capability binding. Binding and full-graph completion are performed by the M3
  synthesizer rather than hidden in the conversion registry.
- Core materialization and Application persistence tests pass; the original M0 regression still
  reaches the expected final missing-segmentation assertion until candidate synthesis/salvage land.

## Milestone 3 — Registry-driven synthesis (2026-09-05)

Status: complete.

- Added a deterministic `RegistryPipelineSynthesizer`. Candidate ranking is driven by runnable
  state, goal coverage, binding completeness, geometry safety, review/commit coverage and the
  explicit accuracy/cost/latency priority. Candidate origin is deliberately not a score input.
- A safe baseline and a Registry-composed geometry-refinement Candidate now compete as ordinary
  plans. Templates are seeds; a template can win only when its explicit score is better.
- The composed refinement consumes the discovered typed Fragment and produces the concrete
  detection → box-prompt → prompted-segmentation → mask-to-bbox → geometry-evaluation →
  geometry-decision → review → commit chain.
- Model bindings are resolved from Available Provider/Expert Registry records. Mock Providers,
  fixture connections, missing availability evidence and unpinned expert checkpoints cannot make a
  production Candidate runnable.
- The M0 fixture now proves that the runnable Registry refinement is persisted and selected before
  the discovery limit. Its final assertion intentionally remains red until Milestone 4 changes
  limit handling from template fallback to Candidate salvage.
- All 103 Core tests and 63 runnable Application tests pass; strict focused Clippy passes after the
  final formatting check.

## Milestone 4 — budget-safe deterministic salvage (2026-09-05)

Status: complete.

- Discovery now has an eight-call ceiling and cannot consume the separately persisted three-call
  materialization, three-call validation and three-call finalization reserves.
- A complete runnable Candidate ends ordinary discovery immediately. Candidate Selection and Draft
  Salvage are runtime-owned phases, so no additional model turn can discard or rewrite the selected
  plan.
- Salvage re-reads the current Registry, re-evaluates every binding, reranks all Candidates,
  materializes into the original Working Draft, normalizes runtime compatibility bindings, performs
  static validation and saves either a review-ready or explicitly blocked Draft.
- `DiscoveryLimitTriggeredSalvage` is a successful stop reason, paired with the structured
  `RunnableDraftMaterialized` or `BlockedDraftMaterialized` salvage outcome. It is not reported as
  budget exhaustion.
- The formerly ignored M0 regression is enabled and passes with prompted segmentation in the final
  Draft. A separate revalidation test disables the selected segmenter after discovery and proves
  the safe reviewed baseline wins rather than using stale Ready state.
- All 103 Core tests and 65 runnable Application tests pass; the only ignored Application test is
  the explicitly billable external Provider smoke test. Strict focused Clippy passes.

## Milestone 5 — prompt, API and product trace (2026-09-05)

Status: complete.

- The Builder contract now treats `build_mode` as authoritative, explains automatic Candidate
  persistence and deterministic salvage, and explicitly makes Workflow templates ordinary seeds.
- The HTTP request and Web client carry the tagged Build Mode. Build from scratch is the visible
  default; Improve Existing requires an explicit immutable `workflow_id@version`; Repair Draft and
  Resolve Bindings require an editable Draft.
- Improve Existing now copies the explicitly selected Published Version into a new Working Draft
  without clearing its graph. The original Published Version remains unchanged. FromScratch is
  covered with an existing Published bootstrap and proves that its marker is neither inherited nor
  used as salvage while the historical Version remains present.
- Resolve Bindings removes topology mutation, template and conversion-discovery actions from the
  model-visible tool set. It can inspect current model readiness, bind profiles, validate, Dry Run
  and finish without rediscovering the Catalog.
- The GUI exposes Candidate selection, node chain, model bindings, planning events, typed paths,
  Build Mode and salvage outcome. A preserved Candidate without materialization is shown as a
  diagnostic warning instead of disappearing behind a generic failure.
- `/advisor status` in the TUI reports Build Mode, phase, outcome, stop reason, Candidate selection,
  conversion-path count and salvage result.
- Focused Application regression tests, Server request decoding, TUI status, strict focused Clippy,
  Web typecheck and all 62 Web unit tests pass. No publication, formal Run, credential, remote or
  push was performed.

## Milestone 6 — current Project and release validation (2026-09-05)

Status: complete.

- Runtime supplements model-directed discovery with deterministic Registry conversion synthesis at
  salvage. The supplement is goal-, capability-, contract- and availability-driven; it contains no
  EfficientSAM, SAM, Qwen or RoboCup branch.
- The Builder Context and product readiness summary now count verified Ready local Expert Model
  Instances, eliminating the contradictory state where a working prompted segmenter appeared
  unavailable to the model and as `None ready` in the GUI.
- The final current-project GLM session selected the Registry-synthesized Qwen → box prompts →
  prompted segmentation → mask-to-bbox → geometry evaluation/decision → Review → Commit plan.
  The Draft passed static validation, and a real one-image Sample Test exercised both Qwen and the
  Ready local EfficientSAM process with zero node failures.
- Browser reload recovered the same selected Draft and persisted Sample Test. Candidate,
  selection, bindings, alternatives, planning state and salvage outcome remain visible.
- The release suite passes strict Rust formatting/Clippy, all workspace tests and build, both
  architecture scans, Web typecheck/62 unit tests/build, all 43 Chromium E2E scenarios and diff
  checks. No Workflow was published, no full dataset Run was started, no remote was changed and no
  push occurred.
