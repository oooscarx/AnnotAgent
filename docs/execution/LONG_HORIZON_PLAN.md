# AnnotAgent Label Pipeline Alpha — Long-Horizon Plan

This file is the durable execution map for the active Label Pipeline Alpha release. Workflow Alpha
M0–M9 is the completed foundation. A milestone is complete only when its gate has direct automated
evidence and a dedicated local commit.

## Invariants

- Core, Runtime, Server, and generic Web surfaces remain domain-neutral.
- Projects, Skills, Workflow versions, Models, Artifacts, and Runs have independent persisted identities.
- Published Workflow versions are immutable and Runs execute a complete snapshot.
- Exact geometry remains in typed Artifacts; a VLM never re-types deterministic or specialist-model coordinates.
- Model output is untrusted and cannot reach Commit without validation, review, or an explicit provenance-aware policy.
- No API key, authorization header, image payload, or secret reference value enters logs, SQLite history, traces, exports, or Git.
- Work is local only: do not push or modify the Git remote.

## Milestones

1. **M0 — Baseline and execution ledger**
   - Capture Git and complete Rust/Web baseline.
   - Maintain the five execution documents and `scripts/acceptance.sh`.
   - Status: completed in `9a19176`; acceptance evidence recorded.
2. **M1 — Protocol, Artifact, and state semantics audit**
   - Close remaining typed Artifact shapes and observable structured failures.
   - Prove all tool-call replay and Run/Task state gates.
   - Status: completed in `309d31a`; acceptance evidence recorded.
3. **M2 — Versioned strongly typed Workflow**
   - Add typed nodes, ports, edges, policies, resource requirements, precise validation, migrations, multi-Skill composition, and immutable snapshots.
   - Status: completed in `684ce6f`; acceptance evidence recorded.
4. **M3 — Generic DAG Runtime**
   - Execute published snapshots with topological parallelism, retry, fallback, gates, suspension/resume, cancellation, cache, usage, and replayable trace.
   - Status: completed in `33ab172`; acceptance evidence recorded.
5. **M4 — Model Registry and mixed backends**
   - Complete registry metadata, deterministic CV, versioned HTTP worker protocol, JSON-only VLM fallback, health, and failure behavior.
   - Status: completed in `b41f55d`; acceptance evidence recorded.
6. **M5 — Persistent Dataset Coordinator**
   - Add transactional global budget, image/node checkpoints, leases, restart recovery, and 100-image acceptance.
   - Status: completed in `92a5c5b`; acceptance evidence recorded.
7. **M6 — Advisor and Workflow Editor**
   - Complete constrained advisor, all persisted editing actions, sample-image Dry Run, compare, clone, archive, and explicit Run version selection.
   - Status: completed in `364c3ee`; acceptance evidence recorded.
8. **M7 — RoboCup hybrid Skill**
   - Supply three templates, real mixed execution through generic contracts, evaluation CLI, synthetic fixtures, and conditional live smoke evidence.
   - Status: completed offline in `08d3958`; live Qwen and configured external-weight smoke remain conditional.
9. **M8 — Review and data round trips**
   - Add geometry editing, undo/redo, comparisons, correction reasons, Native/COCO/LabelMe import, and compatibility reports.
   - Status: completed in `3636e0f`; acceptance evidence recorded.
10. **M9 — Hardening and release acceptance**
    - Complete security tests, observability, required docs, two offline demos, API/browser smoke, and the complete blocking matrix.
    - Status: completed in `b3ba536`; 113 Rust and 13 Web tests plus browser acceptance pass.

### Label Pipeline Alpha

11. **LP1 — Label composition and typed intermediate contracts**
    - Add shared stages, per-Label Pipelines, registry bindings, explicit set Artifacts, parent and
      subject lineage, static validation, and compilation to the existing flat immutable DAG.
    - Status: completed locally; 117 Rust tests and strict Clippy pass. Commit recorded with this
      milestone evidence.
12. **LP2 — Executable Core nodes and formal Skills**
    - Implement Image Input, Crop, Filter, Map Label, Attach Attribute, Confidence Gate, Human
      Review, Commit, Artifact Cache, and Replay over typed Pipeline Artifacts.
    - Add Classification Skill and Detection Skill with mock and versioned HTTP JSON bindings.
    - Crop remains a Core node; the detector produces only `DetectionSetArtifact`.
    - Status: completed locally; 122 Rust tests and strict Clippy pass. Commit recorded with this
      milestone evidence.
13. **LP3 — Offline examples and lifecycle gates**
    - Ship whole-image classification, detection, and crop-classification examples.
    - Prove shared detector execution once/image, classifier Replay without detector rerun, Dry Run
      isolation, 100-image batch, pause/resume/cancel, and active Run recovery.
    - Status: pending.
14. **LP4 — Bounded Advisor and application APIs**
    - Make Advisor input target-Label aware and constrain output to real Registry nodes, Models,
      Validators, and Refiners.
    - Persist editable Drafts, validate, Dry Run 1–10 images, publish immutable versions, expose
      typed node Artifacts, and Replay from an exact node.
    - Status: pending.
15. **LP5 — Product GUI and release acceptance**
    - Add Project Label authoring, Shared Stages, per-Label Pipelines, Node Catalog editing,
      bindings/configuration, Artifact bbox/crop preview, Inspector, and Replay.
    - Unimplemented controls remain disabled with an explicit reason; no mock screen may claim a
      missing Runtime capability.
    - Run all Rust/Web/security/browser gates and publish final evidence locally.
    - Status: pending.

## Working sequence

The active implementation order is Label composition/contracts → executable Core nodes and Skills
→ offline examples/lifecycle → bounded Advisor/APIs → GUI and release acceptance. RoboCup remains a
regression-tested later extension example, not a primary blocker.

## Completion rule

Label Pipeline Alpha is releasable only when every one of its 20 Release Blocking gates in
`ACCEPTANCE_EVIDENCE.md` has reproducible passing evidence. Missing evidence is incomplete, never
assumed success.
