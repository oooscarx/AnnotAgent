# UIAPI-017 round 3 — implemented current-binding execution overlay

This is the executable follow-up to `UIAPI-017_OVERLAY_DEPENDENCY.md`. The previous blanket live-binding refusal is removed for the bounded adapters below. Original immutable Workflow/checkpoint validation and existing command recovery remain in force. No new executor, migration, installation or historical credential recovery is introduced.

## HTTP adapter delta

Read `GET /api/runs/R/replay/N?project_id=P&bindings=<URL-encoded JSON object>`.

`bindings` is optional. It maps exact descendant node IDs to current Registry selections, for example:

```json
{
  "scene.classifier":"model-profile:11111111-1111-4111-8111-111111111111",
  "object.segmenter":"model-instance:22222222-2222-4222-8222-222222222222"
}
```

Omitting a node uses its declared Registry identity resolved at the **current** enabled revision/installation, not the historical frozen execution settings. Invalid selections, upstream/core nodes, unavailable or capability-incompatible models refuse the entire scope. Explicit Mock selections are not a mechanism for enabling a production Registry Mock model.

Preview retains source record/snapshot/checkpoint/image hashes, graph closure, preserved upstream IDs and sandbox destinations. It now returns `binding_selections`, `current_bindings` and these limits:

```json
{
  "limits": {
    "maximum_model_requests":12,
    "timeout_seconds":30,
    "unknown_cost":true,
    "parallel_model_nodes":1,
    "provider_retries":0,
    "http_redirects":false
  },
  "current_bindings":[{
    "node_id":"scene.classifier",
    "selection":"model-profile:11111111-1111-4111-8111-111111111111",
    "binding_digest":"<current snapshot + destination/permission digest>",
    "destination":"http://127.0.0.1:TEST_PORT",
    "permissions":{"connection_policy":{}},
    "capability":"image_classification"
  }],
  "available":true,
  "refusal_reasons":[]
}
```

The example permission object is abbreviated; actual preview includes the current policy. The safe destination label displays the origin; the digest binds the exact current endpoint, current model snapshot, routing/organization/workspace/safe-header/credential-reference metadata and permissions. It never contains credential bytes. Plugin digests bind the verified installed model/package/assets and manifest permissions. GET does not resolve credentials or start a worker.

For external models, exact POST `/api/runs/R/replay/N` is:

```json
{
  "project_id":"P",
  "command_id":"33333333-3333-4333-8333-333333333333",
  "scope_hash":"<complete preview scope_hash>",
  "bindings":{"scene.classifier":"model-profile:11111111-1111-4111-8111-111111111111"},
  "maximum_model_requests":1,
  "allow_unknown_cost":true
}
```

Repeat exactly the same optional `bindings` selection used for preview. External execution requires an explicit limit1–12 and `allow_unknown_cost:true`; Core/Mock-only scope keeps limit0/false. The actual requested limit is part of the persisted exact command even when lower than the preview ceiling. Stale hashes, revoked/disabled bindings, unavailable scope and incompatible limits refuse with409 `replay_scope_not_admitted`, `admitted:false`, before command reservation or credential resolution.

Preview refusal codes include `multiple_current_providers_unsupported`, `model_operation_unsupported`, `current_binding_unavailable_or_unsupported`, and the previous source/checkpoint/ownership/availability guards; `snapshot_integrity_mismatch` now explicitly rejects corrupt snapshot material during preview.

## Receipt and actual execution

First dispatch remains200 with the existing durable command receipt. New receipts persist the full preview in `authorized_scope`, including per-node permission digests. GET `/api/runs/R/replay/N/commands/C?project_id=P` remains passive. Same-scope POST returns the original command's latest receipt without another dispatch, even after current Model/Profile/Provider revocation. Changed command scope returns409. Earlier receipts without `authorized_scope` remain readable.

Completed `result` retains the actual `NodeReplayReport` and adds `execution_bindings` and `scope_hash`. Render source configuration separately from the current execution bindings: the source inspection describes the original Workflow; this does not mean the old model produced the new result. Inspect node status/error/output; receipt completion alone is not successful classification or formal acceptance.

A current credential is resolved transiently through the existing SecretStore only after durable admission and permission recheck. No credential is read from Run history or saved in the receipt. Provider initialization uses only the approved descendant snapshots. The immutable Workflow is restored as the DAG's source before replay; an adapter wrapper supplies ephemeral binding-only node copies. Topology, parameters, labels, geometry policies, gates, retry/fallback and destinations remain source-owned. No source hash is rewritten.

The existing shared `SampleCalls` allowance now has a Replay guard. It revalidates current scope/permission digests and deadline before worker startup and immediately before each Provider/backend invocation, then atomically reserves the common numeric allowance. Model nodes execute serially within the Replay. Provider automatic HTTP retries and redirects are disabled; current safe routing headers and the current Provider timeout (capped at30 seconds) are used. Overlay adapter caching is disabled for the isolated executor, preventing reuse under an old binding identity.

A permission change between calls rejects the next invocation. Existing timeout/process-loss/unknown-outcome semantics and no-redispatch guarantee are unchanged. Pre-execution credential/provider failures use safe `current_credential_unavailable` / `current_provider_unavailable` categories. No automatic retry or invented successful resume is introduced.

## Supported scope and limitations

- Current Model Profiles: classification and object/VLM detection through the existing OpenAI-compatible adapters, with enabled/current image capability checks.
- Installed Plugin/Model Instance bindings: classification, object/VLM detection and prompted segmentation through the existing Plugin pipeline adapters, requiring matching capability and verified current installed assets/permissions. No install or download occurs.
- One current Provider connection per Replay; installed Plugin bindings may coexist with that Provider. Mixed Provider connections refuse explicitly.
- Grounding/recovery-agent/YOLO and other model-operation families outside these adapters refuse with `model_operation_unsupported`; no generic compatibility is claimed.
- Source must have a valid owned checkpoint/image, settled upstream scope and compatible source graph. Mandatory Review and geometry gates remain. Formal annotations, source Run/checkpoint, Published snapshots and Project default are not changed.
- Thirty-second execution window and at most12 shared model requests. This is a request-count authorization with unknown monetary cost explicitly accepted, not a new price ledger.

## Actual isolated verification

The existing server HTTP test now starts a loopback Axum OpenAI-compatible endpoint with deterministic synthetic tool output. A separate current Registry Profile and session-only TEST credential are selected for a frozen source classifier. Actual HTTP preview → exact POST → GET receipt produces a successful classifier node with exactly one counted HTTP model request. Reposting the original command adds zero requests.

Changing the enabled model's semantic revision rejects its old preview with zero additional requests. Disabling the Provider after a fresh preview also rejects before invocation. The original successful command still recovers after revocation. Original checkpoint JSON, Published JSON and formal Annotation objects remain identical. The fixture also asserts the actual request uses the current TEST credential and selected remote model name; existing export assertions still pass.

The shared adapter regression calls two existing pipeline wrappers: permission revocation after the first invocation prevents the next; restoring permission permits only the remaining numeric allowance, then exhaustion prevents any further invocation. Runtime source-hash substitution refusal and durable reservation/restart regressions remain in the suite.

No commercial call, real workspace/key, service restart, frontend edit, model install or push was performed. The actual loopback acceptance is classification; it does not claim hardware Plugin/SAM quality or a newly installed model test.


Verification results: Application/Server/Runtime/Provider full suites **323 passed, 3 existing ignored, 0 failed**; durable Storage command regression **1 passed**. The final loopback HTTP proof passed again after adding credential/model-name and full Annotation equality assertions. Strict all-target clippy and workspace fmt check passed.

Reproduction commands:

```sh
cargo test --offline -p annotagent-server label_pipeline_http_advisor_dry_run_inspector_and_replay_are_real
cargo test --offline -p annotagent-application replay_guard_rechecks_revocation_before_every_adapter_call
cargo test --offline -p annotagent-storage replay_command
cargo test --offline -p annotagent-application -p annotagent-server -p annotagent-runtime -p annotagent-provider
```
