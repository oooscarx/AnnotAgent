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

### Typed materialization integration and live baseline

- Backend `25ea0ef` integrated as `1422f24`. Candidate composition now persists and materializes. Full Application integration tests exposed an earlier local Schema-root change's dependent `conversation_future_proposal` use of `oneOf`; updated it to add its required goal to the root typed schema. Application rerun: 157 passed / 1 ignored.
- Existing test Schema edited with CAS to revision 2, ball only; Project labels and annotations unchanged. New Journey `067d16dd-5e69-4936-84cd-900bc82a8c53` produced typed Draft `61204c76-51e4-4ca1-b903-8226de18b53e` but its final planning request timed out at 120 seconds. No sample executed. Remote billing for that failed call remains unknown; it was not retried.
- Created independent explicitly VLM+SAM task `38a9553e-d821-43e9-8325-b5a613e47de5`; manually specified its one-label Schema through the existing human-schema API (`342ec299-39a6-41c7-a797-f838feff6588`). This Schema is human-specified, not an LLM result. The user authorized the vision test; no formal annotations/publication.
- Journey `1e457bbd-a00e-437d-a608-d1872a8b0ac1` / Builder `27ad63e9-9d77-4926-b2ae-c09abecae02c` still salvaged a conservative VLM-only baseline after discovery. UIAPI-011 second-round evidence sent to backend: no SAM despite explicit requirement, baseline candidate only. Await actual refinement-candidate fix; no repeated same planning request.
- Baseline Sample Test `18a6b8d1-62fd-4c9e-be01-fb4ec35f0017` succeeded as an operation: 3 real Qwen requests (773 input tokens each; outputs 11/75/75; durations 1287/1409/1670 ms). `color_2271701.png` had no candidate; `color_1001525.png` and `color_289771.png` each yielded one needs-review bbox. SAM NOT invoked. This is not accuracy validation or completion of the VLM+SAM goal.

### Admitted-scope fix and bounded-planning regression (September 10)

- Backend `f4f4ce7` integrated as `72aa0cd`; salvage retains the admitted Schema, target and Registry intersection. Application tests: 158 passed / 1 ignored, including its preservation regression.
- Independent explicit VLM+SAM task `8d433696-56ef-400f-b37b-79eddd3aee31`, human Schema `b51c0125-eefd-4de0-ac4b-f41d0aef3cd5`, Journey `315aea41-dae0-48a4-bffb-7167806ecec1` did not run samples. Builder `90dc7d7e-b4f5-42e8-ae67-68443bf497a6` spent eight returned calls inspecting/configuring individual nodes; `inspect_models_batch` rejected the Ready Model Instance selection. It ended `budget_exceeded / model_turn_budget_reached`, with no preserved candidate or salvage. No SAM inference yet.
- UIAPI-011 third concrete request sent only to the confirmed backend: fix Ready Model Instance inspection and preserve validated controlled candidates before the budget boundary; do not increase steps, overwrite human plans or claim fake SAM success. Further paid planning paused pending that delivery.
- UI now exposes this terminal budget outcome instead of “waiting for subsequent action”. Real Chromium refresh check on the above task: explicit saved-Draft/sample-not-completed receipt, zero POST requests. Web typecheck, 60 test files (290 passed / 1 todo), and production build passed; existing bundle-size warning remains.
