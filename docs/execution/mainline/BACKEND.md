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

## B1

- `GET .../tasks/{task}/workspace` now adds a deterministic
  `read_model_revision`, the existing delivery view, and a `mainline` projection.
  It reconciles the matching human Schema, current whole-image review counts,
  package consents/jobs and task completion without dispatching work.
- `POST .../tasks/{task}/advance` accepts an exact command, read-model revision and
  action. The only B1 `authorized` action is deterministic
  `prepare_delivery_schema`; Builder, Journey, image processing and packaging keep
  their separate existing preview/consent boundaries.
- Lost-response retry is backed by the existing persisted human-Schema
  `source_request_id`. A changed command scope or stale Task revision is rejected;
  a model call, completed Builder or completed processing operation does not mark the
  dataset Task complete. Only a ready package does.
- Focused HTTP test covers passive projection, local advance, exact replay, stale
  revision conflict and one-schema persistence. No Provider is created or called.

## B3 incremental delivery

- Delivery processing now freezes the exact saved image IDs and hashes. It rejects a
  `limit` for delivery Tasks; the legacy prefix behavior remains for non-delivery
  processing. A pure boundary test proves a non-prefix 3-of-10 selection and rejects
  changed hashes.
- Formal sources now join Task processing operation → exact delivery revision → Batch
  image → child Run. A project-global terminal Run is no longer offered as a review
  source. `GET formal-result` and paged `GET delivery-review-items` expose that
  lineage and bounded review state.
- Package consent list/get/authorize/cancel HTTP routes now expose existing durable
  storage. The last current whole-image review, or consent after reviews are ready,
  admits at most one existing local packaging job. GET remains passive. A real HTTP
  test creates original images/formal annotations, arms before review, observes the
  blocked state, saves final review, validates one ready ZIP job, exact replay and
  restart recovery without a Provider.

## B2 incremental selection

- `GET .../tasks/{task}/visual-selections` is the canonical SampleCandidate source.
  It returns one atomic envelope for Project/conversation/Task Schema, Draft revision,
  Sample Test, image bytes/result revision and each terminal candidate's own Artifact.
  A two-candidate regression proves Artifact IDs are not shared at image scope.
- A missing/partial delivery intake now exposes the existing text-only Schema preview
  as `propose_delivery_semantics` alongside manual intake. It still requires explicit
  call confirmation and produces a proposal only; it sees no pixels and grants no
  Builder/processing authority.
- Formal review annotations now expose canonical Conversation references containing
  the full Task/Schema/delivery/processing/Batch/Run/annotation revision/snapshot
  lineage. Application and storage validate that lineage independently before the
  message is committed. Formal feedback enters the existing feedback read surface
  with the exact saved subject and existing CAS edit action; it never enters the
  generic queue. `model_call_supported:false` records the bounded behavior: prose
  does not authorize inferred bounding-box geometry.
- Once an exact Journey Sample is `passed` or `human_approved`, the Task read model
  exposes `start_delivery_processing` with the existing processing preview URL and
  current delivery/Draft/Sample scope. This closes the post-Sample action gap without
  dispatching on GET or treating Sample permission as formal processing permission.

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
