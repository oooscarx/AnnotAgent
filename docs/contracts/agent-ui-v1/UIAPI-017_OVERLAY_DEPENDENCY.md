# UIAPI-017 round 2 — concrete execution-overlay dependency

**Status: live Replay remains unimplemented.** This delivery is an executable boundary regression and a minimum scoped interface specification, not a live HTTP success claim. The existing Core/Mock command contract is unchanged. No credential or external model was accessed.

## Verified interfaces that prevent a safe adapter-only change

1. `crates/annotagent-application/src/published_run.rs::PublishedWorkflowRuntime::new` accepts one Workflow, one Provider kind and one optional temporary key. It initializes every `workflow.snapshot.model_profiles` entry before the descendant set is passed to `executor_for_nodes`. Filtering runners later therefore does not establish descendant-only current connection authority.
2. The same file's `execution_for_node` indexes a shared `BTreeMap<ModelProfileId, ModelExecution>` using the immutable node's `model_profile_binding`. It cannot express two nodes with the same historical Profile but separately approved current bindings. `BoundClassificationRunner`, `BoundDetectionRunner` and `BoundPromptedSegmentationRunner` similarly read the historical `context.node.model_binding` and `workflow.snapshot.plugin_models` to start Plugin execution.
3. `crates/annotagent-server/src/lib.rs::resolve_runtime_model_profiles` supports one Provider connection and resolves its current credential, but takes only frozen profile snapshots. It is not a current descendant binding authorization validator. Reusing it before exact scope checks would turn a passive preview into credential resolution; using it alone after checks would omit revalidation when permission is revoked during an operation.
4. `crates/annotagent-application/src/sample_limits.rs::SampleCalls::begin` has local/batch/conversation paths. The local allowance checks cancellation and atomically decrements a counter; it has no current Registry-permission guard. Merely wrapping Replay with `with_sample_request_limit` proves the numeric cap, not stale/revoked-binding rejection before every call. Plugin startup occurs before its inference backend is wrapped, so a Replay guard is also required before starting an installed worker.
5. `crates/annotagent-runtime/src/dag.rs::replay_from` requires the source checkpoint Workflow hash to equal the supplied immutable Workflow hash; `run` then verifies snapshot material. Replacing snapshot bindings without rehashing fails the latter; rehashing fails the former. **Neither check should be removed, and the source checkpoint hash must not be rewritten.**

These are implementation dependencies within existing modules, not a missing third-party engine or a reason to enable historical credential fallback.

## Minimum scoped change, without a new DAG engine

The following is a proposed internal interface, **not an exposed HTTP API**:

- Add a server-derived `ReplayExecutionOverlay` containing the original source record/checkpoint hashes, exact ordered downstream node IDs, and a per-node current binding record. Each record contains the selected Profile or Plugin/Model Instance ID, its frozen current snapshot, its permission/destination digest, and the required node capability. Include image/input identity, sandbox-only destinations, maximum model requests, deadline and explicit unknown-cost authorization in the overlay digest. Do not persist credentials in it.
- Reuse `conversation_journey.rs::journey_model_description` (currently private) for safe destination/permission digests and `ModelProfileSnapshot::frozen` for enabled/current Profile and Provider checks. Reuse `freeze_plugin_model_selections` for Ready installed Plugin identity. Extend capability checking for each node; an Image-capable model alone is not sufficient evidence that a prompted-segmentation node can use it. No preview reads credential bytes or starts a worker.
- Add `PublishedWorkflowRuntime::for_replay` (or an equivalent private builder) which preserves the original Workflow object used by the DAG and builds execution adapters **only** from the approved overlay. Pass current credentials transiently after owned exact admission. Initially keep the existing one-Provider connection restriction explicit; reject mixed connections rather than silently applying one credential to another destination.
- Introduce a narrow runner wrapper or per-node execution lookup. It supplies only the approved execution binding to the existing classification/detection/segmentation adapter while the DAG continues to validate the original Workflow/checkpoint. A wrapper can borrow an ephemeral node clone differing only in binding fields; topology, parameters, labels, geometry policies, retry/fallback and destinations remain original. Use current overlay Plugin snapshots for these runners. Record the overlay digest/current model identity in Replay result metadata so it cannot masquerade as inference by the old model.
- Replay creates a fresh `PublishedDagExecutor`, so its cache is not shared with the source Run. Nevertheless, turn off deterministic cache reuse for overlay-bound runners within that executor, or include the overlay digest in their execution cache identity. `node_cache_key` currently derives model configuration from the original snapshot; it cannot distinguish two current overlays for otherwise identical original nodes.
- Add a Replay-specific guard to the existing `SampleCalls` wrapper: immediately before worker startup and each Provider/backend invocation, re-read the exact current binding/permission digest and cancellation/deadline, then reserve from the **same shared** allowance. A revoked or changed binding must reject before `inner.complete`/`infer`/`infer_pipeline`. Use the existing persisted command as the only dispatch authority; do not reserve a new command or retry a timed-out one.
- Keep existing receipt ownership, command replay and process-loss `outcome_unknown` semantics. Store the authorized overlay/digest with the request and verify the source record hash again when loading the checkpoint. GET remains passive.

This does not require publishing a new Workflow, altering the original Run/checkpoint, or replacing the runtime DAG executor. It does require coordinated Application runner construction + per-call admission work; changing only the HTTP body or deleting the live guard cannot meet the requested invariant.

## Required acceptance before live scope becomes available

1. Use a loopback deterministic HTTP OpenAI-compatible TEST Provider with an actual Registry Profile, not the Mock adapter. Start from a completed source checkpoint, authorize the exact current descendant binding, and count HTTP requests. Source record and upstream outputs must remain identical; formal annotation and Published/default counts/JSON remain identical.
2. Change/disable the Profile or Provider after preview: exact POST rejects with zero HTTP requests. Change/revoke it between two descendant calls: the next call rejects before invocation and the original receipt records the bounded failure. Test the same permission/digest change for an installed TEST Plugin; do not install/download a real model.
3. Change command/image/model/destination/limit/unknown-cost scope and verify rejection; concurrent/restarted same command never invokes twice. Exhaust the shared allowance across whole-image/crop/Plugin calls, not once per adapter.
4. Assert actual current binding identities in output metadata, actual downstream execution and preserved upstream checkpoint, and no source Published/formal writes. An HTTP response or a completed receipt alone is not evidence of successful classification.

None of this live-binding acceptance is claimed by this boundary delivery.

## Executable dependency regression

Added `replay_binding_substitution_requires_separate_overlay_not_snapshot_rehash` in `crates/annotagent-runtime/tests/published_dag.rs`. A deterministic counting runner executes the original source once. Mutating its frozen model binding fails snapshot integrity; recomputing that hash fails checkpoint identity. Neither failed attempt invokes the runner, and the original checkpoint is unchanged.

Run: `cargo test --offline -p annotagent-runtime replay_binding_substitution_requires_separate_overlay_not_snapshot_rehash`.

Verification: targeted runtime regression **1 passed, 0 failed**; Runtime all-target strict clippy (`-D warnings`) and workspace fmt check passed.
