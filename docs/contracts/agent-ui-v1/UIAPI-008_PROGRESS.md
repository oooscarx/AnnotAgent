# UIAPI-008 — Schema call progress and safe failures

Backend base: 334f671; checked local main 83d2adb: Rust and migrations are identical to backend base. This is an additive backend commit; no frontend/main merge, real workspace inspection, service restart, paid call or push.

## Adapter fields (existing endpoints)

Existing `POST T/schema-proposals`, queued `POST T/message-queue/{id}/schema-proposals`, `GET T/calls`, and `GET T/workspace` (`calls[]`) return the same `ConversationCallReceipt` with these additive nullable fields. No new progress endpoint or SSE stream is introduced.

| Field | Type | Meaning |
|---|---|---|
| `id`, `task_id`, `request_hash`, `status`, `evidence` | unchanged | Preserve exact call identity and existing completion/unknown semantics |
| `started_at` | RFC3339 string or null | Durable reservation time (`conversation_model_calls.created_at`), not proof of remote receipt |
| `completed_at` | RFC3339 string or null | Local settlement time, including local unknown-outcome settlement; never remote completion time |
| `duration_ms` | nonnegative integer or null | Persisted end minus persisted start in milliseconds, clamped at zero for clock rollback; null while active/legacy end unknown. Recovery duration includes downtime |
| `stage` | string or null | `reserved`, `provider_request`, `response_received`, `settled`; last persisted local boundary. Null for historical records without metadata |
| `failure` | object or null | `{stage,category,http_status}`; closed safe enums, no free text or remote payload |

`provider_request` means invocation of the Provider completion method, including its preparation/network wait. It does not prove bytes reached the remote model. `response_received` means a complete `ModelResponse` returned and Schema interpretation is about to run. `settled` means receipt saved locally, including `in_doubt`. Short stages may finish between polls. Other users of the shared call ledger get reservation/settlement timing but no invented Provider boundaries. No percent complete, token count, first-token timestamp or remote stage is inferred.

Failure `stage`: `prepare_request`, `provider_request`, `response_body`, `response_decode`, `structured_output`, `handler`, `recovery`.

Failure `category`: `configuration`, `cancelled`, `timeout`, `connection`, `transport`, `http_status`, `invalid_response`, `invalid_structured_output`, `provider_error`, `local_error`, `interrupted`. `http_status` is a number only when a non-success response status was actually received; otherwise null. UI may map 401/403, 429, 5xx to explanatory labels, preserving the actual code. Generic legacy/custom Provider errors have no recoverable typed cause: category `provider_error`, never their arbitrary strings.

The safe failure is also saved at `evidence.failure` for new Schema failures/recovery. `evidence.error` remains the generic outcome explanation. Complete Provider output with invalid Schema semantics remains `status=completed` plus `failure.category=invalid_structured_output` and the existing decision error: completed transport is not an accepted Plan/Schema/annotation.

All existing admission, ownership, authorization, budget and exact-ID rules remain. An error after reservation conservatively preserves `in_doubt`; no automatic retry/refund/resume is added. Same-ID retry returns the stored receipt without a new request. Do not use `completed_at` or `stage=settled` to turn an unknown outcome into success. UI can poll the existing calls/workspace while the original POST remains pending, with one outstanding poll and task/call identity checks.

## Storage / migration

0058 creates `conversation_call_progress` with `CREATE TABLE IF NOT EXISTS` in the existing migration transaction. It adds no credentials or remote text. Reservation creates stage metadata; settlement atomically saves end/failure alongside original evidence. Duplicate settlement does not update time. Handler drop and startup recovery settle remaining reserved calls to in_doubt with typed interruption reason; repeated recovery leaves terminal metadata unchanged. Metadata cannot move a settled call back to an active stage.

Historical rows retain their existing created_at as started_at; end/duration/stage/failure remain null if not previously observed. Old discarded causes cannot be reconstructed. In particular the reported call `65f477f1-6ce2-4006-a715-cb0560407108` was not inspected or modified, and this repair cannot recover its lost error. Existing JSON receipts deserialize with missing new fields. The additive table can be ignored by an old binary, but old binaries will not record progress.

## Streaming investigation and bounded follow-up

`VisionModelProvider` has only `complete(ModelRequest, CancellationToken) -> ModelResponse`. OpenAI-compatible implementation sends Chat Completions, reads the complete HTTP body, parses JSON, then promotes/validates structured tool output. It has no SSE event parser, tool-argument delta assembler, content delta callback, or durable delta replay. Registry client's `bytes_stream()` only bounds an ordinary registry response; it is not model token streaming. Existing Run SSE is execution history, not Provider text deltas.

This delivery exposes **no text or structured-output deltas**. `stream:true` in extra request fields is rejected as typed configuration failure before any network request; otherwise a streaming response would be misparsed as whole JSON. Reasoning/chain-of-thought is not added to receipts. The provider now observes cancellation while reading the response body as well as while awaiting response headers.

A bounded follow-up would require an explicit opt-in streaming completion contract, Chat Completions SSE framing and safe `content`/tool-argument assembly, cancel/truncation handling, and tests proving only final complete structured output can be validated/materialized. Replay/backpressure would require an agreed delivery contract. This work is not implemented or claimed here; current whole-response results remain available through the existing receipt/evidence.

## Validation

Deterministic loopback Provider tests verify actual 401/502 status, invalid body/JSON, response-body timeout and cancellation, no raw secret/body in diagnostic, and stream=true refusal before network. Application tests check live provider_request, completed timing, cancelled unknown receipt and same-ID single-call replay. SQLite tests cover stage ownership, recovery/no replay, terminal immutability, and additive upgrade with legacy null end times. All use temporary databases and test providers; no 8787/8788 access.

Commands: `CARGO_TARGET_DIR="$PWD/target" cargo test -p annotagent-core -p annotagent-provider -p annotagent-storage -p annotagent-application -p annotagent-server --offline`; `CARGO_TARGET_DIR="$PWD/target" cargo clippy --workspace --all-targets --offline -- -D warnings`; `cargo fmt --all --check`. Final results and SHA are recorded in the delivery reply.

Real HTTP verification: fresh isolated seed completed 280 requests. `http_call_progress_check.py --manifest <TEST workspace>/manifest.json` verified 42 actual call observations across 23 identities, including provider_request and settled/in_doubt with typed cancellation and persisted times. See UIAPI-008_TRACE.json. Run the existing `http_fixture.py --enable-fixture --smoke` first; the checker reads only that marked fixture trace and does not call a model. Builder-operation receipts are a distinct DTO and are not claimed to have these new fields.
