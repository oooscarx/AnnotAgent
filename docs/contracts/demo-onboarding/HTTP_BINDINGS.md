# AnnotAgent Demo onboarding HTTP bindings

Audit baseline: `8a7d8007904a00af62f3b179c216b8a3b2732a1d`.
Common ancestor with the earlier mainline backend work is
`d3220cb54bd50589efed86b6c0753bf4ea8db0db`. This document distinguishes APIs
already present at the baseline from the bounded Demo additions being delivered.
`GET` never creates a Project, probes a Provider, imports candidates, grants a call,
starts inference, resumes work, accepts annotations, or admits a package.

## Existing mainline APIs reused unchanged

Let `T=/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}`.

| Capability | Existing API and authoritative response | Boundary |
|---|---|---|
| Task read model | `GET T/workspace` -> `workspace.mainline: MainlineTaskView` | `completion.status=package_ready` is the only whole-task completion. A completed model call is only a step receipt. |
| Readiness | `GET T/capability-readiness` -> `CapabilityReadiness` | Passive Registry snapshot. `setup_requests[].return_path` is server-owned. |
| Formal source | `GET T/formal-result`; `GET T/delivery-review-items`; `GET T/delivery-images/{image_id}?source_run_id=...` | Sources remain exact Task processing operation -> Batch -> child Run. Project-global Runs are excluded. |
| Human review | `POST T/delivery-images/{image_id}`, `/objects`, `/missing-objects` | Existing image/snapshot/review CAS; no candidate is accepted by import or GET. |
| Package | `GET/POST T/delivery-package-consents`; `GET/POST T/delivery-packages` | Readiness and the one-shot admission transaction remain server-owned. |
| Stop and events | existing conversation stop requests, Run/Batch controls and Run SSE | Remote uncertainty remains `in_doubt`/unknown; no automatic retry. |
| Context archive | existing context-archive export, import preview/confirm/receipt and archived-context GET/list | Archive-only import creates inert IDs and never restores grants, queue dispatch, tools or live Tasks. No Demo extension is planned. |
| Model Profile CAS | `GET /api/model-profiles/{id}`; `PATCH /api/model-profiles/{id}` with `expected_revision` | `limits`, `generation_defaults`, `pricing` are already persisted and revisioned. Old Published snapshots remain immutable. |

## R1 DemoCatalog — delivery contract

| Method and URL | Response |
|---|---|
| `GET /api/demo-catalog?cursor=&limit=50` | `DemoCatalogPage {catalog_revision,items:[DemoCatalogEntry],next_cursor}` |
| `GET /api/demo-catalog/{demo_id}/versions/{version}` | Exact allowlisted `DemoManifest`; no local path fields |
| `GET /api/demo-catalog/{demo_id}/versions/{version}/assets/{asset_id}` | Bytes for an asset named and hashed by that exact manifest; immutable ETag is the manifest SHA-256 |

Catalog sources are repository-owned `examples/demo-packs/{demo_id}/{version}`
directories compiled/validated at startup. IDs use lowercase ASCII letters, digits and
hyphens; version is an exact SemVer string. The loader rejects symlinks, traversal,
unknown fields, duplicate IDs, hash/size/mime mismatches and assets outside the pack.
The production Registry never auto-discovers TEST or Mock packs.

## R2 StartDemo — delivery contract

`POST /api/demos/start` uses the existing local session/CSRF boundary:

```json
{
  "command_id":"UUID",
  "demo_id":"object-detection-review",
  "demo_version":"1.0.0",
  "source_mode":"preset_candidates",
  "model_profile_id":null
}
```

`source_mode` is exactly `preset_candidates|live_model`. A Model Profile ID is
required only for `live_model` and is forbidden for `preset_candidates`. First
success returns 201 `StartDemoReceipt`; exact replay returns 200 with the same IDs.
Reusing a command with changed Demo/version/mode/model returns 409
`demo_command_conflict`. `GET /api/demos/start/{command_id}` recovers the receipt and
never starts work.

The transaction allocates a new independent Project owner, Conversation, Task,
imports exact manifest images, saves delivery intake and records source provenance.
It then exposes existing mainline actions. Preset candidates enter a dedicated import
source with `review_status:needs_review`; they are not a Mock Provider call, do not
create model usage and are never marked human accepted. Live mode creates no preset
candidate rows and must pass current readiness/authorization before inference.

## R3 effective model request — implemented

Model Profile create/PATCH/GET persists revisioned `limits`, `generation_defaults`
and `pricing`. `generation_defaults` now also accepts
`reasoning_wire_parameter:"reasoning_effort"|"enable_thinking"` and
`supported_reasoning_modes:string[]`. A configured mode must belong to the declared
set. `enable_thinking` accepts only `enabled|disabled`; any reasoning setting requires
`protocol_features.reasoning_controls=true`.

`GET /api/model-profiles/{id}/effective-request` is passive and returns:

```json
{
  "model_profile_id":"UUID","model_profile_revision":7,
  "provider_id":"UUID","provider_adapter":"open_ai_compatible",
  "endpoint_summary":"https://provider.example/v1","remote_model_id":"TEST-model",
  "context_tokens":32768,"requested_maximum_output_tokens":4096,
  "effective_maximum_output_tokens":2048,"maximum_input_context_tokens":30720,
  "temperature":0.1,"top_p":"0.9","structured_output_mode":"tool",
  "image_detail":"high","system_prompt_version":"demo-v1",
  "reasoning":{"requested_mode":"enabled","supported_modes":["disabled","enabled"],
    "support_known":true,"wire_parameter":"enable_thinking","wire_value":true},
  "pricing_snapshot":{"model_profile_id":"UUID","model_profile_revision":7,
    "pricing":{"currency":"USD","input_per_million_tokens":"2",
      "output_per_million_tokens":"8","cached_input_per_million_tokens":null,
      "per_image":null,"per_request":null,"source":"user_configured",
      "updated_at":"RFC3339"},"captured_at":"RFC3339"},
  "snapshot_sha256":"64-lowercase-hex"
}
```

The request assembler caps output tokens by the Model limit, reserves that output
budget from the context window before Builder compaction, maps `top_p`, and maps the
reasoning control to exactly one Provider field. `enable_thinking` is a boolean and
is never also emitted as `reasoning_effort`. The effective snapshot is frozen before
an admitted call; editing a Profile cannot relabel an in-flight attempt.

## R6 physical attempt usage — implemented for conversation text Providers

`GET T/model-usage?cursor=0&limit=50` returns an owner-checked page. Cursor is the
exclusive integer `sequence`; limit is `1..100`.

```json
{
  "scope":{"project_id":"P","conversation_id":"UUID","task_id":"UUID"},
  "state":"complete",
  "summary":{"attempt_count":1,"known_cost":"0.007","currency":"USD",
    "costs_by_currency":[{"currency":"USD","cost":"0.007"}],
    "input_tokens":1500,"cached_input_tokens":0,"output_tokens":500,
    "token_unknown_attempt_count":0,"unknown_cost_attempt_count":0},
  "attempts":{"items":[{
    "sequence":1,"attempt_id":"UUID","call_id":"UUID","attempt_number":1,
    "kind":"task","status":"succeeded","model_profile_id":"UUID",
    "model_profile_revision":7,"provider_id":"UUID","provider_name":"TEST Provider",
    "request_id":"TEST-request","input_tokens":1500,"cached_input_tokens":0,
    "output_tokens":500,"image_count":0,"usage_source":"actual",
    "cost":"0.007","currency":"USD",
    "pricing_snapshot":{"model_profile_id":"UUID","model_profile_revision":7,
      "pricing":{"currency":"USD","input_per_million_tokens":"2",
        "output_per_million_tokens":"8","cached_input_per_million_tokens":"1",
        "per_image":null,"per_request":null,"source":"user_configured",
        "updated_at":"RFC3339"},"captured_at":"RFC3339"},
    "effective_request":{"snapshot_sha256":"64-lowercase-hex","runtime_request":{}},
    "started_at":"RFC3339","completed_at":"RFC3339","duration_ms":4,
    "failure":null
  }],"next_cursor":null}
}
```

`GET T/model-usage/attempts/{attempt_id}` returns one exact owned row. Status is
`started|succeeded|failed|in_doubt`; failure is the safe typed `ModelFailure` and
never contains a raw Provider body. Each physical OpenAI-compatible HTTP attempt is
inserted before transport. Retries share the admitted `call_id` and have distinct
monotonic `attempt_number` and `attempt_id`. Logical call settlement remains in the
existing conversation ledger.

Cost uses Decimal values from the attempt's nested price snapshot: ordinary input,
reported cached input, output, image count, and per-request components. The fixture
case is `1500*2/1_000_000 + 500*8/1_000_000 = 0.007`. Missing tokens, missing cached
count, or a required price yields `cost:null`; it is never coerced to zero. Summary
token totals are null when any included attempt lacks that token count. Mixed
currencies remain separate in `costs_by_currency`, with top-level cost/currency null.
States are `no_model_requests|complete|partial|unknown`.

The OpenAI-compatible Schema, Builder, queued planning, feedback, and future-rule
routes install this observer after their existing authorization checks; the outer
conversation adapter supplies the reserved call identity without serializing it to
the Provider. Preset candidates produce `no_model_requests` and zero rows. Active
Provider probes retain their separate Model Profile usage list. Published/sample
vision Provider attachment and the explicit confirmed TEST request remain tracked
acceptance gaps; those paths must not claim complete R6 evidence yet.

## Errors and compatibility

- 404 hides foreign/missing Project, Task, Demo, command and attempt ownership.
- 409 covers command reuse, stale profile/scope/hash and budget conflicts.
- 422 covers unknown fields and typed DTO failures; invalid pack content prevents
  startup/catalog admission and is never partly imported.
- 429/capacity and configured call limits reject before a physical attempt row is
  created. Once transport may have received bytes, outcome may be unknown.
- Additions are versioned and additive. Existing Task, archive, review, Run and
  package URLs are reused rather than wrapped in a Demo-only Runtime.

