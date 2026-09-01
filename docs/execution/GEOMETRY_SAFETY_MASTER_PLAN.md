# AnnotAgent Geometry Safety Master Plan

This ledger executes `GEOMETRY_SAFETY_MASTER_PROMPT.md` without mutating historical Published
Workflow Versions or relying on live provider credentials.

## Milestones

| Milestone | Deliverable | Exit evidence |
|---|---|---|
| M0 | Reproduction and baseline | Unsafe VLM score-only Commit fixture passes under the legacy validator; repository inventory and acceptance ledger recorded. |
| M1 | Quality semantics and contracts | Operation-scoped quality contracts distinguish score meaning, geometry origin, calibration and auto-accept eligibility; migrations and tests pass. |
| M2 | Static geometry safety | Project policy, blocking validation codes, legacy compatibility and safe-draft migration are tested. |
| M3 | Review evidence | Structured correction reasons, quality reports, size buckets, persistence and APIs are tested. |
| M4 | Calibration | Exact-scope calibration, thresholds, insufficient-evidence states and staleness are tested. |
| M5 | Safe first draft | Builder tools and system policy produce review-safe VLM drafts and never fabricate unavailable refiners. |
| M6 | Refinement path | Detection → Prompt → Mask → BBox lineage and geometry comparison are executable and audited. |
| M7 | Improve Automation | Evidence-driven failure diagnosis, patch drafts, holdout comparison and recommendation rules are implemented. |
| M8 | Product and release | Guided/Expert UI, TUI, migrations, E2E, docs and full repository checks pass. |

## Non-negotiable boundaries

- Prompt guidance is not a substitute for Rust static validation.
- Model scores are never invented or reinterpreted as measured geometry quality.
- A configured adapter, a registered node, an available model and a healthy Worker are distinct states.
- Existing Published Versions and historical Runs remain immutable and inspectable.
- Improvements create Draft patches; humans publish.
- No API key, model weight, remote change or push belongs to this work.

## Work sequence

Each milestone updates the status and acceptance ledgers, runs scoped tests, fixes regressions and
lands as an independent local commit before the next milestone begins.
