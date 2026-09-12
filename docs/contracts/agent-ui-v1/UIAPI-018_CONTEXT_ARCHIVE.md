# UIAPI-018 — context archive v1

Implemented contract (2026-09-10). No inference, credential resolution, file reads,
publication, dispatch or live-context mutation in these endpoints.

## URLs

- `GET /api/projects/P/conversations/C/context-archive`: one SQLite read transaction,
  all persisted messages and tasks (not the 100-row history page). Returns archive below.
- `POST /api/projects/P/context-imports/preview`: `{ "archive": <archive> }`.
  Read-only validation. Returns `valid:true`, `preview_hash`, `archive_hash`,
  `target_project_id`, `counts`, `integrity`, `continuation`, `mode:"archive_only"`.
- `POST /api/projects/P/context-imports`: `{ "command_id":"UUID", "preview_hash":"SHA256",
  "archive":<archive>, "confirm_archive_only":true }`. Explicit confirmation, 201.
  Exact command replay returns the SAME receipt/context/ID map; changed project,
  payload or confirmation hash is 409. One atomic transaction, durable across restart.
- `GET /api/projects/P/context-imports/COMMAND`: recovery receipt; absent/wrong owner 404.
- `GET /api/projects/P/archived-contexts/CONTEXT`: isolated loaded archive and receipt.
- `GET /api/projects/P/archived-contexts?after=0&limit=50`: owner-scoped sequence cursor,
  limit 1..100, `{items:[receipt],next_cursor:null|number}`. GET never initializes anything.

POST uses existing session/CSRF protection. Unknown request fields rejected (422).
Invalid archive/version/preview 409 with safe `code`; missing/foreign objects 404.
Oversized requests follow server body limit (413); archive cap is 1 MiB and 10,000
records. Export over cap FAILS, never silently returns a truncated document.

## Archive JSON

```json
{
  "format":"annotagent.context",
  "version":1,
  "payload":{
    "source":{"project_id":"P","project_owner_id":"stable UUID","conversation_id":"UUID"},
    "captured_at":"RFC3339",
    "records":[{"kind":"messages","id":"message UUID","data":{"conversation_id":"UUID","sequence":1,"message_id":"UUID","input":{"id":"UUID","text":"user goal"},"created_at":"RFC3339"}}],
    "resources":[{"kind":"image_id","id":"image identity","embedded":false,"availability":"not_verified"}],
    "integrity":{"snapshot":"sqlite_transaction","all_messages":true,"all_tasks":true,"truncated":false,"redacted_fields":0,"missing_resources":[],"limitations":["raw_model_http_transcripts_not_persisted","hidden_reasoning_not_exported","external_resources_not_embedded","historical_authority_not_transferable"]}
  },
  "archive_hash":"SHA256 of canonical payload JSON"
}
```

Canonical hash: recursively sorted object keys, compact JSON UTF-8, preserve array
order, and normalize finite exactly-integral JSON floats in the JavaScript safe-integer
range to integer form (`-0.0`, `0.0`, `1.0` become `0`, `0`, `1`). This keeps the digest stable when a browser
performs `JSON.parse` followed by `JSON.stringify`; non-integral numbers retain their
value. Server computes it. It is integrity, not provenance/authenticity. Imported
content is **untrusted historical evidence**. Exact legacy v1 documents whose stored
digest used the earlier number-spelling-sensitive canonicalizer remain accepted;
new exports always use the browser-stable digest. Semantic value changes still return
`archive_hash_mismatch`.
`integrity.truncated:false` means no export pagination/truncation; existing bounded
tool results remain as persisted (including their original truncation flags). It
does not assert that the original runtime recorded unlimited model/tool content.
Messages are exported in ascending saved `sequence`.
`records` is a typed-kind envelope over persisted values, with JSON DB columns decoded
and `_json` suffix removed. `messages.data.input.text` is saved user text;
`tasks` contains saved identity plus `state` (`outcome_unknown`, `stopping`, `running`, `waiting_for_human`, `awaiting_approval`, `idle`) and `state_scope:"task_activity_at_capture"`; related receipts preserve execution details.
`agent_sessions.data.session.steps[]` contains actual observable tool `arguments`,
`result`, `success`, `started_at`, `finished_at`. `session.working_memory` preserves stored typed facts (not a hidden reasoning transcript).
`session.model_calls[]` contains
persisted usage/timing/error metadata; `session.builder_proposal` is the actual saved
assistant proposal; `schema_revisions.data.definition` is saved structured Schema.
There is no fabricated assistant final message or delta/hidden reasoning transcript.

The export allowlist includes conversation/task evidence and their related Schema,
Builder sessions/Drafts, Sample reports/feedback/plan revisions and processing records.
Reference resources list images, artifacts, model identities and Runs/versions not
embedded. No image bytes, local paths, weights, secrets, Provider configuration,
keyring values or executable grants are packaged. Sensitive-key fields and
credential-bearing strings are redacted and counted. Redaction intentionally loses
information; arbitrary user prose is not a guaranteed secret detector.
Unavailable related objects are explicit `missing_resources`, never invented.
All listed records are inert evidence, including queue/stop/HumanRequest states.
The snapshot covers SQLite only; project files, remote state and model memory are
not captured, and no archive claims reproducible or resumable execution.

## Import result / continuation

```json
{
 "command_id":"UUID","context_id":"NEW UUID","project_id":"P",
 "archive_hash":"SHA256","preview_hash":"SHA256","sequence":1,
 "mode":"archive_only","created_at":"RFC3339",
 "id_map":[{"kind":"messages","source_id":"OLD","id":"NEW UUID"}],
 "continuation":{"can_resume":false,"available_actions":["view","export"],
 "reasons":["imported_history_is_inert","new_live_task_and_fresh_authorization_required"]}
}
```

Loaded-context GET is `{receipt,archive,objects:[{id,kind,source_id,data}],trust:"untrusted_import"}`.
Each historical record has a new local object ID; its `data` retains original IDs
for faithful evidence. Resolve source references through `receipt.id_map`, never
call live mutation endpoints with imported identities. Source conversation gets its
own map entry. Duplicate natural IDs across kinds remain distinct. No new active
Conversation/Task, grant, HumanRequest/outbox, Draft, Sample, queue or Published rows
are inserted. Existing one-active-Conversation-per-project remains unchanged.

This is complete **loading for inspection**, not execution-state restoration. To
continue work, create a fresh live task via existing Send and explicitly select
current resources/authorize current scope; v1 provides no automatic conversion.
Archived running/pending/unknown states stay historical, cannot be resumed. Export
from the loaded-context GET's `archive` preserves original integrity/hash. Preview
binds target stable project identity, archive hash and archive-only mode. Preview is
stateless: confirmation recomputes validation; preview/GET cannot reserve authority.

## Exact record kinds and UI paths

| Kind | Source / fields to render |
|---|---|
| messages | `data.input.text`, `data.sequence`, `data.created_at`; only saved user messages |
| tasks | `data.id`, `data.source_message_id`, `data.schema_revision`, `data.state` |
| calls | `data.id`, `task_id`, `status`, `created_at` (persisted start), `evidence` |
| call_progress | `data.call_id`, `stage`, `completed_at`, `failure` (nullable legacy fields) |
| builders / agent_sessions | operation status/evidence; `data.session.steps`, `model_calls`, `builder_proposal`, `working_draft`, `plan_candidates`, `planning_events` |
| schema_drafts / schema_revisions | source call/request and base revision; `data.draft_id`, `revision`, `definition` |
| workflow_drafts | `data.draft` (saved full Draft), `revision`, `content_hash`, `status` |
| sample_operations / sample_tests | operation `request/status/error`; report `input`, `model_bindings`, `report`, revisions/hashes/timestamps |
| sample_feedback / plan_revisions | `data.feedback`, sample/image/revision identities |
| human_requests / image_reviews | `data.request` or `record`, `answer`, `status`; historical only |
| queue / queued_planning / queued_copies | sequence/cancellation and stored planning/copy evidence; no authority |
| stops / cancellations | `data.record` or call/requested timestamp; unknown outcomes stay unknown |
| journeys / journey_dispatch / journey_schema | stored scope/input/dispatch/result evidence; never replayed |
| resume_evidence / resume_results / answer_delivery / deferrals | saved checkpoint reference, answer/feedback linkage and previous status/result; no outbox scheduling |
| feedback / feedback_answers / future_schemas / clarifications | saved structured input/result and Schema references |
| processing_operations | `data.request`, `data.state`, persisted timestamps; Run/artifact payloads remain external refs |
| send_receipts / agent_model / exports / export_events | frozen send/model selection and annotation-export delivery evidence |

`resources[].availability:not_verified` is NOT a promise that a local file exists;
`missing_or_not_owned` intentionally does not disclose foreign-project existence.
Only related owned Draft/Sample rows are followed, not all project history. No
all-project Run/Published/annotation dump or local filesystem traversal occurs.
All record IDs are scoped by kind. `id_map` resolves the archived evidence graph;
`objects[].data` remains original evidence and must not be used as a live command.

### Error codes

`archive_unsupported_version`, `archive_hash_mismatch`, `archive_invalid`,
`archive_sensitive_content`, `archive_too_large`, `archive_preview_conflict`,
`archive_command_conflict`, `archive_invalid_page` return409. Invalid JSON/typed
request fields return422 (malformed JSON400). Missing/foreign receipt/context/source
Conversation returns404. Existing session/CSRF middleware supplies401/403/413.
`valid:true` means supported structure/hash/safe inert storage, not verified origin
or a claim that historical external resources can execute. Truncated imports reject.

### Migration and verification

Additive `0064_context_archives.sql` creates one isolated table and owner/sequence
index; startup migration only, no backfill and no active-journal uniqueness change.
No frontend or package changes. Actual TEST response set: `UIAPI-018_EXAMPLES.json`.

```sh
cargo test --offline -p annotagent-storage context_archives --lib
cargo test --offline -p annotagent-server context_archive --lib
cargo test --offline -p annotagent-storage -p annotagent-application -p annotagent-server --lib
cargo clippy --offline -p annotagent-server -p annotagent-storage --all-targets -- -D warnings
cargo fmt --all --check
```

Tests use tempfile SQLite, InMemorySecretStore, and a real loopback HTTP listener on
port0 (OS-assigned, stopped by test). No Provider/Plugin/runtime is called. No new
server/fixture runtime, installation, or real workspace establishment is required.
