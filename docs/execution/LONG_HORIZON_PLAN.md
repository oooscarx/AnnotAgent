# AnnotAgent Workflow Alpha — Long-Horizon Plan

This file is the durable execution map for the Workflow Alpha release. A milestone is complete only when its gate has direct automated evidence and a dedicated local commit.

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
2. **M1 — Protocol, Artifact, and state semantics audit**
   - Close remaining typed Artifact shapes and observable structured failures.
   - Prove all tool-call replay and Run/Task state gates.
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
10. **M9 — Hardening and release acceptance**
    - Complete security tests, observability, required docs, two offline demos, API/browser smoke, and the complete blocking matrix.

## Working sequence

The active order is protocol/state correctness → Artifact → Workflow model → DAG Runtime → Model backends → batch checkpoint → Advisor/editor → RoboCup example → Review/import/export → hardening. Model-specific integrations and visual canvas work cannot jump ahead of scheduling and persistence correctness.

## Completion rule

Workflow Alpha is releasable only when every non-live-conditional row in `ACCEPTANCE_EVIDENCE.md` has a reproducible passing command or inspected behavior. Missing evidence is treated as incomplete, not assumed success.
