# UIAPI-015 — native version/publication integration handoff

This issue is renamed from the earlier publication-only UIAPI-014 to avoid confusion with localization repair. The executable backend delivery is **7c7c8df0ff234477aff67ae48ab53b6ef91897bd**, independently cherry-pickable. Do not reimplement the existing processing publication engine. The native exact publication path reuses storage publication and the existing readiness/safety boundaries.

## Actual APIs

- `GET /api/projects/P/workflows/W/versions/1`: 200 full immutable `PublishedWorkflowVersion`, including `project_id`, `workflow_id`, `version`, `source_draft_id`, `content_hash`, `published_at`, complete `draft` and complete `snapshot`. Wrong owner/missing returns404. History exclusions do not hide exact references. Read the returned snapshot, never the current editable Draft.
- `GET /api/workflow-drafts/D?project_id=P`: obtain `revision` and Draft `content_hash` from the same confirmed object.
- `POST /api/workflow-drafts/D/publish`: explicit exact request below, with existing session/CSRF. 200 first success and successful exact replay both return the same complete PublishedWorkflowVersion as version GET. Empty-body legacy calls lack caller CAS/recovery and must not be used by the native confirmation flow.

```json
{
  "command_id": "11111111-1111-4111-8111-111111111111",
  "project_id": "TEST-project",
  "expected_revision": 7,
  "expected_content_hash": "<Draft.content_hash from confirmed owned GET>"
}
```

Persist this entire command before POST. Recover a lost success response by resending this identical body to the identical Draft path; never generate a replacement command on refresh. GET/mount does not publish. Both revision and hash bind the exact confirmation. A changed revision returns:

```json
{
  "status": 409,
  "code": "workflow_draft_revision_conflict",
  "error": "Workflow Draft changed; reload before publishing",
  "expected_revision": 7,
  "current_revision": 8
}
```

Other errors: wrong owner404; hash mismatch409 `workflow_draft_content_conflict`; reused successful command with changed scope409 `workflow_publication_command_conflict`; another command for the already-published Draft409 `workflow_already_published`; incomplete/unknown exact fields422. Existing readiness and lifecycle rejection still apply. Original success replay works after restart and does not move the default back after a later publication.

The full field map, snapshot-vs-Draft hash distinction, additive0061 migration and readiness behavior are in [the original publication contract](UIAPI-014_PUBLICATION.md). Publication uses saved Sample evidence and static checks; no inference, sample rerun or Run is started.

## Existing clone contract and explicit limitation

`POST /api/workflows/W/versions/1/clone` with no body returns201 with a complete new `WorkflowDraft`: new stable UUID, the version's original `project_id`, `status:"editing"`, `revision:1`, recomputed content hash and frozen authoring contents copied into the editable Draft. It leaves the Published version unchanged and performs no inference. This is an explicit user action only.

This legacy clone route has **no caller project-owner parameter, command ID or atomic lost-response recovery**. It derives the destination Project from the stored immutable version. Repeating POST creates another copy. The owned version GET can verify the selected source before presenting the action, but does not turn legacy clone into an owned/idempotent command. Do not retry uncertain clone responses or dispatch clone on mount. The publication commit does not claim to close that separate clone recovery gap.

## Reverified isolated tests

`cargo test --offline -p annotagent-storage -p annotagent-server publication`: **8 passed, 0 failed** (1 HTTP test + 7 storage/processing-publication tests).

Coverage includes wrong owner, actual intervening revision update, wrong hash, changed command scope, identical two-writer result, receipt-write rollback, reopen/lost-response replay, full frozen GET, separate clone edits preserving the original snapshot, and no new Sample/Run/network during exact publication. Temporary TEST SQLite and deterministic setup only; no real workspace, keys, paid calls or service restart.
