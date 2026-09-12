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

## R3 effective model request — existing fields and additions

Existing `ModelProfile` wire fields are:

```json
{
  "revision":7,
  "limits":{"context_tokens":32768,"maximum_output_tokens":2048,
    "maximum_images_per_request":3,"maximum_image_pixels":12000000},
  "generation_defaults":{"temperature":"0.1","top_p":"0.9",
    "maximum_output_tokens":1024,"structured_output_mode":"tool",
    "reasoning_mode":"medium","image_detail":"high",
    "system_prompt_version":"demo-v1"},
  "pricing":{"currency":"USD","input_per_million_tokens":"2",
    "output_per_million_tokens":"8","cached_input_per_million_tokens":null,
    "per_image":null,"per_request":null,"source":"user_configured",
    "updated_at":"RFC3339"}
}
```

Model Profile create/PATCH/GET already persists those values. Builder configuration
already derives max output, temperature and context compaction from the selected
revision and freezes model/provider/config in its authorization digest. Published
Run setup already derives max output, temperature and reasoning from the frozen
`ModelProfileSnapshot`.

Delivery adds passive `GET /api/model-profiles/{id}/effective-request` and an explicit
confirmed TEST request/receipt. The response carries requested/effective values,
Provider-specific mapped fields, supported/rejected reasoning mode, context policy,
model/profile revision and a redacted digest. In-flight calls keep their frozen
snapshot when the Profile is edited. No prompt, credential, raw response or hidden
reasoning is exposed.

## R6 attempt usage — delivery contract

`GET T/model-usage?cursor=&limit=50` returns:

```json
{
  "scope":{"project_id":"P","conversation_id":"UUID","task_id":"UUID"},
  "state":"complete",
  "summary":{"attempt_count":1,"known_cost":"0.007","currency":"USD",
    "input_tokens":1500,"cached_input_tokens":0,"output_tokens":500,
    "unknown_attempt_count":0},
  "attempts":{"items":[{"attempt_id":"UUID","call_id":"UUID",
    "attempt_number":1,"kind":"task","status":"succeeded",
    "model_profile_id":"model-profile-id","model_profile_revision":7,
    "provider_id":"provider-id","provider_name":"TEST provider",
    "request_id":"TEST-request","input_tokens":1500,"cached_input_tokens":0,
    "output_tokens":500,"usage_source":"actual","cost":"0.007",
    "currency":"USD","pricing_snapshot":{"input_per_million_tokens":"2",
      "output_per_million_tokens":"8","cached_input_per_million_tokens":null,
      "per_request":null,"captured_at":"RFC3339"},"started_at":"RFC3339",
    "completed_at":"RFC3339","duration_ms":4,"failure":null}],
    "next_cursor":null}
}
```

Every physical OpenAI-compatible HTTP attempt gets a durable row before/after the
attempt. Retries have distinct `attempt_id` and `attempt_number`. Unknown tokens or
price produce `cost:null`, never zero. Failed/in-doubt attempts remain visible;
cached input is priced separately when reported. Mixed currencies are returned as
per-currency summary buckets and never added. Pricing is a Decimal string snapshot
from the exact Model Profile revision and is never recomputed after edits. States are
`no_model_requests|complete|partial|unknown`. Owner checks and SQL pagination precede
the limit. `GET T/model-usage/attempts/{attempt_id}` returns one exact owned row.

Active Provider probes remain `kind:probe` under Model Profile usage and do not count
as Task usage unless a new explicitly Task-bound test command says so. Preset mode
returns `no_model_requests`, zero attempts and known zero tokens; it never fabricates
a zero-cost model attempt.

## Errors and compatibility

- 404 hides foreign/missing Project, Task, Demo, command and attempt ownership.
- 409 covers command reuse, stale profile/scope/hash and budget conflicts.
- 422 covers unknown fields and typed DTO failures; invalid pack content prevents
  startup/catalog admission and is never partly imported.
- 429/capacity and configured call limits reject before a physical attempt row is
  created. Once transport may have received bytes, outcome may be unknown.
- Additions are versioned and additive. Existing Task, archive, review, Run and
  package URLs are reused rather than wrapped in a Demo-only Runtime.

