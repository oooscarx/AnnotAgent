# UIAPI-011 round 3: manual Draft budget fallback and ModelInstance inspection

## Implemented scope

The earlier fix repaired discovery salvage input. The separate manual path set `current` after `create_pipeline_draft`, preventing the discovery deadline from triggering. Model-turn exhaustion then completed with BudgetExceeded without synthesizing candidates.

For Schema-bound FromScratch/ImproveExisting calls, the Runtime now derives registered conversion fragments and saves bounded Registry candidates before the first Provider turn. Preseeded candidates do not immediately interrupt manual drafting. On model-turn/step budget exhaustion, it refreshes availability within the admitted Schema/model scope, revalidates/ranks candidates, materializes a deterministic alternative and performs the existing static validation. No extra Provider turn, paid retry, sample, publication or annotation acceptance occurs.

If an in-progress Draft exists, fallback creates a **new Draft ID**. The old nodes, bindings, policies and references remain stored without modification. A `working_draft_created` planning event names the preserved ID and the new alternative ID. The session working Draft/proposal points to the alternative. Existing RepairDraft/ResolveBindings flows are excluded; this repair does not replace a human-authored repair plan.

## Existing receipt fields

No new HTTP endpoint or migration. Successful deterministic materialization returns the existing Builder result:

- `outcome: draft_ready_for_human_review` when the candidate is runnable and static validation passes; otherwise the existing blocked-Draft outcome.
- `builder_stop_reason: model_turn_budget_reached` (or `total_tool_budget_reached`) records the reason paid/model exploration ended, **not** a claim of execution failure or model retry.
- `salvage_outcome: runnable_draft_materialized` or `blocked_draft_materialized`.
- `plan_candidates`, `selected_candidate_id`, `working_draft`, `builder_proposal`, `planning_events` and validation retain their current formats.

A segmentation node/LabelPipeline is planning evidence only. VLM + prompted segmentation actual artifacts still require the integration owner's separately approved execution. An unavailable or invalid refinement cannot be called executed or successful. No label/model-brand hardcoding or larger steps limit was added.

## `inspect_models_batch` Tool contract

`ids` remains an array of 1–8 Registry IDs. Its declared enum now includes the admitted Model Profiles **and** admitted expert/model-instance IDs. Exact IDs and existing unambiguous local-instance aliases use the same resolver as other Builder model tools. Unknown/unadmitted IDs fail; this tool never falls back to the global Registry.

The result remains `{context_revision, models:[...]}`. Model Profile elements retain their prior serialized shape. Expert elements are additive tagged objects:

```json
{
  "kind": "expert_model",
  "model_id": "model-instance:<registered UUID>",
  "manifest": {"model_id": "model-instance:<registered UUID>"},
  "binding_kind": "model_binding",
  "next_inspection_tool": "inspect_model_contracts",
  "model_profile_binding_supported": false
}
```

`manifest` above is abbreviated: actual response is the existing credential-free ExpertModelManifest with capabilities, contracts, checkpoint and truthful availability evidence. ModelInstance IDs must not be passed to `bind_model_profile`.

## Isolated tests

- Existing four-turn no-discovery Conversation Schema regression remains passing.
- New eight-turn script: context → batch model inspection → feasibility → create Draft → image node → VLM node → profile binding → runtime policy. At budget termination, the original partial graph remains under its original ID and a separate statically valid projected segmentation Draft is returned. No inference Provider is called.
- Existing native ModelInstance materialization test now asserts batch inspection returns the exact native ID, declared enum inclusion, correct next-tool/binding contract and rejection of unknown instances.
- Existing unavailable-segmenter and manual/repair tests remain authoritative.

Delivery is verified in an isolated worktree containing only this increment over f4f4ce7. Commands: `cargo test -p annotagent-application --offline`; `cargo clippy --workspace --all-targets --offline -- -D warnings`; `cargo fmt --all -- --check`.

Paused history scope/management/0059 work and the unrelated Schema root-object/future-proposal changes are excluded from this delivery. No real workspace establishment, service restart, paid call or push is performed.

Verified totals: Application 159 passed / 0 failed / 1 existing ignored; workspace all-target Clippy (`-D warnings`), fmt and diff checks passed. Disabling only budget fallback reproduces the eight-turn regression failure: no separate materialized alternative exists.
