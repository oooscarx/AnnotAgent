# UIAPI-017 — bounded Replay commands and explicit live-binding gap

**Superseded for supported live adapters by the implemented [round 3 contract](UIAPI-017_LIVE_REPLAY.md). The boundary/dependency notes below describe earlier deliveries.**


## Delivery boundary

This increment implements passive owned exact preview and durable no-duplicate command/receipt around the existing sandbox DAG replay. It supports Core and frozen Mock adapters only. **It does not unblock live classifier Replay with current permitted Model Profile/Plugin bindings.** Preview explicitly refuses that scope. Do not remove the old live-binding guard or describe this delivery as full Replay parity.

Existing runtime initializes all frozen profile executions, including ancestors, with one connection context. Current-binding replacement would also change immutable snapshot hashes checked by the DAG checkpoint. There is no existing descendant-only current-binding authorization/credential resolver to reuse unchanged. A bounded follow-up must validate current Provider/Model/Plugin permissions, freeze exact selected descendant bindings, preserve the original checkpoint identity while supplying a separately authorized execution overlay, and apply the existing request allowance before any invocation. Neither saved credentials nor merely changing the recorded Provider name establishes this authority. No new engine or paid test is included here.

## Passive preview

`GET /api/runs/R/replay/N?project_id=P` returns200:

```json
{
  "project_id":"P","source_run_id":"R","node_id":"N",
  "source_record_hash":"...","source_snapshot_hash":"...","checkpoint_hash":"...",
  "image_hash":"...","scope_hash":"...",
  "downstream_nodes":[{"node_id":"N","kind":"vision_model","model_profile_binding":null,"model_binding":"mock-classifier"}],
  "preserved_upstream_nodes":["image"],
  "destinations":{"sandbox":true,"formal_annotations":false,"source_checkpoint_write":false,"published_write":false},
  "limits":{"maximum_model_requests":0,"timeout_seconds":30,"unknown_cost":false},
  "current_bindings":[],"available":true,"refusal_reasons":[]
}
```

The downstream list describes the graph closure, not a promise that every branch executes. The runtime report contains actual executed/preserved nodes. Owner checks require the Run's actual stable Project identity and matching frozen Workflow Project; legacy unattributed Runs cannot acquire ownership from their display name. Missing/foreign/non-replayable source returns404. Reads create no grant, command, model execution or history scope.

Refusal reasons include `current_binding_replay_unsupported`, `source_run_not_terminal`, `checkpoint_snapshot_mismatch`, `legacy_snapshot_draft_mismatch`, `upstream_checkpoint_not_settled`, `source_image_unavailable`. Live Provider, unresolved profile, Plugin/Model Instance binding, or external frozen profile anywhere in the runtime snapshot makes `available:false`; current bindings are not fabricated. Unknown model cost is therefore not authorized in this increment.

## Explicit command and passive receipt

Existing `POST /api/runs/R/replay/N` accepts this complete exact body:

```json
{"project_id":"P","command_id":"11111111-1111-4111-8111-111111111111","scope_hash":"<preview hash>","maximum_model_requests":0,"allow_unknown_cost":false}
```

Persist the complete command first. It returns200 with a durable receipt containing `command_id`, `project_id`, `run_id`, `node_id`, exact `request`, `status`, `started_at`, `completed_at`, `result`, `failure`. Initial status is `running`; the original command is dispatched once after reservation. GET `/api/runs/R/replay/N/commands/C?project_id=P` reads the receipt; no execution. Original POST replay returns the same operation's latest receipt without dispatch, even if the source subsequently changes. Changed command scope returns409 `replay_command_conflict`; stale preview, unavailable scope, nonzero request cap or unknown-cost permission returns409 `replay_scope_not_admitted` with `admitted:false` and current preview, before reservation. Partial/unknown exact fields return422.

On completion, `result` is the existing `NodeReplayReport`: `sandbox:true`, `source_run_id`, `replayed_from`, actual `reexecuted_nodes`, `preserved_upstream_nodes`, and `inspection`. Receipt `completed` means the sandbox returned a report, not accepted annotations or successful classification; inspect node statuses/errors. No formal commit, original checkpoint update or Published write is performed.

Safe failure categories are `scope_changed_before_execution`, `sandbox_execution_failed`, `execution_timeout`, `executor_restarted`. A timeout, execution error or a prior process's unsettled receipt is projected as `outcome_unknown`; no automatic retry/redispatch. The last category is a read-only projection using executor process identity: GET does not rewrite the record. `completed_at` stays null when no terminal receipt was persisted before process loss. Unknown outcomes cannot be resumed by reposting the same command. A new command requires a fresh explicit confirmation.

The exact Application path rechecks the captured source record hash against the actual checkpoint loaded for execution and uses the existing request allowance with a zero external-request cap. Server execution is bounded to30 seconds. Prior empty-body POST remains the legacy synchronous interface, without command recovery; native UI must use the exact body and must never POST on mount/refresh.

## Storage and isolated tests

Additive migration0063 stores Replay command receipts only. SQLite `BEGIN IMMEDIATE` reserves one owner/path/scope per command UUID. Concurrent reservations yield exactly one dispatch; reopened pending commands are not reserved anew. No true workspace data was changed.

Focused tests cover:

- Application: passive exact preview, foreign owner rejection, stale checkpoint rejection, live-model compatibility refusal, actual sandbox Replay preserving source checkpoint/upstream and formal annotations.
- HTTP: passive preview, stale POST409, owned command/GET receipt, polling to actual report, exact POST replay, changed command409; existing Inspector/export assertions preserve formal results.
- Storage: two connections reserve one command, reopen never redispatches, foreign reads/changed scope reject, completion receipt replay.
- Server projection: prior process's unsettled receipt becomes outcome_unknown without mutation.

All fixtures are TEST-only/Mock and temporary SQLite; no real credentials, inference, services, frontend changes or remote operations.

Verification: focused Application **1 passed**, Server **2 passed**, Storage **6 passed** (9 distinct tests; 0 failed). Commands: `cargo test --offline -p annotagent-application published_label_pipeline_executes_and_persists_typed_checkpoint` and `cargo test --offline -p annotagent-storage -p annotagent-server replay_`. Strict all-target Application/Storage/Server clippy and workspace fmt check passed.
