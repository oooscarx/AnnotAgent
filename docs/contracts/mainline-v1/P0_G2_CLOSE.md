# P0 G2 backend close evidence

This file maps A4, A6 and A7 to executable backend evidence. It does not claim live
commercial Provider validation.

## A4 exact scope

- `processing_operations::tests::delivery_scope_freezes_exact_three_of_ten_and_rejects_limit_or_changed_hash`
  freezes a non-prefix three-image delivery scope out of ten and rejects a changed
  image hash or a caller-supplied limit.
- `tests::confirmed_batch_materializes_exact_non_prefix_three_of_ten_scope` in
  `annotagent-application` proves the formal Batch persists only those three exact
  image IDs.
- `conversation_journey::tests::server_derived_journey_scope_detects_data_recipient_and_schema_changes`
  proves health/progress timestamp changes preserve authorization, while Provider
  destination, enabled readiness, image bytes/hash and Schema revision changes
  invalidate it without reserving a call.
- `conversation_journey::tests::reject_scope_expansion_and_invalid_limits_before_sealing`
  rejects changed model binding digest/recipient, images/hash, Schema identity,
  Sample operation ID and maximum-call budget before sealing.

The authoritative scope fields are `consent.images[{image_id,content_hash}]`,
`allowed_models[{model_id,binding_digest}]`, `schema_id`, `schema_revision`,
`schema_digest`, `maximum_builder_calls`, `maximum_sample_calls`, `expires_at` and
the processing preview's delivery revision/hash, exact images, Draft snapshot,
destinations and budget fingerprint.

## A6 typed outcomes

`GET D/workspace` returns `mainline.result_diagnostics[]`; the exact mapping is in
`HTTP_BINDINGS.md`. Unit regressions are:

- `mainline_task::tests::call_diagnostics_separate_not_sent_unknown_and_invalid_structure`
- `mainline_task::tests::sample_diagnostics_keep_legal_empty_separate_from_projection_failure`
- `mainline_capability::tests::capability_diagnostics_separate_missing_weights_from_missing_capability`
- `p0_diagnostic_fixture::tests::seed_is_idempotent_and_persists_eight_real_read_model_scenes`
- `conversation_human_requests::tests::reference_trigger_requires_quality_evidence_not_normal_empty_or_transport_failure`
- `conversation_journey::tests::unknown_or_invalid_child_result_stops_without_fictional_sample`

The projection copies only typed safe failure fields and stable identifiers. It
does not expose raw Provider bodies, credentials or hidden reasoning. A legal empty
result remains unaccepted; a projection failure does not erase another candidate.

## A7 lifecycle and budget

- `conversation_journey::tests::dispatch_claim_recovery_and_stale_worker_settlement_are_safe`
  and `explicit_execution_queue_is_durable_idempotent_and_single_claimed` cover
  restart recovery, duplicate completion and one durable worker claim.
- `sample_operations::tests::receipts_deduplicate_scope_and_cancel_is_terminal` and
  `restart_preserves_receipts_without_reexecution` cover Sample persistence.
- `persistent_batches::cancellation_prevents_new_image_nodes_from_starting`,
  `failed_image_retry_preserves_usage_and_does_not_repeat_completed_work` and
  `startup_requeues_orphaned_image_and_checkpoint_survives_reopen` cover successor
  stop, retained usage/results and deterministic local restart.
- `conversation_calls::tests::allowance_and_unknown_outcomes_survive_retries_revocation_and_restart`
  and `next_phase_keeps_spend_and_original_receipts_without_scope_or_retry_reset`
  cover expired/revoked/budget-limited calls and unknown outcomes without free or
  automatic retry.
- `agent_ui::tests::stop_http_trace_keeps_unknown_receipt_and_spent_budget_on_retry`
  exercises the production HTTP stop routes and verifies `stopping` to
  `outcome_unknown`, a persisted `in_doubt` call, disabled resume and unchanged
  spent budget on replay.
- `mainline_task::tests::authorization_diagnostics_keep_expiry_and_exhaustion_explicit`
  verifies stable expired/exhausted blocker codes, exact grant scope, read-only
  budget routing, retained results and disabled automatic retry.

The isolated fixture manifest records browser-independent execution and restart:
`p0_autonomy.technical_relay_clicks=0`, one Journey consent, three Sample images,
`controls.interrupted`, `controls.resumable`, `stop.final.normalized_state`,
`restart_verified`, and `seed_snapshot_unchanged`. Fixture startup is explicitly
TEST-only; it uses isolated SQLite/workspace paths and an external-model-only
loopback Provider.

The same manifest now includes eight browser-selectable scenes under
`diagnostic_scenes.scenes.<code>`. Each contains a project-owned `task_url`, the
production `workspace_url`, stable Task ID and the exact diagnostic observed from
`mainline.result_diagnostics[]`. The seed exists only in the explicitly opted-in
ignored test server; production startup and every GET remain side-effect free.
The legal-empty and projection-failed records reference real persisted, project-owned
Drafts and exact current Sample snapshots; the fixture verifies those linked GETs
before exposing the browser URLs.

## Focused commands

```text
cargo test -p annotagent-server delivery_scope_freezes_exact_three_of_ten_and_rejects_limit_or_changed_hash
cargo test -p annotagent-application confirmed_batch_materializes_exact_non_prefix_three_of_ten_scope
cargo test -p annotagent-application server_derived_journey_scope_detects_data_recipient_and_schema_changes
cargo test -p annotagent-storage reject_scope_expansion_and_invalid_limits_before_sealing
cargo test -p annotagent-application mainline_task::tests::
cargo test -p annotagent-server capability_diagnostics_separate_missing_weights_from_missing_capability
cargo test -p annotagent-server p0_diagnostic_fixture::tests::seed_is_idempotent_and_persists_eight_real_read_model_scenes
cargo test -p annotagent-server stop_http_trace_keeps_unknown_receipt_and_spent_budget_on_retry
cargo test -p annotagent-storage --test persistent_batches
```
