# UIAPI-010 Model Profile atomic edit

This ticket concerns Model Profile concurrency, separately from the earlier Schema output-tool compatibility ticket.

## HTTP adapter contract

GET `/api/model-profiles/:id` remains passive and returns `{ "model": ModelProfile, "revisions": [...] }`. Read `model.revision` for the edit baseline.

PATCH the same URL with editable fields plus optional `expected_revision`:

```json
{"expected_revision":1,"display_name":"Renamed model"}
```

Success: HTTP 200, the ModelProfile itself (no `model` envelope), with `revision: 2`. Every successful HTTP PATCH, including metadata-only and no-op edits, appends one revision. Quality contracts bind to that new revision. Semantic changes still invalidate verification; renaming alone preserves status. Previously frozen Published Versions and old revisions are not rewritten.

Stale baseline: HTTP 409:

```json
{"status":409,"code":"model_profile_revision_conflict","expected_revision":1,"current_revision":2,"error":"Model Profile changed; reload before saving","suggested_action":"reload_model_profile"}
```

The conflict contains no credential, provider response or full profile. `current_revision` is the revision observed at conflict detection, not a promise it remains current. Reload and let the user reconcile edits; do not automatically retry with the new revision. Repeating a successful request with the old expected revision returns 409 (not a second write).

`revision` is server-owned and still rejected as an unknown request field (422). Omitted/null `expected_revision` remains accepted for compatibility, but cannot protect edits made from an older browser snapshot. Even these requests compare their handler-read revision atomically at persistence and may return 409 if another writer wins during the request. Existing authentication, origin/CSRF and ownership requirements are unchanged.

## Persistence and boundaries

SQLite BEGIN IMMEDIATE protects latest-revision read, comparison and insert in one transaction, across independent connections. Both competing PATCH writers cannot commit revision N+1. No SQL migration, new endpoint, grant reset or data conversion is required.

The existing internal `save_model_profile` API retains its semantic-revision rules for probe/health metadata. This change provides CAS for HTTP editor writes; it does not introduce a universal metadata/event revision or change probe/runtime APIs. Existing frozen model selections remain pinned to their revision.

## Isolated verification

- Storage: two independent connections to one temporary SQLite database race metadata edits from revision 1. Exactly one succeeds, one receives typed conflict/current 2. Reopening confirms revision 1 and persisted Published Version (including frozen model snapshot) are unchanged.
- HTTP router with real handlers/storage/security middleware: server-owned revision rejected, expected revision succeeds, stale writer gets safe 409, GET retains winning value, omitted-field compatibility succeeds, old baseline remains stale.
- Commands from backend worktree: `cargo fmt --all -- --check`; `CARGO_TARGET_DIR="$PWD/target" cargo test -p annotagent-storage -p annotagent-server --offline`; `CARGO_TARGET_DIR="$PWD/target" cargo clippy --workspace --all-targets --offline -- -D warnings`.
- No external provider calls, services, real workspace/database, keys, or frontend changes needed.

UIAPI-009 remains an unimplemented cutover scope; its prior delivery is the concrete gap specification `UIAPI-009_GAP.md`, not runtime acceptance.
