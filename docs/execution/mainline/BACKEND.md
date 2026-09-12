# Mainline Backend execution

- Baseline: `d3220cb54bd50589efed86b6c0753bf4ea8db0db`
- Worktree: `/Users/oscar/Documents/my_workspace/AnnotAgent-mainline-backend-v1`
- Branch: `codex/mainline-backend-v1`
- Initial status: clean. The shared frontend worktree was intentionally not modified.

## Current work

B0 source audit and integration contract. Existing delivery intent, deterministic
Schema preparation, formal image/object review, frozen training package engine,
Conversation/Plan/Builder/Journey/Sample/Processing, stop/queue/resume, Model Profile
CAS and history scope are being reused.

## B0 findings

- The current mainline delivery code is present only on the `d3220cb` line; the
  independently advanced task-history frontend line does not contain those Rust
  modules. Backend work therefore starts from the shared contract baseline.
- Package consent/readiness/admission exists in Application/Storage but has no HTTP
  route and no last-review trigger.
- Existing Task workspace does not aggregate delivery/package or distinguish one
  request completion from package-level Task completion.
- SampleCandidate chat lineage is fully validated. Formal Run/Annotation visual
  lineage is not a Conversation reference variant yet; bbox-only writes remain unsafe.
- Processing preview currently derives project order + optional count, not an exact
  delivery image-ID list. The required 10-select-3 proof is not yet established.
- Model Profile CAS and persisted history cutoff are implemented. Targeted reruns on
  this worktree passed: Storage two-connection CAS preserves Published snapshot;
  Server stale/legacy PATCH behavior; Server cutoff confirmation, restart, scoped
  pagination and management fail-closed behavior (3 tests, 0 failed).

## First deliverable

`docs/contracts/mainline-v1/HTTP_BINDINGS.md` and `EXAMPLES.json` map implemented and
planned contracts for all three frontend owners. No production behavior changes in B0.

## Next

B1 extends the existing Task workspace with a revisioned read model and a thin,
idempotent authorized advance command. B2 adds full formal visual lineage and result
projection. B3 exposes/claims existing package consent and exact Task image-to-Run
review scope. B4 adds capability readiness and bounded/concurrency HTTP evidence.
