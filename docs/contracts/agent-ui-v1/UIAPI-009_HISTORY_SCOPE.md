# UIAPI-009 — persisted history scope (implemented)

This supersedes UIAPI-009_GAP.md. Establishment changes visibility membership only; it does not archive, delete, rewrite references, reset grants or call a Provider. No real workspace scope was established for this delivery.

## Establishment and recovery

All browsers connected to the same workspace SQLite database share one immutable scope.

1. `GET /api/history-scope` is passive: `200 {"scope":null}` before establishment. Mount/GET never establishes a scope.
2. `POST /api/history-scope/preview` with `{"policy":"exclude_existing_history_v1"}` reads a consistent identity snapshot. Example response (counts/hash vary with database):

```json
{"policy":"exclude_existing_history_v1","expected_snapshot_hash":"<sha256>","excluded_counts":{"run":3,"batch":1,"pipeline":2,"workflow_draft":2,"workflow_version":1},"preserves_published_versions":true,"preserves_annotations":true,"preserves_direct_references":true}
```

3. After explicit user confirmation, obtain the existing privileged confirmation for action `POST /api/history-scope` through `/api/session/privileged-confirmation`, and POST:

```json
{"command_id":"11111111-1111-4111-8111-111111111111","expected_scope_revision":null,"expected_snapshot_hash":"<exact preview hash>","policy":"exclude_existing_history_v1","confirmed":true}
```

First creation returns 201; exact replay returns 200, both with the GET shape:

```json
{"scope":{"id":"22222222-2222-4222-8222-222222222222","revision":1,"established_at":"2026-09-10T00:00:00+00:00","policy":"exclude_existing_history_v1","establishment_command_id":"11111111-1111-4111-8111-111111111111"}}
```

IDs/timestamp above are illustrative. Retain the actual response ID. All POSTs require existing same-origin/session/CSRF protection; establishment also requires the existing single-use privileged nonce. Obtain a fresh nonce for a lost-response retry, retain the original command and entire body. A successful replay does not recompute or move membership. GET can recover the established scope read-only. There is no scope update/reset/delete endpoint.

SQLite `BEGIN IMMEDIATE` serializes establishment. Exact command replay is compared before fresh snapshot validation. Errors: 409 `history_scope_snapshot_changed` (refresh preview and confirm the changed scope), `history_scope_command_conflict` (same command, changed body), `history_scope_already_established` (different command, response includes existing `scope`). Invalid confirmation/policy/nonnull initial expected revision is 400; malformed DTO is rejected by the existing JSON extractor. Missing privilege is 403. Scope storage errors use the normal error envelope plus `admitted:false`; do not infer admission from an unknown HTTP outcome.

## Http Adapter mapping

Append `history_scope=<scope.id>` to these existing lists:

| Route | Items | Pagination |
|---|---|---|
| GET `/api/runs?project_id=P` | `runs` | existing `page` |
| GET `/api/batches?project_id=P` | `batches` | existing `page` |
| GET `/api/projects/P/pipelines` | `pipelines` | scoped `page` added |
| GET `/api/projects/P/trash` | `items` | scoped `page` added |

Use `limit` (default50, maximum100), `offset` (default0). `page={total,limit,offset,next_offset}`; null `next_offset` ends pagination. Existing lifecycle filters remain (`include_archived`, `include_deleted` for Pipelines; `kind` for Trash). Ownership, lifecycle and membership filters run in SQL before both count and LIMIT/OFFSET, in one read transaction. Ordering has stable identity tie-breakers. These remain live offset pages, not a cross-request frozen data snapshot; refresh offset0 after mutation.

An explicit scope before establishment yields409 `history_scope_not_established`; a different ID yields409 `history_scope_mismatch`. Never silently fall back to all history. Omitting the scope retains legacy list behavior, including legacy unpaginated Pipeline/Trash wrappers. Workflow Draft and Published Version standalone lists and all direct lookups retain their existing contract; this is not a global object access restriction.

For BOTH `POST /api/projects/P/management/preview` and `/management/actions`, append the same `history_scope` query. Keep the existing request fields, expected object revisions, idempotency key and preview `confirmation_token`. The server binds scope into the confirmation and persisted command. Optional body `history_scope`, if supplied, must equal query (otherwise400). Legacy unscoped confirmation hashes remain unchanged.

Excluded selected objects or affected descendants return409 `history_object_out_of_scope` before mutation, including Batch→Run, Pipeline descendants, and affected default-version references. Scope is rechecked within the execution transaction, before idempotent replay/effects, including cancel-then-trash completion. Existing management errors/blockers remain: wrong-owner preview returns200 with `can_execute:false` and `blockers[].code=foreign_project_object`; confirmed execution returns404. Existing active-run, reference, revision and permanent-delete guards remain authoritative. Management errors keep their existing envelope (do not require `admitted` there).

## Storage and preserved objects

Additive migration `0059_history_scope.sql` creates singleton `history_scopes` and frozen `history_scope_exclusions`. It is independent of already integrated0060; opening an older database applies missing IF-NOT-EXISTS tables without establishing a scope. Membership keys use actual persisted project identity, object kind, stable ID and version. The cut captures all existing Run/Batch/Pipeline/Draft/Version identities including archived/deleted records. Workflow Version count is an additive preview field required for Trash/cascade protection; Published rows themselves remain intact and readable. New versions under an excluded Pipeline remain outside scoped history management.

A later timestamp update/restore cannot expose an excluded identity; a newly created identity with a backdated timestamp remains visible. Existing exact Run/Draft/Published Version/annotation references remain readable under their original owner/lifecycle rules. Unscoped management remains the legacy API and is not restricted by this UI visibility policy. No front-end timestamp filtering or localStorage boundary is needed.

## Isolated verification

Tests use TEST-only temporary SQLite stores, no live workspace/server/model calls:

- `history_scope_passive_replay_restart_and_conflicts`: passive reads, exact replay, changed payload, reopen.
- `history_scope_two_connections_have_one_establishment_winner`: concurrent SQLite connections, one atomic winner.
- `history_scope_membership_pagination_ownership_trash_and_cascades`: before/after equality of all pre-existing tables, snapshot conflict, backdated new identities/updated old identities, owner-safe SQL pages, hidden cascades rejected, confirmation binding, trash/restore and direct Published/annotation references.
- `history_scope_http_confirmation_restart_lists_and_management_fail_closed`: real Router/HTTP handlers and Store, nonce enforcement, stale snapshot/command conflicts, independent Router reopen, scoped pagination/errors, wrong owner, direct Draft GET, scoped preview/action and Trash.

Run `cargo test --offline -p annotagent-storage -p annotagent-server -p annotagent-core -p annotagent-application`; `cargo clippy --offline --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`. No runtime service or port is required.

Validation result: Application/Core/Server/Storage combined **530 passed, 3 existing ignored, 0 failed**; workspace all-target strict clippy passed. The added scope tests use real SQLite and HTTP handlers; no external provider is configured.
