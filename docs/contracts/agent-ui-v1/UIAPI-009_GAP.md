# UIAPI-009 — audited gap; proposed contract, NOT implemented

Status: **history cutover remains blocked**. This delivery is the requested concrete gap specification, not an executable scope API. No GET/POST below should be wired as if already available. No real workspace was read or changed, and no historical rows were hidden/deleted by this audit.

## Actual capabilities

- GET `/api/runs?project_id=P&limit=N&offset=N`: existing project-owner filter and SQL pagination, default limit50/max100, excludes deleted rows. `page={total,limit,offset,next_offset}` and `runs[]`. Order is `updated_at DESC,id DESC`, not a creation boundary. Source: server `list_run_summaries/product_runs`, storage `summary.rs::list_run_summaries`.
- GET `/api/projects/P/pipelines?include_archived=bool&include_deleted=bool`: `{pipelines:[]}`; project-owned lifecycle records, currently unpaginated. Storage `management.rs::list_project_pipeline_lifecycle` selects `workflow_pipelines`, orders updated_at/ID.
- GET `/api/projects/P/trash?kind=run|batch|workflow_draft|workflow_version|pipeline`: `{items:[]}`, currently unpaginated. Includes several distinct identity tables; it is not merely a Run list. Application management_scope resolves the existing stable project identity and route ownership.
- Existing management preview/execute enforce exact project, object revisions, confirmation and active/reference safety. They do not accept a history-cutover scope or reject out-of-scope targets for a new history view.
- Existing direct Run, workflow/draft/version, annotation and task-object references must remain usable under their existing ownership/availability rules. They are not equivalent to list visibility.
- Migration0015 named workspace_identity concerns project-scoped image identity; it does **not** establish a history epoch. Settings revision, conversation journal sequence, Project creation timestamps and browser storage are not workspace-wide history boundaries.

No existing persisted API safely supplies the requested one-time cross-browser cutover. Filtering by updated_at is incorrect when old records change; filtering one fetched page corrupts totals/pagination; setting deleted_at/archived_at would alter real lifecycle state and is not a visibility-only cutover.

## Proposed precise API (requires backend implementation)

1. GET `/api/history-scope` -> 200 `{scope:null}` before establishment, otherwise `{scope:{id:UUID,revision:1,established_at:RFC3339,policy:"exclude_existing_history_v1",establishment_command_id:UUID}}`. Passive read; never initializes or migrates a cutover. Same local database returns the same scope to every browser.
2. POST `/api/history-scope/preview` with `{policy:"exclude_existing_history_v1"}` -> `{policy,expected_snapshot_hash,excluded_counts:{pipeline,workflow_draft,run,batch},preserves_published_versions:true,preserves_annotations:true,preserves_direct_references:true}`. Uses existing session/CSRF. Read-only storage operation; counts/identity hash are computed from one consistent snapshot. No model or lifecycle mutation.
3. POST `/api/history-scope` with `{command_id:UUID,expected_scope_revision:null,expected_snapshot_hash:SHA256,policy:"exclude_existing_history_v1",confirmed:true}`. Explicit user action after reviewing preview. Same-origin/session/CSRF and the existing privileged nonce mechanism bound to this exact action. Transaction checks the preview snapshot, creates one workspace scope and its frozen excluded identities atomically. Returns the GET shape, status201 first creation, 200 exact command replay. Same command changed payload ->409 `history_scope_command_conflict`; concurrent different command after establishment ->409 `history_scope_already_established` plus existing scope; snapshot changed ->409 `history_scope_snapshot_changed`, admitted:false. No overwrite/reset/update/delete scope API. A lost-response retry never moves the boundary.
4. Reuse existing list endpoints with `history_scope=UUID`. Validate this workspace's established scope first; missing establishment ->409 `history_scope_not_established`; unknown scope ->409 `history_scope_mismatch`. Unscoped callers retain existing behavior. New UI must explicitly request the established ID and must not fall back to all-history on an error.
5. Scoped Pipeline and Trash endpoints add `limit/offset` and `page` with the same default50/max100 rules as Runs, retaining existing wrapper keys. Counts and row pages apply project/lifecycle/scope filters **before** LIMIT/OFFSET in one read transaction; order uses existing updated_at plus stable identity tie-breakers. Scope stays immutable; normal offset pages are still live lists, not frozen snapshots across concurrent writes. Refresh from offset0 after a mutation.
6. Scoped management preview/execute use the same `history_scope` query on both requests and include it in the preview confirmation binding. Reject a wrong workspace scope or any selected/effectively affected excluded object with409 `history_object_out_of_scope` before a write. Existing project, revision, active-run, references and permanent-delete guards stay authoritative. No bulk action may operate on hidden rows just because an older preview/token named them. Direct-reference views are not filtered or silently redirected; an explicit unscoped management action remains governed by the old contract, outside the new scoped history UI.

## Persisted membership and invariants

Use an additive singleton scope table plus frozen exclusion membership keyed by scope, actual stable project identity, object kind and identity (including version where applicable). Capture existing Pipeline/Draft/Run/Batch IDs at establishment in a single SQLite write transaction; do not use clocks or mutable updated_at to decide membership. Objects created after the transaction remain visible even if their caller-supplied timestamps are old. Old objects stay excluded after updates, restore or a browser restart. A new explicit copy has a new identity; copying does not rewrite the original identity or grant.

Published Versions, annotations, source files, grants, budgets and links are not modified. Published Version lookup/listing outside the scoped Pipeline history remains unchanged. Hiding an old Pipeline in the new list must not remove its published versions or prevent a current task from resolving its exact source. A new version under an excluded old Pipeline does not implicitly make that Pipeline a newly created history item; creation of a new Pipeline identity must remain an explicit existing operation.

Trash list membership uses the same frozen identity test as ordinary lists, irrespective of when an old object was trashed/restored. Scope establishment itself never moves anything into Trash. Cascades involving excluded descendants must be refused by scoped actions rather than silently deleting hidden history. No retroactive task/reference rewrites.

## Required isolated acceptance tests (not yet implemented/run)

- Fresh GET and repeated GET cause no scope/exclusion writes; two clients see null.
- Exact confirmed POST replay and close/reopen retain identical ID/revision/membership; concurrent establishment has one winner; changed command or snapshot conflicts.
- Old/new Run, Batch, Pipeline and Draft data around the atomic cut; intentionally backdated new rows remain visible, old updated rows remain hidden.
- Scoped count/offset pagination excludes old rows before page selection; project A never returns or mutates project B; missing/foreign scope fails closed.
- Trash and restore do not change scope membership; no deletion/archival occurs at cutover; scoped cascades cannot affect hidden predecessors.
- Current task's exact old Run/Draft/Published Version/annotation references still resolve unchanged. Direct lookups and unscoped API results are unchanged.
- Grants/call budgets, publication rows, annotation bytes/reference IDs and run events compare unchanged before/after establishment; no Provider invocation.

This audit ran only source/ref inspection and documentation diff checks. No isolated runtime tests are claimed because there is no implementation in this delivery. A frontend-only timestamp or localStorage workaround does not close this gap.
