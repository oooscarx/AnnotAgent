# UIAPI-014 — bounded typed localization repair

## Delivered behavior and limits

The existing authorized `RepairDraft` Builder consumes the immutable sample-plan feedback and terminal-only observations already loaded by `sample_repair_evidence`. For the latest selected subject feedback, `poor_boundary` or `wrong_target` with an actual terminal bounding box can prepare one controlled local-search revision. No corrected box is required: `corrected_value:null` is supported. Notes are not parsed for permissions, labels, model identities, or executable instructions. Missing candidates, additions, classification values, unmatched/intermediate subjects, `correct` and `cannot_judge` do not seed a box search. Historical feedback superseded by a later selected correction is ignored.

The source must already have a Schema-bound typed VLM detection/filter → prompted segmentation → mask conversion → geometry evaluation/decision → Human Review plan. The revision keeps its Schema, target labels, existing model profiles/native selection IDs, runtime policies, and source sample linkage. It refuses an already-expanded route or a graph whose flat topology differs from its authored projection. Other labels remain in place. No new model is selected or installed.

The resulting typed route is:

`original image + coarse filtered box → core.expand_region → core.crop → existing VLM on crop → core.project_coordinates → existing label filter → box prompts → core.prompt_coverage_gate → existing prompted segmenter → mask_to_bbox → existing geometry evaluation/decision → mandatory Review → existing terminal`

Core's existing relative expansion defaults remain 4× width/height, minimum96px, maximum0.5 image fraction. The crop is from the original image. The local VLM keeps the existing prompt/resources/model binding and uses local-crop coordinates; its local node permits at most1 model call and no added retry/fallback. The source coarse inference remains in the plan; it is not replaced by a human box.

Coverage uses the projected local candidate and the original-view detection evidence. Only `refine` routes prompts/coverage to the segmenter and mask converter. This bounded repair has one recovery attempt; uncertain refinement is disabled. Coverage disagreement, missing evidence, mixed routes or exhausted recovery goes to Review. Existing overlap/geometry thresholds are unchanged. A target may remain outside the expanded crop, or a good local candidate may disagree with a bad coarse box: this does not justify bypassing coverage. SAM/native segmentation is therefore **not guaranteed to execute**, and successful planning is not improved annotation quality.

Preparation occurs on the existing authorized repair copy, after Core/grammar/geometry validation and admitted Registry candidate checks. Runtime compatibility IDs use the existing profile normalization helper; profile identity and native model selection remain authoritative. The candidate persists in `session.plan_candidates` with source `registry_synthesis`, ID `sample-localization-repair-{session_id}`, and real selected feedback revision IDs in `evidence` (`tool_name:"saved_sample_feedback"`). Full source linkage remains in the existing sample-plan evidence API.

A prepared repair cannot be replaced by the old create/empty/setup template tools in that operation. Other edits are still subject to existing validation. On model/tool budget termination, an unchanged prepared content hash that revalidates can return `outcome:"draft_ready_for_human_review"`, Draft `ready_for_human_review`, with the actual budget stop reason retained. Changed/invalid drafts do not use this finalization; Provider failures/cancellation are not converted to success. No additional budget, inference retry, grant, acceptance or publication is created. A separately authorized sample/Journey remains necessary.

## Existing HTTP chain — no new endpoint or request field

Let `T=/api/projects/{project}/conversations/{conversation}/tasks/{task}`.

1. Read the actual sample terminal candidates and image identity/hash. Create `POST T/human-requests` with `{id,conversation_id,task_id,sample_test_id,image_id,content_hash,outcome_id,expected_feedback_sequence,reason_code,question,resume_checkpoint_ref}`. IDs are stable UUIDs where required; `resume_checkpoint_ref` is the new repair-copy identity. Existing ownership/current-sequence checks apply.
2. `POST T/human-requests/{id}/answer` with `{answer:{revision_id,sample_test_id,image_id,sequence,reason:"poor_boundary",outcome_id,corrected_value:null,note,created_at}}`. Preserve exact sample/subject and monotonically checked sequence. The server persists the correction and prepares the original-preserving copy; use the response's `status:"applied"` and `resume_draft_id`. Do not fabricate a corrected bbox or completion. If local continuation failed after saving, use the existing request resume operation; do not rewrite the answer. Without `journey_consent_id`, this answer does not launch a Journey.
3. For separate planning, GET `T/builder-preview?operation_id={newUUID}&schema_id={savedUUID}&schema_revision={N}&repair_request_id={appliedRequestID}` (optional actual planner `model_id`). POST `T/builder-operations` with the returned `{selection,repair,previous_grant_id,scope_hash,expires_at}` plus `allow_unknown_cost:true` only after the existing explicit authorization. Reuse the operation UUID for recovery; exact existing repeats return their receipt, not another dispatch. Stale source revision/hash, wrong owner, changed Schema or insufficient allowance is rejected under the existing contract. Poll `T/builder-operations?operation_id={id}` and inspect its evidence/session proposal.
4. Existing Journey alternative: GET `T/journey-preview` with `consent_id`, `builder_operation_id`, `sample_operation_id`, saved `schema_id/schema_revision`, `repair_request_id`, optional `planner_model_id`, and `allowed_models` as the JSON array of exact approved model selection IDs. Save the exact returned consent via `POST T/journey-consents`, then `POST T/journey-consents/{id}/execution`. GET execution is read-only. This uses the same repaired Builder path and existing separately frozen sample scope; do not launch both separate Builder and Journey for the same intended repair.
5. For separate sampling, GET `T/sample-preview?draft_id={repairID}&request_id={newUUID}`; use its real image indices, revision, `authorization_fingerprint`, and `conversation_budget`. POST `/api/projects/{project}/sample-operations` with `{request_id,draft_id,image_indices,expected_revision,authorization_fingerprint,conversation:{conversation_id,task_id,previous_grant_id,scope_hash,expires_at,allow_unknown_cost:true,human_review:true}}`. Poll `/api/projects/{project}/sample-operations/{request_id}`. Existing guided scope selects the explicit first1–3 project images; it does not authorize arbitrary image changes. No scope/grant is enlarged by this repair.

## Before/after mapping

GET `/api/projects/{project}/sample-plan-copies/{repairID}` returns `sample_test_id`, `baseline_draft_id`, `project_id`, saved `feedback` and lineage. GET `/api/workflow-drafts/{baselineDraft}/sample-test?test_id={sourceSample}` retrieves the immutable baseline; GET the repaired Draft sample-test with the exact resulting test ID for after. Both use the existing sample wrapper. Compare matching saved `inputs[].image_id/content_hash` and terminal `report.samples[].projection`; masks/intermediate stages remain trace evidence, not independent accepted results. Differences in image/model scope must be visible. No new aggregate quality score or sample-comparison engine was added; Workflow Published Version comparison is not a sample comparison.

Frontend-reported live case: source sample `ec3648a4-1c8a-4235-b7d1-e90d823c4f6d`, Applied request `d0f572be-8268-5023-96d5-62c557466461`, copy `1722a8ce-a758-54d8-b51f-a76feae3ee36`, `poor_boundary`, null corrected geometry. This backend delivery did not inspect/mutate that real workspace or run its paid inference. The integration owner performs actual post-merge acceptance.

## Isolated acceptance and files

- `crates/annotagent-application/src/localization_repair.rs`: pure typed candidate composition and subject-evidence filtering.
- `crates/annotagent-application/src/lib.rs`: existing RepairDraft integration, candidate persistence, guarded budget finalization and isolated regression.
- `saved_feedback_builds_typed_localization_repair_without_changing_source_or_models`: real temporary SQLite sample+feedback+copy, scripted existing Builder, Core/grammar checks, exact Schema/models/source preservation, unauthorized model rejection, repeated repair refusal, legacy-template overwrite rejection, bounded budget ready outcome, no dry-run/Run/publication and unchanged original sample/feedback.
- `feedback_subjects_require_saved_terminal_boxes_and_never_parse_notes`: unrelated image, intermediate/classification/no subject, superseded feedback and non-repair reasons excluded.

```sh
cargo test --offline -p annotagent-application -p annotagent-runtime
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Delivery-only isolated results: 210 passed,1 existing ignored; full workspace all-target Clippy `-D warnings`, fmt and diff checks passed. Existing runtime coverage rejection/unknown/independent-overlap, coordinate projection and refiner drift lineage tests are included. No network model, weight installation, real user data/service update, Web edit, migration, push or merge. UIAPI-009/012/013 changes are excluded from this increment.
