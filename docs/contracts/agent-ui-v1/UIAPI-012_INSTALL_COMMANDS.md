# UIAPI-012 — durable installation command recovery

## POST contract

`POST /api/model-installations` accepts the existing five selection fields and optional UUID `command_id`:

```json
{
  "command_id":"4b97fbd5-594b-4909-8f85-5d664863d720",
  "catalog_id":"catalog-id-from-registry",
  "bundle_id":"bundle-id-from-registry",
  "bundle_version":"1.0.0",
  "plugin_id":"plugin-id-from-registry",
  "plugin_version":"1.0.0"
}
```

Persist the command before sending it. UUIDs should be unique per explicit installation intent, not regenerated after timeout/refresh. Unknown request fields are rejected (422); existing license, trusted plugin, bundle compatibility, session/CSRF and privileged-confirmation checks remain in force. POST replay still needs the normal mutation security headers; read-only command GET is the preferred lost-response recovery.

The exact comparison scope is the five selection strings **plus the resolved installation-root path** within this workspace database. It is returned in `operation.scope`; the command ID is returned as `operation.command_id`. Different version/catalog/plugin/directory with the same command returns HTTP409 `code=model_install_command_conflict`, with no new installer dispatch. Equivalent-but-differently-spelled strings are conservatively treated as changed scope.

First accepted request: HTTP202 and existing operation object plus `command_id` and `scope`. Admission reserves the command and operation in an immediate SQLite transaction **before** spawning the existing installer. Concurrent identical commands have one dispatch winner. Exact replay: HTTP200 with the original operation ID and its latest known receipt, even after terminal failure/success; it never restarts the operation or revalidates a changed catalog as a new installation. Another command targeting the same active Bundle/Plugin tuple returns the existing `model_installation_active` conflict, including when its catalog/root differs.

Omitting command_id remains backward compatible and allocates a server command UUID. Such clients cannot recover a completely lost first response by a client-known command, so the new UI must supply it.

## Read-only recovery

`GET /api/model-installations/commands/{command_id}` returns the operation object directly (200). It does not create, reserve, reconcile, download or run anything. A missing command returns404; **absence is not proof that the original request cannot still arrive** and does not authorize retry with a new command.

Existing GET `/api/model-installations/{operation_id}` also reads durable receipts. Existing list GET returns `{operations:[...]}` with up to32 most recently updated durable receipts; commands and direct operation lookups remain resolvable after they age out of that list or the in-memory cache. No client should infer non-execution from list absence.

Example persisted receipt (other existing progress fields omitted):

```json
{
  "id":"bc6df77c-853e-449d-885b-df003d0b0cca",
  "command_id":"4b97fbd5-594b-4909-8f85-5d664863d720",
  "scope":{
    "catalog_id":"catalog-id-from-registry",
    "bundle_id":"bundle-id-from-registry",
    "bundle_version":"1.0.0",
    "plugin_id":"plugin-id-from-registry",
    "plugin_version":"1.0.0",
    "installation_root":"/explicitly-selected/TEST-model-root"
  },
  "status":"unknown",
  "stage":"downloading_bundle",
  "suggested_action":"inspect_installed_models_do_not_retry"
}
```

`unknown` is additive to running/succeeded/failed. When durable status was running but the current server has no live installer for that operation, GET/replay projects unknown, preserves the last stage/bytes/timestamps and does not change the stored row. It must not imply completed/failed or restartable. A crash between reservation and worker dispatch is also unknown. Terminal receipts survive process restart. Progress and terminal updates are persisted; a persistence failure becomes unknown instead of claiming durable completion.

There is no automatic restart, new-command retry, command deletion, or clearance of uncertain running reservations. Inspection/reconciliation of actual installed artifacts remains necessary; this patch does not add a recovery installer engine. Runtime installation-root changes after dispatch retain the existing settings/installer semantics; this command boundary is not a global settings lock.

## Migration and verification

Additive migration `0060_model_install_commands.sql` adds one table keyed by command UUID and unique operation UUID. It can run without0059, is idempotent, and does not alter bundles, plugin licenses, Published Versions, grants or existing user files. Existing pre-upgrade in-memory operations cannot be reconstructed if their process has already exited; no records are fabricated for them.

Isolated tests use temporary SQLite/application roots and existing failure-before-install fixtures:

- Actual HTTP concurrent identical POSTs: one202, one200, same operation; exact replay after the deliberately disregarded first receipt returns that ID, changed plugin scope returns409, a new server state reads the durable terminal receipt.
- Orphan running reservation: repeated GET and POST replay return unknown with the same ID, never enter catalog validation/installer dispatch; stored row remains byte-equivalent. Unknown command GET leaves tables unchanged.
- Independent SQLite connections race one command: exactly one dispatch winner. Reopen/replay preserves membership; changed scope/directory conflicts and a second active command is rejected.

Commands from delivery-only TEST worktree: `cargo test -p annotagent-server -p annotagent-storage --offline`; `cargo clippy --workspace --all-targets --offline -- -D warnings`; `cargo fmt --all -- --check`. No real workspace, paid Provider, external weights, service restart or Web changes are required.

Verified: Storage/Server246 passed,0failed,2existingignored; workspace all-target Clippy `-D warnings`, fmt and diff checks passed in the isolated delivery-only worktree.
