# UIAPI-014 — frozen version reads and exact native publication

This is an independent increment from the Applied-repair admission fix. Existing processing orchestration already binds a PublicationApproval to its saved sample/model/schema scope, but the native publish route had no caller revision/hash or replay command. The existing Store contains complete immutable PublishedWorkflowVersion JSON. This increment exposes that object and reuses the existing publication transaction; it does not introduce another publication engine.

## Read an actual frozen version

`GET /api/projects/{project_id}/workflows/{workflow_id}/versions/{version}` returns200 with the complete saved version, not a reconstructed current Draft:

```text
{
  workflow_id: string, version: integer, project_id: string,
  source_draft_id: string, content_hash: string, published_at: RFC3339,
  draft: WorkflowDraft,
  snapshot: {
    schema_version: integer, draft: WorkflowDraft|null,
    enabled_skills: object, models: VisionModelDescriptor[],
    model_profiles: ModelProfileSnapshot[], plugin_models: PluginModelSnapshot[],
    prompt_resources: object, safety_compatibility: string
  }
}
```

This is a field map; the real response contains the full objects. `draft.nodes`, `draft.edges`, `draft.label_pipeline`, `draft.annotation_schema`, resource/runtime policies and model bindings are the frozen authoring state at publication. `snapshot.draft` and snapshot model/resource arrays are the frozen execution inputs. `draft.status` is published; snapshot draft status need not equal that status. The top-level `content_hash` hashes frozen snapshot material; `draft.content_hash` hashes Draft content. Do not substitute either for the other.

Wrong Project, absent version or absent Project returns404, without a foreign snapshot. GET does not validate live model availability, initialize history scope, mutate data or execute inference. Existing archived/deleted-but-retained direct references remain readable; genuinely purged rows are absent. Legacy snapshots may have `snapshot.draft:null` or conservative safety compatibility: show the actual legacy object rather than substituting the current Draft. History scope list exclusions do not affect this exact-reference route.

## Publish exactly the confirmed Draft

Reuse `POST /api/workflow-drafts/{draft_id}/publish` with existing same-origin/session/CSRF. The exact native request is:

```json
{
  "command_id": "11111111-1111-4111-8111-111111111111",
  "project_id": "TEST-project",
  "expected_revision": 7,
  "expected_content_hash": "<content_hash from the confirmed Draft GET>"
}
```

Obtain both expected fields from the same owned `GET /api/workflow-drafts/{draft_id}?project_id=P`. Persist this complete body before sending. All four fields are required together; partial exact requests/unknown fields are rejected (422). Empty-body and empty-object calls remain legacy-compatible and do not claim caller CAS or command recovery. The new UI must always use the complete exact request.

First success and exact successful command replay both return200 with the same complete PublishedWorkflowVersion described above, including original version/time/hash. The command namespace is workspace-wide. Scope is exactly command ID + Project + path Draft ID + expected revision + expected content hash. Reusing a command with changed scope returns409, never another version. A lost response is recovered by repeating the original full POST, not by creating a new command or modifying the expectations. Replay is checked before current Draft/model/Sample validation, so it does not re-publish or require an editable old Draft. There is no GET that creates a publication.

Ownership and exact revision/hash are checked before publication work and again at the existing SQLite write boundary. A publication lease excludes concurrent management/editor changes. The transaction atomically inserts version + command result and updates the Draft/Pipeline/current Project default according to existing publication behavior. If saving the command receipt fails, those writes roll back together. Published versions that already exist are never edited.

The exact path uses Core static validation with a metadata-only catalog and the already persisted passing Sample Test. It does not invoke the HTTP dry-run/sample-test endpoint, run a Provider/Plugin inference, start a Run, auto-retry or automatically refresh stale sample evidence. Existing native sample seal, model freeze, safety, reference, lifecycle and publication-readiness guards remain. Preparing an unready Draft or repeating an actual sample requires separate existing user actions.

## Errors for the Adapter

| HTTP | Code / meaning |
|---|---|
|404|Owned Draft/Project/version not found; no foreign object returned|
|409|`workflow_draft_revision_conflict`, with `expected_revision` and `current_revision`|
|409|`workflow_draft_content_conflict`: confirmed hash differs, or freezing current bindings would change the confirmed content|
|409|`workflow_publication_command_conflict`: successful command already has another exact scope|
|409|`workflow_already_published`: this is a different command against an already published Draft; recover the original command or read its version|
|409|`operation_in_progress` / existing lifecycle conflict: another lease prevents publication; no automatic retry|
|400|Existing sample/static/safety/model readiness failure; error text describes the blocker|
|422|Incomplete/unknown/malformed exact request|

Revision conflict example:

```json
{"status":409,"code":"workflow_draft_revision_conflict","error":"Workflow Draft changed; reload before publishing","expected_revision":7,"current_revision":8}
```

Do not treat an uncertain HTTP result as proof of no publication. Only the durable original command replay or exact version read resolves that uncertainty. Replaying a successful publication does not move the Project default back if a later publication has changed it.

## Migration and tests

Additive `0061_workflow_publication_commands.sql` creates a command/request/result table only; no old versions, annotations, grants or history memberships are rewritten. No real workspace migration/publication was performed for this delivery.

Isolated tests use temporary SQLite, TEST drafts, Mock sample preparation, an uncontacted loopback listener and no real credentials:

- Store: actual N→N+1 edit rejects old N; wrong hash/owner write nothing; original command replay survives reopen; changed scope and different-command republish reject; two independent SQLite writers publish one identical result.
- Store: forced command-receipt INSERT failure rolls back version, Draft status and Project default; a subsequent explicit retry can succeed.
- HTTP: owned frozen GET and foreign/missing404; revision/hash409; exact publish and restart replay; editing a separate cloned Draft leaves the old returned version unchanged; no new sample records/Run rows/network connections during publication or reads.

Commands: `cargo test --offline -p annotagent-storage -p annotagent-application -p annotagent-server`; `cargo clippy --offline --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`.

Verification results: isolated Application/Storage/Server suite **417 passed, 3 existing ignored, 0 failed**; final backend branch publication/processing-fence tests **7 passed**; workspace all-target strict clippy and formatting passed. These checks preserve the separately committed repair-admission increment.
