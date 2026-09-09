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

### Verified real VLM + SAM execution

- Third-round backend delivery `217fde6` integrated as `b155b6b`: persist scoped Registry candidates before paid discovery, accept admitted native Model Instance inspection, and materialize a separately identified validated alternative at the budget boundary without overwriting the incomplete Draft. No increased model-turn allowance. Local Application regression: 159 passed / 1 ignored; CLI build, fmt and diff checks passed. Strict Application/Core/Storage/Server Clippy had passed before this last delivery; backend reports isolated all-workspace Clippy for the increment, not claimed as a new local all-workspace run.
- Only verified owned 8788 PID 74046 was restarted after all previous calls settled; 8787 untouched. New service serves this recovery worktree's Web build and original absolute workspace. No other branches, Published Versions, original images or history were removed.
- Task `357155d7-3073-4dea-b07b-90adfa10ed4e`; human-specified ball Schema `20fbcab7-3505-414b-98cc-cf37ec1a9e46` revision 1. Default Agent preference restored to GLM by revision check after freezing Qwen for this independent task. No credential values read.
- Journey `86404500-5fc0-4160-9848-f699afaf61bd`, Builder `956b98ea-2adc-44e4-9726-0b49ff0cdab2`: 8 returned planning calls, then a validated Registry alternative Draft `a235c5a0-3d20-4cf4-9021-6897d9117557` revision 2. Exploration stop remains `model_turn_budget_reached`; outcome is genuinely `draft_ready_for_human_review`, not exhausted-without-result.
- Sample `ec3648a4-1c8a-4235-b7d1-e90d823c4f6d` succeeded at 2026-09-09T16:13:58Z. Actual path: Qwen detector → label filter → box prompts → EfficientSAM-Ti ONNX → Mask to bbox → geometry evaluation/decision → human review. Three authorized real B-Human images, no Mock. Each has a nonempty prompt, a Mask artifact and a converted bbox artifact with lineage. Model Instance `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`, `rust_plugin`, CPU, contract `efficientsam-ti-split-onnx-v1`; saved bundle/checkpoint hashes identify the real model.

| Image | Qwen node ms | Qwen tokens in/out | SAM node ms | Result |
| --- | ---: | ---: | ---: | --- |
| color_2271701.png | 1673 | 725 / 75 | 4591 | refinement rejected; review |
| color_1001525.png | 1206 | 725 / 75 | 1739 | refinement rejected; review |
| color_289771.png | 1586 | 725 / 72 | 1680 | refinement rejected; review |

- **Execution is verified; annotation quality is not solved.** All three masks moved/expanded beyond current safety limits (`refiner_drift`), so unsafe refined boxes were not accepted. Coarse localization remains review evidence; the visible second-image coarse box is still on grass. No geometry thresholds were weakened. No formal Commit/Publish/full Dataset Run or human acceptance was performed. The intentional human gate is the endpoint, not an automatic annotation-success claim.
- Cost is unknown, not zero: the saved legacy sample report has a zero estimated-cost field without authoritative pricing; actual token counts above and Provider billing remain the source for cost verification. No billing total is claimed.
- Real HTTP assertions: all three samples nonfailed, exact native SAM identity, Mask + mask-to-bbox artifacts present, zero committed annotations. Browser read/refresh: zero POSTs and saved result retained. Screenshot `../evidence/provider-failure-recovery/vlm-sam-live-result.png`: source `b155b6b`, 1440×900, DPR 1, light, Live. URL: `http://127.0.0.1:8788/projects/robocup-ball/work?task=357155d7-3073-4dea-b07b-90adfa10ed4e&pane=image&image=dbe3d88d-ed0a-55af-b762-b5a1d33d3b35`.
- Remaining: GLM upstream availability is not resolved; this verified chain uses Qwen for planning and vision, local EfficientSAM for segmentation. Improving coarse localization/coverage and reviewing rejected geometry are separate quality work, not concealed by pipeline completion. Changes remain on `codex/provider-failure-recovery`; no push.

## User-authorized Agent repair continuation

- User asked to add and test evidence-driven local re-localization before segmentation. UIAPI-014 sent only to the confirmed backend: use existing typed Crop/Expand/Project/Coverage nodes for Schema-bound repair; preserve original Draft and model scope, no geometry threshold bypass. Backend confirmed the missing typed repair path; pending its independent implementation before more paid testing.
- Added actual UI issue-only feedback for the selected saved sample: wrong target / poor boundary → existing Human Request answer API. It does **not** submit the current bbox as HumanVerified geometry. Applied feedback offers a separate exact Journey repair preview/approval, never an automatic model call. Selection/result revision guards prevent feedback being silently retargeted after refresh.
- Real browser on second image saved `poor_boundary` feedback through request `d0f572be-8268-5023-96d5-62c557466461`; one answer POST, no model POST. Refresh retained applied feedback. Opening repair consent made zero POSTs and displayed exact model/destination/scope/unknown cost; did not approve the displayed GLM request.
- Latest succeeded sample is now selected independently of older pending questions. Comparison reads the original sample through the matching Journey repair request, only for matching image IDs/content hashes; no guessed chronological before/after. Original snapshots remain intact.
- New transport regression verifies issue-only null geometry, correct exact answer route, no model call and stale/foreign selection rejection. Focused HTTP adapter tests: 16 passed; typecheck passed. Full Web before this added regression: 290 passed / 1 todo; production build passed.

### Typed repair integration and admission blocker

- UIAPI-014 `5b58276` integrated as `e758c14` without UIAPI-012/013 or history changes. Local Application/Runtime library tests passed (161 + 33; 1 Application ignored), selected strict Clippy, fmt and CLI build passed. Final Web prior to admission-display regression: 292 passed / 1 todo, typecheck/build passed. Exact-source comparison regression subsequently passed in the focused suite.
- Verified no reserved calls/running samples/running Journey dispatches, then replaced only owned 8788 PID 77289. Started the new recovery binary using the original absolute workspace, preserving 8787 and all samples.
- Explicit new Journey `ef18b12d-023c-4b88-a131-b9212166593a` / Builder `9eab0216-74f0-4c61-9457-8d85d1740b5d`, allowed Qwen + local EfficientSAM, same three images, current cumulative Builder grant and 12 sample calls. Existing Applied feedback R `d0f572be-8268-5023-96d5-62c557466461` bound exact repair copy revision 1. Saved typed revision 2 now has real Expand/Crop/local VLM/Project/Coverage nodes.
- **No new paid request was admitted:** first call was blocked in 0 ms with “Task is waiting for human input; no model call was admitted”, because the other two image requests remain pending. Builder outcome failed; planned sample `33e2de20-4232-4821-b3bd-a36f19890898` was not run. Do not call this a provider outage or a successful repair.
- UIAPI-014 second concrete request asks for exact authorized repair admission without clearing/accepting unrelated pending image requests, plus multi-image Store/HTTP regression and resumability of the already-prepared typed copy. No same-operation retry and no blind paid repeats. UI receipt now exposes the actual Builder admission error instead of the completed transport wrapper.
- Live refresh additionally exposed newest-first Journey history showing the older successful sample as the current status. `845a976` orders Journey receipts by actual dispatch time; regression and Chromium now show the latest failure after refresh, zero POSTs. Web full run before that last regression: 294 passed / 1 todo; latest focused adapter suite: 20 passed; build passed. Browser's first strict locator matched both summary and hidden detail, so it was corrected to the status region and rerun successfully (not an app failure).

### Real bounded repair verified — 2026-09-10

- Integrated only backend `220576f` as `fdb1ea6`. Exact Applied feedback + current authorized Builder/Sample operation can proceed while unrelated images remain pending. Scope, grant, stop, expiry, Schema, editable-copy and lease checks remain. Unrelated publication/history deliveries were not merged. Documentation conflicts retained our history and appended only the repair-admission delivery.
- Initial local Rust test invocation reused a stale Core artifact from the shared build cache (an unrelated `history_scope` field); invalidated the local Core source timestamp and rebuilt without modifying its content. Initial Web typecheck found an incomplete Storage test double; `d6c2e0e` supplies the full interface. Both failures were corrected and rerun, not hidden.
- Local Rust: Application **161 passed / 1 ignored**, Runtime **33 passed**, Storage **166 passed**. Application/Runtime/Storage/Server all-target strict Clippy passed, fmt and CLI build passed. Final Web: typecheck, **299 passed / 1 todo**, production build passed; existing large-chunk warning remains. Full workspace/all-features suite was not rerun this increment.
- Verified zero active calls/samples/Journey dispatches before replacing only owned 8788 PID 89065 with this worktree's built server. 8787 untouched. Web served `/assets/index-Deghn_pA.js`, matching `web/dist/index.html`; screenshot source **70a61236a78e2c5f976706f5444be608c75d1871**. No user credentials read, no original data removed, no push or main merge.
- Fresh explicitly authorized Journey **8c692723-0003-40eb-b35d-c9b9ebf7da1c**, Builder **10d4d881-88b2-471f-92fe-dd5d50f08811**, repaired Draft **1722a8ce-a758-54d8-b51f-a76feae3ee36 revision 3**, Sample **acc182fc-7433-4e78-9ee6-5d829fe6673b**. Preview froze current revision 2/hash, same saved Schema, exactly Qwen profile + Ready EfficientSAM instance, three existing images, existing cumulative Builder allowance 16 and Sample allowance 12. Unknown cost explicitly approved under user's paid-test authorization; no automatic retries or scope reset.
- Builder made **5 actual Qwen planning calls** and returned `draft_ready_for_human_review`, stop `DraftReady`. Sample made **6 actual VLM image calls** (whole-image + local crop per image) and **1 actual local EfficientSAM call**; all call receipts completed. Sample duration 13,173 ms, VLM usage 3,872 input / 383 output tokens. Monetary cost remains **unknown**, not the legacy estimated-cost zero. This is a real-model test, not Fixture output.

| Image | Actual submitted VLM views | Refinement / result |
| --- | --- | --- |
| color_2271701.png | 544×448 full image; 96×96 crop at (124,158) | Local VLM returned no candidate. No SAM Mask; empty terminal projection is **not** proof of no ball. |
| color_1001525.png | 544×448 full image; 96×96 crop at (223,184) | Local box projected to original coordinates; partial coverage, bounded recovery exhausted. No SAM; retain candidate for review. |
| color_289771.png | 544×448 full image; 222×199 crop at (144,165) | Coverage permitted real EfficientSAM-Ti CPU inference (2,205 ms), Mask→bbox and geometry checks; one mandatory review candidate. |

- Changed image digests and `model_input_trace.source_region_pixels/submitted_dimensions` prove local calls received actual crops, not repeated whole-image requests. Segmentation identifies `model-instance:ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`, `rust_plugin`, `efficientsam-ti-split-onnx-v1`, and actual bundle/weight hashes. No geometry threshold was relaxed.
- Visual inspection: third-image box moved from old `[0.395,0.548,0.077,0.092]` to `[0.428309,0.506696,0.099265,0.142857]`, visibly better aligned around the ball, **but still loose above it**. Second image changed to `[0.488059,0.469643,0.023294,0.027857]`; it remains uncertain, not an accuracy claim. No ground-truth IoU/recall measurement or general accuracy percentage is claimed. First image still needs omission checking. Zero committed annotations, no publish/full batch or automatic acceptance.
- Original pending requests `948dcf2f-646f-58b9-8ead-3a57b975a862` and `d28045bb-78f7-56ef-a543-ddb41f7feac0` remain pending on the original sample; selected `d0f572be-8268-5023-96d5-62c557466461` remains Applied. Before/after reads that exact source sample rather than a guessed previous result.
- Added actual route-risk text beside the canvas (`70a6123`): empty local search does not mean no target; exhausted recovery stays uncertain; absence of a Mask cannot be presented as segmentation success. Four regressions cover those distinctions, including mixed routes with a genuine Mask.
- Real Chromium checks: all three image deep links show the new saved sample and “修复前后”; comparison, image changes and refresh issue **zero POSTs**. No model calls from GET/mount. Screenshots below are actual **Live** application, 1440×900, DPR1, light theme, source SHA above:
  - `../evidence/provider-failure-recovery/agent-repair-live-empty.png`
  - `../evidence/provider-failure-recovery/agent-repair-live-comparison.png`
  - `../evidence/provider-failure-recovery/agent-repair-live-sam.png`
  - URL: `http://127.0.0.1:8788/projects/robocup-ball/work?task=357155d7-3073-4dea-b07b-90adfa10ed4e&pane=image&image=48df9bf2-9f58-5908-a639-c853a4c1bab4`
- Remaining limits: one local search is intentionally bounded and may miss the ball or disagree with coarse evidence. Reason-only feedback requires an existing terminal HumanRequest; missing-candidate creation is not newly implemented. Model self-repair is now executable and visibly comparable, **not reliably accurate autonomous annotation**. Current generic server projection descriptors can still call relocalized geometry “Whole-image localization”; UI shows actual coverage risk, but those descriptive fields are not asserted as corrected here. No endless additional paid attempts were launched.
