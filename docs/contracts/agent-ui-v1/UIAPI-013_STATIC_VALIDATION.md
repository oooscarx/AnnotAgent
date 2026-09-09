# UIAPI-013 — owned Draft lookup and static validation

## HTTP adapter contract

`GET /api/workflow-drafts/{draft_id}?project_id={project_id}` returns HTTP 200 with the persisted, unwrapped `WorkflowDraft` (the same object shape returned by PATCH). Use `id`, `project_id`, `revision`, `content_hash`, `nodes`, `edges`, `nodes[].model_binding`, `nodes[].model_profile_binding`, `label_pipeline`, and `annotation_schema` from that object; do not reconstruct a draft from a Published Version. `project_id` is required. A missing draft, missing project or a draft belonging to another project returns 404. This is ownership scoping within the existing local workspace/session security model, not a new user ACL.

`POST /api/workflow-drafts/{draft_id}/validate` accepts only:

```json
{"project_id":"TEST-owner","expected_revision":1}
```

Both fields are required. Unknown fields or invalid JSON field types return 422. Wrong project ownership returns 404 before revision details are exposed. A stale revision returns 409:

```json
{"code":"workflow_draft_revision_conflict","status":409,"draft_id":"TEST-stable-static","project_id":"TEST-owner","expected_revision":2,"current_revision":1,"error":"Workflow Draft changed; reload before validating"}
```

Success is HTTP 200 even when static validation finds blocking issues. Its fields are:

| Field | Exact meaning |
| --- | --- |
| `project_id`, `draft_id` | Actual persisted ownership and stable Draft identity |
| `revision`, `content_hash` | The persisted snapshot checked by this request |
| `validation_kind` | Literal `static` |
| `validation.valid` | Core report: no blocking static issues |
| `validation.issues[]` | Core issues, each `{code:string,path:string,message:string,blocking:boolean}` |
| `validation.execution_order` | Core-computed array of node IDs |

For example the TEST mismatched edge produces this real Core issue (other issues may accompany it):

```json
{"code":"artifact_type_mismatch","path":"nodes[1].inputs.candidates","message":"edge from image.candidates produces BoundingBox, but this port accepts SemanticMask","blocking":true}
```

Read GET, save edits through existing PATCH, then validate the returned persisted revision. Validate does not accept an unsaved graph and does not save anything. It checks one immutable loaded snapshot; another writer may subsequently create a newer revision. Compare returned revision/hash with the editor's current saved revision before displaying the report. Revision mismatch requires reload, not automatic validation of different content. Repeating an unchanged request performs another read-only check and needs no command ID, grant, or approval token. Catalog metadata can change between checks. Catalog/project/schema loading failures remain HTTP 400 through the existing safe API error envelope.

## Execution boundary

This calls the existing Core Workflow static validator, typed Pipeline grammar, label projection, model binding and Geometry Safety checks. Catalog construction uses persisted metadata and the existing non-executing catalog backend. It does not instantiate OpenAI or HTTP Worker clients, resolve credentials, invoke a Provider, invoke or start a Plugin, dispatch sample-test/dry-run, create a Run or sample record, or change Draft/Published Versions/annotations. Recorded model availability is metadata, not a fresh credential/health probe. A valid report is not runtime readiness, successful inference, sample approval or publish permission. Existing execution and publication guards remain unchanged.

`POST .../dry-run` is unchanged and remains an execution endpoint: empty `image_indices` can select images and invoke models. The editor must never use it as static validation.

No database migration, new runtime, frontend types, or production fixture is required. Existing GET/PATCH/direct reference and publication behavior remains intact.

## Isolated regression

`owned_static_validation_is_revisioned_core_only_and_never_executes`: temporary TEST SQLite/projects, owned GET, wrong-owner GET/POST, stale revision409, invalid type422, real artifact type issue, repeated validation, zero connections to an ephemeral loopback listener, malformed Provider/Worker URLs accepted by static validation, unchanged Draft and all Published snapshots, no Run/Sample.

`static_worker_manifest_keeps_recorded_evidence_without_resolving_authentication`: an intentionally invalid TEST authentication reference leaves recorded metadata intact on the static path; runtime authentication checks still reject it.

Run offline in a delivery-only checkout (no server or real workspace required):

```sh
cargo test --offline -p annotagent-application -p annotagent-server
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Verification result: Application unit159 + integration1, Server60 passed (220 total); 3 existing ignored. Full workspace all-target Clippy with `-D warnings`, fmt and diff checks passed. All tests used isolated temporary TEST data; no service was started or updated.
