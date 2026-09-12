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

## B4 capability readiness

- `GET .../tasks/{task}/capability-readiness` and the Task workspace now return one
  task-owned, versioned Registry snapshot across Model Profiles, installed Plugin
  models and Model Instances. Agent and visual roles, Draft revision/hash, quality
  contracts, readiness, production eligibility, blockers and setup API are explicit.
- The current unexpired/unrevoked Journey consent is projected with its exact
  permission digest and allowed model binding digests. A Registry change updates the
  snapshot revision but never expands that grant or marks the Task resumable.
- The read path resolves no credential bytes and performs no network health check,
  Plugin call, install or inference. Task model-call receipts are counted once by
  durable call identity; zero calls are known zero and non-comparable priced usage is
  retained as unknown.

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

Frontend integration support only. New work is limited to concrete contract defects
reported against these committed B0–B4 interfaces.

## Delivery commits

- B0 contract audit: `99d16f997db2816a7f41db8640d1d6dd1b75693f`
- B1 Task read model and authorized local advance:
  `22ddee918730f29e5f984e19550c5b62201e6945`
- B3 exact delivery processing/formal source/package consent:
  `1e2bac549f52ed98c2f9fbec9ed9f288f3e05251`
- B2 Sample visual selection and natural-language intake action:
  `2fe42ebcb1746bd985e5204854b2de8c101c5160`
- B2 formal annotation Conversation lineage:
  `a4e4be1d765c3cf3c18a538571490fda5ca6dcf8`
- Post-Sample formal processing action:
  `3bf3b4e32d0a3a07f156c2f89f6703e81a410195`
- B4 passive task capability snapshot:
  `3d5b4429db4963a5d2ee81fabec67f2e979dde4b`

The integration tip is the last SHA and includes the complete ordered chain. There
are no new SQL migrations in this branch. Existing annotations, Workflows, Runs,
grants and history are reused; the formal Conversation reference is an additive JSON
variant.

## Reproducible isolated HTTP environment

From this worktree:

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --smoke
```

For browser integration, omit `--smoke`; optionally pass an already-built directory
with `--web-dist /absolute/path/to/dist`. The launcher allocates two loopback ports,
rejects port 8787, creates a marked `TEST-agent-ui-*` workspace under the system temp
directory, strips Provider credential environment variables, and starts the existing
Rust TEST Provider plus the real Router/SQLite application. Its printed manifest can
be reused with `--workspace <printed TEST path>`; GET does not seed or resume work.

The delivery-tip smoke run used API `127.0.0.1:55661`, TEST Provider
`127.0.0.1:55662`, and retained isolated evidence under
`/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-55jnxoy6`.
Both owned processes stopped after the run. No real credential, paid Provider, model
download, user workspace or 8787 service was used.

## Final verification

- `cargo fmt --all -- --check`
- `cargo clippy -p annotagent-storage -p annotagent-application -p annotagent-server --lib --tests -- -D warnings`
- `cargo test --workspace --offline`: 793 passed, 0 failed, 6 explicitly ignored.
- Opt-in real HTTP/SQLite/TEST Provider smoke passed at delivery tip, including
  Journey, Sample, queue/HumanRequest, stop/interrupted, paused resume, bbox, SSE and
  export paths. The Task workspace call in that smoke also executes the new passive
  capability projection.
- Focused B0–B4 regressions cover Task advance replay/stale CAS; non-prefix 3-of-10
  scope; formal source and annotation lineage; per-candidate Artifact identity; final
  review one-shot package admission/restart; Registry Profile revision change and
  owner isolation; 100-image pause/restart/resume; and bounded summaries for 100
  Projects with 1000 Runs/reviews.

## Remaining limits

- Conversation thread text is still persisted user text. Assistant-facing content is
  the real structured Schema/Builder/Sample/processing/feedback evidence; no generic
  assistant prose or private reasoning is invented.
- Formal annotation prose opens an exact structured CAS edit action. It does not ask
  a model to infer geometry.
- Capability readiness is a credential-free saved-evidence snapshot. `unknown` does
  not trigger a probe. Its Registry revision is a snapshot SHA-256, not a global
  monotonic counter.
- Task cost counts durable Conversation call receipts once. Processing allowance is
  separately present in `budget`; heterogeneous Provider pricing that cannot be
  compared remains unknown.
- The TEST suite validates package contents and portable ZIP semantics. Commercial
  model accuracy, explicitly supplied large-weight ONNX integrations, browser-downloaded
  ZIP in an unrelated environment, and an official training loader remain unverified.

## P0 automatic continuation increment

- Base: `563f2f7c64e8523fe2ad0b6d1c478f8c5dfab4ce` (durable Journey queue and
  Builder-to-Sample continuation).
- Image import/upload responses now return exact current image identities. A new
  Task Send may freeze those identities through `task_images`; duplicate command
  replay restores the same Task and partial DeliveryIntake.
- A partial intake containing exact images and missing only label/target semantics
  exposes one `build_and_test_pipeline` action. Its no-query Journey preview freezes
  a maximum of three Sample images, exact Provider/model binding digests and stable
  command IDs. Six Task images remain six; they are not truncated to the Sample cap.
- Saving the exact Journey consent durably queues Schema→Builder→Sample. No second
  technical execution click is needed. Existing callers may repeat the execution
  POST safely.
- Automatic model selection is limited to one ready Project-bound visual model,
  preferring `primary_inference`. Missing/ambiguous binding returns a server-owned
  `visual_inference` setup request and performs no call.
- The Schema Provider transport now carries its admitted call UUID into the existing
  per-attempt usage observer. This fixes a TEST HTTP failure that previously settled
  immediately as `in_doubt` before reaching the Provider. There is still no retry of
  an unknown result.
- Fresh isolated HTTP smoke at API `127.0.0.1:8870`, TEST Provider
  `127.0.0.1:8871` completed Schema, Builder, Sample, formal processing, stop,
  review, SSE and export. Evidence is retained in
  `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-imsjr9j9`.
  This was a dirty-tree precommit diagnostic and is not represented as evidence for
  the base SHA; a clean committed-SHA replay is required for the handoff.
