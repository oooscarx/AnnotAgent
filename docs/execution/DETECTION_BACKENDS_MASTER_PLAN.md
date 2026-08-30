# Detection Backends Master Plan

Updated: 2026-08-30

This plan implements **AnnotAgent Open-Vocabulary + Specialist Detection Alpha** on top of the
existing Agent + Skill architecture and Guided Project Workspace. Model brands remain Model
Registry data and backend-adapter implementation details; Core and the generic Runtime schedule
capabilities and typed Artifacts.

## Non-negotiable boundaries

- AnnotAgent remains the product and global brand.
- Core and generic Workflow code cannot branch on LocateAnything, RF-DETR, YOLO, Qwen, RoboCup,
  football, or any other model/domain identity.
- Missing detector scores remain unknown. They are never replaced with a default or averaged with
  incomparable values.
- Every detector contribution remains individually inspectable through Artifact lineage.
- HTTP workers are optional, bounded, untrusted processes. Their absence cannot prevent AnnotAgent
  from starting.
- Mock evidence proves the offline contract. Real model results are reported only when actually
  executed with a verified model version and legal environment.
- No remote mutation, push, model-weight commit, API-key use, automatic weight download, reset,
  rebase, amend, or destructive checkout is authorized.

## Milestones

| Milestone | Scope | Required local commit |
| --- | --- | --- |
| M0 | Git/code/test baseline, protocol and Artifact audit, official-license source plan, execution ledgers | `docs: establish mixed detection backend baseline` |
| M1 | Detection capabilities, Model Descriptor/version/contracts, score semantics, license, availability, validation | `feat(core): model open-vocabulary and specialist detection capabilities` |
| M2 | Optional detection score, per-source evidence, Candidate Clusters, lineage, persistence/API compatibility | `feat(core): preserve detection evidence and score semantics` |
| M3 | One versioned detection worker protocol, health/capability discovery/infer, loopback policy, limits, cancellation and structured errors | `feat(provider): add versioned detection worker protocol` |
| M4 | Open-vocabulary Capability Skill plus LocateAnything HTTP worker/adapter, Mock, Settings and docs | `feat(models): integrate locateanything grounding backend` |
| M5 | Generic Object Detection Skill plus RF-DETR HTTP worker/adapter, Mock, metadata, Settings and docs | `feat(models): integrate rfdetr detection backend` |
| M6 | Generic Candidate Match and Evidence Gate nodes, explanation contract, tests and evidence UI | `feat(runtime): combine detector evidence without fabricating scores` |
| M7 | Registry-bounded cold-start/specialist Advisor and bounded evidence-driven Recovery Agent | `feat(agent): select open-vocabulary fallbacks from detection evidence` |
| M8 | Capability-bound RoboCup Ball hybrid templates, validators, correction policy and seven offline scenarios | `feat(robocup): add specialist and open-vocabulary ball workflow` |
| M9 | Guided recommendations, Model worker management, mixed-evidence Results/Review, TUI and accessibility | `feat(ui): explain mixed detector evidence and fallbacks` |
| M10 | 100-image reliability, cache/replay/fault tests, browser acceptance, smoke audit, docs and demo | `test(release): validate open-vocabulary and specialist detection alpha` |

## Completion protocol

For every Milestone:

1. implement real behavior and migrations;
2. add focused tests and run them;
3. update Status, Decisions, Acceptance, Blockers, and Known Limitations;
4. scan Core and generic UI for model/domain branches;
5. create the exact independent local commit;
6. continue to the next Milestone without waiting for confirmation.

Alpha is complete only when every in-repository Release Blocking row is `PASS` and remaining
external model checks are explicitly `LIVE-CONDITIONAL` with an exact blocker.

## Progress

- M0 complete — `cf2d988 docs: establish mixed detection backend baseline`
- M1 complete — `53a7085 feat(core): model open-vocabulary and specialist detection capabilities`
- M2 complete — `1017146 feat(core): preserve detection evidence and score semantics`
- M3 complete — `333098a feat(provider): add versioned detection worker protocol`
- M4 complete — `a3ff9c2 feat(models): integrate locateanything grounding backend`
- M5 complete — `e372d02 feat(models): integrate rfdetr detection backend`
- M6 complete — `5ea8689 feat(runtime): combine detector evidence without fabricating scores`
- M7 complete — `759dedb feat(agent): select open-vocabulary fallbacks from detection evidence`
- M8 complete — this document's containing commit
