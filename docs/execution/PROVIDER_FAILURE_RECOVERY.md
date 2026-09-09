# Provider failure recovery — 2026-09-09

Scope: fix and paid-test new conversations reporting unknown remote results. No automatic retry, grant reset, publication, image inference or dataset run. Existing 8787 service and unrelated main-checkout changes are protected. Implementation isolated in `codex/provider-failure-recovery` from `60b41e0`.

## Evidence and cause

- A newly created user task really received HTTP 503 in 57 ms; this was not another task's stale UI state.
- GLM active probe succeeded (17 tokens), but full Schema call `ca83b6e1-2358-4d71-910a-ed5abf056e17` returned HTTP 429 after 30.072 s. Probe success is not full-workflow availability.
- Qwen full call `8c7ef4f4-e1e9-4c3b-b625-e0499f104fe8` returned JSON-encoded strings for array/object/boolean tool arguments and was correctly rejected (887 input / 2401 output tokens).
- Expose an object-root tool schema with explicit property types instead of root `oneOf`; retain strict response validation without coercion or additional model calls.
- Patched Qwen call `c020bbea-345e-47eb-8de9-d3b8af30f004` succeeded in 65.601 s, 897 input / 4932 output tokens. Its native typed `decision.Ok.draft` was persisted as Schema Draft `04dc294c-cd50-4e74-80d6-fb0c402c3768`, task `fa97bd7b-523d-4733-84b2-b6f343e7ae7c`. This is a target Schema, NOT an executed vision pipeline or proof of bbox quality.
- Default conversation model preference was restored with revision checks after each Qwen test. No credentials were read or printed. Cost is unknown, not free; consult provider billing. GLM's upstream 429 is unresolved.

## UI behavior

Show actual HTTP status and actionable 429/503 descriptions while preserving unknown billing status. Block fresh initial authorization on unknown tasks. Offer explicit retained-text independent request, without sending or approving automatically. After a valid Schema decision, restore the state on refresh and offer sample construction rather than another initial Schema grant. Do not clear old receipts or bypass authorization.

## Verification

- Rust: `cargo test -p annotagent-application conversation_schema --lib --offline`: 7 passed; `cargo build -p annotagent --offline`: passed. Shared backend target directory used for compilation only. Strict rejection regression included. Full workspace/clippy not executed for this focused change.
- Web: typecheck passed; unit 285 passed, 1 existing todo; production build passed (existing bundle-size warning).
- Chromium real HTTP against patched service 8796: successful result survives refresh; no duplicate planning button; HTTP 429 visible; retained-text independent request has clean task URL and preserved text. Browser recorded **zero POSTs** for reads, refresh and independent local draft creation.
- Actual app screenshots: `../evidence/provider-failure-recovery/success.png`, `../evidence/provider-failure-recovery/http-429.png`; 1440×900, DPR 1, light, Live workspace. Source is the implementation commit containing this document, based on `60b41e0`. URLs use `/projects/robocup-ball/work?task=...` on isolated verification port 8796. Screenshots are actual HTTP UI, not Fixture.
- Backup of real history database created with SQLite backup before patched-service startup; no history removed.

## Coordination

UIAPI-010 sent only to confirmed backend thread `01a0855e-9c39-7c33-9f18-93e084d14816`, describing upstream 503/429 and schema types, then providing successful paid evidence. No parallel edits requested for `output_tool`. No push.

## Deployment verification

Implementation commit: `6e8863e`. Merged the concurrently completed main commit `8b19acc` into this isolated recovery branch without conflicts (`7cbdee2`); did not change main or its uncommitted work. Post-integration Web checks: 286 passed / 1 existing todo, typecheck and production build passed. Only verified PID 49202 (8788) was stopped, with zero reserved model calls; 8787 remained untouched. Patched 8788 now serves this worktree's build and the original absolute workspace path. Chromium verified byte-for-byte served index identity, successful task refresh and real HTTP 429 display on 8788. Verification 8796 service was stopped afterward. No further paid calls occurred on refresh or deployment.

## VLM + SAM continuation (explicit user authorization)

- Sample authorization now filters project-bound remote models by enabled/available/image modality, and explicitly lists ready/selectable prompted-segmentation Model Instances. No text-only GLM in image scope; no unavailable legacy Plugin alias. Approval still displays all destinations and requires consent.
- Existing valid Schema is resolved through the read-only call/schema-draft endpoint and its exact revision; no repeat schema_call_id or reset of prior grant. Builder model selection is explicit for this new request.
- Real Journey `13bebeaf-31e9-47c9-9b43-2e732bc07590` (Qwen planner, Qwen vision + EfficientSAM allowed, server-selected first three images, 8 Builder / 12 sample call ceiling) was approved once. Builder `c13a818e-da33-4c8d-9d6f-8354b68aae43` completed after six paid planning calls, but emitted a legacy DAG with `label_pipeline=null` and no SAM. Dispatch correctly refused sampling: “This plan contains bindings not yet supported by bounded guided sampling. No inference was started.” NOT a successful vision test.
- UI now consumes `workspace.journey_consents` dispatch status/errors. Builder completion cannot hide failure or ongoing work in the overall journey. Regression covers running and failure; browser checked real authorization includes Qwen + local EfficientSAM.
- UIAPI-011 sent to the confirmed backend thread with this exact reproduction; requested executable materialization fix, no guard bypass and no independent paid calls. A direct thread message was needed because queued messages had not started a turn in the unloaded backend thread. Work continues; no success claim yet.
