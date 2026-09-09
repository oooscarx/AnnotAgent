# UIAPI-016 — exact, recoverable frozen-version clone

## Existing route, additive exact body

`POST /api/workflows/{workflow_id}/versions/{version}/clone` retains existing authentication/session/CSRF. Native UI sends all three fields:

```json
{
  "project_id": "TEST-project",
  "source_snapshot_hash": "<top-level content_hash from owned frozen version GET>",
  "command_id": "11111111-1111-4111-8111-111111111111"
}
```

Read the source using `GET /api/projects/P/workflows/W/versions/V`. Use the version's **top-level `content_hash`**, not `draft.content_hash`. The command scope includes Project, path Workflow/version, source snapshot hash and command ID. The command namespace is workspace-wide within clone commands.

First success and exact successful replay both return **201**, with the same complete original creation `WorkflowDraft`. Selected response fields (the actual response also includes nodes, edges, bindings and other Draft fields):

```json
{
  "id": "<new stable UUID>",
  "project_id": "TEST-project",
  "status": "editing",
  "revision": 1,
  "content_hash": "<new Draft hash>",
  "name": "<source name> (from v1)"
}
```

Persist the exact command before sending. A lost response is recovered by repeating the original body to the original path. Replay returns the **creation receipt**, not a snapshot of current edits: use its `id` to fetch `GET /api/workflow-drafts/D?project_id=P` for current content. Replay never saves the returned old contents back to the Draft, resets its revision, restores an archived/deleted copy or creates a replacement. If the copy is subsequently unavailable, normal owned GET/lifecycle behavior applies. GET/mount must not dispatch clone.

Legacy empty-body/`{}` calls retain their previous behavior (new Draft each time); they provide no command recovery and must not be used by the native exact flow. Partial exact bodies, unknown fields and malformed UUIDs return422.

## Rejections and atomicity

- Wrong/missing Project or source version in that Project:404 `foreign_project_object` (missing Project uses the existing404 response).
- New command with wrong source hash:409 `workflow_clone_source_conflict`.
- Previously successful command reused with another Project/source/version/hash:409 `workflow_clone_command_conflict`. No original receipt is returned for changed scope.
- Database failure: no partial copy or receipt is committed; preserve an uncertain original command for explicit recovery.

One `BEGIN IMMEDIATE` transaction reads/checks the source, inserts the editable Draft/Pipeline and saves the immutable command receipt. Concurrent writers of the same command return one identity. Receipt INSERT failure rolls back all clone writes. The source Published JSON, existing Pipeline and Project default are unchanged. No Provider/Plugin/model resolution, credentials, inference, sample test or Run is used.

Migration `0062_workflow_clone_commands.sql` adds only the clone command table. It does not rewrite existing versions, annotations, defaults, grants or workspace data.

## Isolated verification

`cargo test --offline -p annotagent-storage -p annotagent-server publication` covers exact clone plus the publication boundary:

- Store: wrong owner/hash; command-scope changes; restart/lost-response replay after an intervening human edit; original Published and default identity/time unchanged.
- Store: two independent SQLite connections return one Draft; forced receipt failure rolls back the Draft before a subsequent explicit command attempt.
- HTTP: owner/hash409/404; creation201; restart/replay201 with identical receipt; edited copy stays edited; changed command scope409. The existing test's no-new-Sample/Run/network checks include exact clone and frozen reads.

TEST temporary databases and deterministic setup only. No real workspace or paid calls.

Verification: publication/clone filter **10 passed**, existing legacy designer HTTP journey **1 passed**, zero failures. Storage/Server all-target strict clippy (`-D warnings`) and workspace fmt check passed.
