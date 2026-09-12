# P0 autonomy UX status

## Baseline

- Integration branch: `codex/p0-autonomy-integration`
- Starting commit: `fbbda66f488dae19063e95f2a8b3061862e06e57`
- User service `127.0.0.1:8788` and its real workspace are read-only evidence sources for this task. They are not restarted or mutated.
- Demo entry and product packaging work are paused behind this P0.

## G0 — reproduced break

The production Thread currently mounts independent projections for the operation label, all execution receipts, usage, model setup, the full Delivery Intake, Builder/Sample/processing buttons, exports, Schema history, feedback history, an approval card, the Plan and the human request. Each block independently decides whether it is current. This allows the already-approved Builder → Sample boundary to appear as a second `test_pipeline_samples` confirmation.

Read-only evidence from `127.0.0.1:8788` also confirms that the existing task read model already exposes `intake`, `steps`, `available_actions`, `review_summary`, `completion` and blockers. The UI therefore does not need a second task state machine; it needs one tested selector over the server projection.

| Edge | Trigger | Authorization source | Expected wake-up | Human decision? |
|---|---|---|---|---|
| upload/send → delivery semantics | user message plus uploaded image snapshot | no paid execution implied | persisted task/Journey | only if a required semantic is missing or contradictory |
| delivery semantics → Schema | server Journey | matching draft preparation scope | same Journey after commit | no |
| Schema → Builder | server Journey | JourneyConsent Builder scope | worker/operation completion | no |
| Builder → Sample | server Journey | same JourneyConsent Sample scope | persisted Builder completion wake-up | no second confirmation |
| Sample → review | server Journey | same frozen sample scope | sample terminal projection | yes, inspect actual image and annotation |
| review → formal processing | explicit user approval | frozen formal scope | processing worker | yes, approve remaining scope |
| complete review → package | explicit package consent | frozen reviewed snapshot | package worker | no additional technical relay |

## Work in progress

- Added a failing-first selector contract for missing information, one bounded approval, real execution, review and Ready package states. The duplicate same-scope `test_pipeline_samples` confirmation is projected as a server continuation defect, never as another user action.
- Replaced the independently mounted production-flow blocks with one `CurrentTaskStatus`. Full task editing is now opened explicitly; it is no longer mounted after every Thread.
- A review work item opens the existing right image pane once per server work item/result revision. This is view-only URL restoration and does not issue an execution request.
- The Backend `inspect_automatic_sample_progress` projection now keeps the HttpAdapter task in `running`, so the existing GET-only refresh observes Builder → Sample continuation instead of showing an idle task. It never renders a second Sample approval.
- Execution receipts, model usage, plan lineage and technical links remain available in one lazy detail disclosure.
- Keep task editing, trace, schema, feedback and historical records reachable on demand.
- Integrated the first durable Backend continuation increment: queued/running Journey intents remain active across browser closure and Builder completion starts the original frozen Sample operation without a second confirmation.
- Integrated Frontend 2's terminal-only Sample result surface and Frontend 3's task-scoped model preparation. A real review state opens the right result surface; model setup first appears as one compact blocker and returns to the same task after a passive recheck.
- A server-issued combined Builder + Sample action may resolve the still-unconfirmed label and output target in its visible proposal when the image snapshot is already frozen. Missing images still block approval, so the UI never grants an empty or inferred dataset scope.
- The Http Adapter now consumes that action's exact task-owned `journey-preview` URL and server-frozen image/model/call scope. One explicit POST saves the approved Journey and lets the durable server worker continue; the browser no longer sends a second `/execution` relay for this path.
- New-task uploads now retain only the stable image ID + content hash returned for that upload request and include that exact list in the persisted Send command. They never infer “latest images” from Project ordering; unknown Send retries keep the original list and reject scope changes.
- Existing described Tasks now use the same exact upload identities through the delivery-intent CAS. The adapter preserves an existing saved label/target/split, persists the command before sending, replays the identical command only after another explicit upload action when the response was lost, and rebuilds only after the server returns `delivery_revision_conflict`.
- Frontend 2's result-fit correction is integrated. The P0 result surface imports the domain canvas CSS, hides its dimension probe from layout and resets Fit only when the image identity changes; polling the same image keeps the user's zoom/pan.

### G0 verification

- `npm test -- --run src/agent-ui/currentTaskPresentation.test.ts src/agent-ui/App.modelSetup.test.ts src/agent-ui/mainline.test.ts` — 14 passed.
- `npm run typecheck` — passed.
- Focused P0 UI unit checks after review-focus hardening — 16 passed.
- Current integrated Web regression at `4e5848d`: 119 test files / 426 tests passed; production build passed.
- Task usage now defaults to a compact per-task summary, deduplicates physical attempts across owned pages, prevents late pages crossing Task ownership, and reports read failures as unknown rather than zero or raw transport text.
- Frontend 2's isolated component harness is 5/5 under the Vite UI Preview runner only. It is not production-page evidence and is excluded from P0 acceptance; the same `/@fs` harness cannot run under `annotagent serve` (5/5 fail by construction), so a real packaged-route HTTP E2E is still required.
- Review focus is now keyed to the stable server work item, human request or Sample instead of the changing read-model revision. Polling no longer repeatedly steals focus, and the visible review action is only a secondary recovery control because the result opens automatically.
- The earlier `P0-F1-HTTP-001` baseline at `ee7a6cd` remains recorded as a real pre-fix failure: its TEST provider Schema request settled `in_doubt`; it is not counted as passing evidence.
- A second clean run exposed `P0-F1-HTTP-002`: the scripted Schema response omitted required delivery semantics. Backend fixture commits `046ecd8` and `dbdd753` made the TEST response contract-complete without adding a production fallback or retrying a saved model call.

### G1 real packaged HTTP evidence

- Current application source SHA: `4e5848d`; isolated service backend SHA: `4f8312ad4c0d2dfc335d16dfda77ab3a55009fd2` (includes Backend exact later-upload SHA `69d8ef1`, clarified-Journey SHA `926d9bd`, A2 completion-race SHA `1ab6834`, bounded clarification SHA `acde6d3`, and Frontend 2 Fit SHA `5dae4db`).
- Current isolated external-model-only manifest: `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-nmf2_dw_/manifest.json`; service `127.0.0.1:8892`, scripted provider `127.0.0.1:8893`. The user service on `8788` was not restarted or mutated.
- Permanent production-route test: `web/e2e/p0-autonomous-sample.spec.ts` — all three tests passed (8.4s + 1.0s + 6.7s) against the built React application and real Rust HTTP/SQLite service. The first route gives a complete cup + bottle bounding-box requirement; the third deliberately leaves `YOLO` output semantics ambiguous.
- The browser uploaded 6 real demo-pack images. The exact 6 `{image_id, sha256}` receipts were frozen into Send; the server selected exactly 3 Sample inputs.
- The browser made one `POST .../journey-consents` and zero `POST .../execution` calls. After the page closed, the durable worker completed Schema → Builder → Sample and created exactly 3 pending HumanRequests bound to the same task and Sample operation.
- Reopening the exact task showed `检查样例结果 · 3 张` and `3 个结果需要人工判断`; reloading retained the same task/result without a mutation.
- The first Builder transport deliberately waited four seconds. Current seed consent admission returned in 13ms; the first passive status read was `dispatch=running` with `sample=null`; the original authorized Sample reached review after 4.73s. No second POST or replacement Sample was created.
- After Sample completion, the sole server action is the read-only `review_sample_results` action with the exact Sample ID and three pending HumanRequest IDs. Formal processing is not offered before those decisions.
- A second production route sends the complete requirement before uploading anything, records the missing-image state, uploads six files into that same Task through one exact delivery-intent CAS, and reaches the same single combined action without Sample execution. It issues no Journey consent or `/execution` write before approval. The adapter unit regression also proves exact replay after a lost delivery response and reload.
- A third production route receives one server-owned output question for ambiguous `YOLO`. The Thread shows one enabled `框住目标` choice and explicitly disabled segmentation/classification choices with their actual capability reasons. Selecting bounding boxes posts one versioned clarification answer, reuses the original Journey consent and Sample ID, makes no second Schema model call and reaches the same three-image review surface. The client does not parse the question, reconstruct the private Schema, or call `/human-schema-drafts`/`/execution`.
- The continuous TEST browser recording and six current-application screenshots are under `web/test-results/p0-autonomous-sample-one-b-38fa8-o-three-real-Sample-reviews/` (`01-ready-to-start` through `06-sample-review-mobile`, plus `video.webm`). Missing-information and recovered ready-state evidence plus a second recording are under `web/test-results/p0-autonomous-sample-descr-9cb76-k-and-exact-six-image-scope/`.
- The bounded clarification question, same-Journey review result and continuous recording are under `web/test-results/p0-autonomous-sample-ambig-02b5e-nd-resumes-the-same-Journey/`.
- Visual inspection confirms the whole source image and all three terminal boxes are visible on initial desktop review after the Fit correction. The same real result is readable in system dark theme and at a 390×844 viewport. The 720-CSS-pixel layout-pressure check from Frontend 2 is not relabeled as native browser 200% zoom.
- The TEST provider proves orchestration and recovery, not commercial-model accuracy. Formal processing, dataset review and package export remain separately authorized scopes.
- Final Rust regression at `4e5848d`: `cargo fmt --all --check` passed; `cargo test -p annotagent-application -p annotagent-server --lib --offline` passed with Application 167 passed / 1 ignored and Server 79 passed / 2 ignored.

### Acceptance status after the first G1 slice

- A1: passed. One unavoidable bounded-scope decision; zero technical relay clicks after approval.
- A2: passed. The delayed Builder route returns first, then the original durable Journey reaches the original Sample ID. The immediate-provider route deliberately registers no HTTP observer until after terminal completion, then replays the same consent and execution three times; it retains one Builder, one Sample operation, the same Sample artifact, unchanged calls and unchanged task budget. Duplicate Builder, Sample and dispatch completion notifications cannot rewrite a terminal receipt. Same-database restart preserves the terminal snapshot.
- A3 protocol: passed. Complete upload-then-description and description-before-later-upload both preserve one Task and avoid repeated technical forms. Ambiguous `YOLO` wording produces one concise question, performs one model call before the answer, creates no premature Sample, then resumes the same Journey with the original consent and exact three image IDs after the saved clarification answer.
- A3 UI: passed. `P0-G1-BE-006` now returns stable server-owned choices and an owned answer endpoint. The Thread asks only for the missing output type, preserves the already saved image/label/authorization scope, and marks contour/classification unsupported rather than parsing text or converting them to bounding boxes. The exact answer command survives a lost response and rejects a changed-scope retry.
- A5: passed: three terminal candidates create three Sample-bound HumanRequests, automatically open the result area and initially fit the full image. Polling does not reset same-image zoom/pan.
- A9 sample portion: missing information, ready, approval, running, clarification and review states have current-application screenshots/recordings; review also has dark-theme and 390×844 evidence. Delivered belongs to the later formal/package slice. Native browser 200% zoom remains unverified and is not claimed.

G1 is complete under the isolated TEST-provider boundary. This proves orchestration, persistence, UI scope and review hand-off; it does not prove commercial-model accuracy.

## G2 — review-gated formal processing and delivery

- Integrated Backend review gating as local commit `5ee188f` (source `7831ecf641c5a8320fee852239e7e206db97181b`) and restart-safe package delivery as local commit `c29abff` (source `6e8f336f2e9e4d6ddf7c39650d6f79d4a50f0d05`). Frontend completion is `7b6d405`.
- Current isolated manifest: `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-6r5kxfpa/manifest.json`; backend SHA `c29abff23b2160511749954e8491624584deb51c`; application `127.0.0.1:8898`; scripted provider `127.0.0.1:8899`. It uses an isolated TEST database and does not touch `8788` or the real workspace.
- Before all three exact Sample HumanRequests were applied, both passive formal preview and a deliberately malformed direct confirmation returned structured `409 sample_reviews_pending`. The processing-operation list remained empty: no publication, Batch or provider call was admitted.
- The Sample canvas now offers both “这个样例结果正确” and the existing correction path. A positive answer saves the exact server-issued Sample candidate reference with `reason=correct`; it remains Sandbox feedback and never writes a formal annotation.
- After all three answers, the passive preview returned `sample_review.ready=true` with the exact applied request IDs. One formal processing confirmation created one Batch for the frozen six-image scope. Refresh did not create another processing operation.
- The production-route browser test then accepted 18 task-owned formal objects and saved six explicit whole-image completeness receipts. Only then did the server expose the single `authorize_training_package` action. Persistent Sample lineage no longer overrides that newer server-owned action in the UI selector.
- One package consent let the server create one package job. The frontend did not POST directly to `/delivery-packages`; it polled the existing server job and downloaded a real ZIP. Reload retained the same download without another mutation. The downloaded test artifact SHA-256 is `56d20aff3e582c4e650bce90c1ef8dd525811f48a57ff70e13a64ba637b50069`.
- Current application evidence is in `web/test-results/p0-autonomous-sample-one-b-38fa8-o-three-real-Sample-reviews/`: screenshots `01-ready-to-start.png` through `09-package-ready.png`, plus `p0-autonomous-delivery-TEST.zip`. These are TEST-provider evidence, not commercial-model output or a claim of model accuracy.

### G2 verification

- `npm run typecheck` — passed.
- `npm test` — 119 files / 428 tests passed.
- `npm run build` — passed; 143 modules transformed.
- `AGENT_UI_TEST_URL=http://127.0.0.1:8898 AGENT_UI_TEST_MANIFEST=... npx playwright test --config playwright.integration.config.ts p0-autonomous-sample.spec.ts` — 3/3 passed. The complete Sample → formal processing → formal review → ZIP case completed in 25.6s.
- `cargo fmt --all --check` — passed.
- `cargo test -p annotagent-export -p annotagent-storage -p annotagent-application -p annotagent-server --lib` — Export 2/2; Storage 202/202; Application 167 passed / 1 ignored; Server 79 passed / 2 ignored.
- `cargo clippy -p annotagent-export -p annotagent-storage -p annotagent-application -p annotagent-server --all-targets --all-features -- -D warnings` — passed.

### Acceptance status after the G2 delivery slice

- A4: partially covered. This route proves exact uploaded-image ownership and immutable Sample/formal scope, but the separate 10-project-images / 3-current-images browser matrix and every scope-expansion invalidation case were not rerun here.
- A6: not complete. Structured `sample_reviews_pending` is distinguished from execution failure, and the existing result diagnostics retain separate UI categories, but the full six-error production-route matrix (missing weight, not received, unknown, invalid structure, legal empty and projection failure) was not rerun in this slice.
- A7: partially covered. Browser closure/reopen reaches the same Sample result, processing is locally idempotent, and package admission/recovery has focused Rust coverage. The complete stop, expiry and exhausted-budget browser matrix remains outside this slice.
- A8: passed under TEST. Six formal image decisions lead to one server-owned package authorization, one restart-safe package job and a real downloadable ZIP.
- A9: current-application evidence now includes ready, approval, running, Sample review, Sample-confirmed, formal review and package-ready states. Dark theme and 390×844 are covered at Sample review. Native browser 200% remains unverified and is not claimed.
- Commercial-provider accuracy and real-person novice usability remain unverified. The deterministic external TEST provider proves state progression, ownership, persistence and delivery semantics only.

## G2 close — exact task scope, typed outcomes and lifecycle UI

- Integrated the Backend typed diagnostic contract as local commit `84d5a5a` (source Backend commit `26732aa2dec8678904a1e205684e9e85630ae05d`). The frontend close commit is `2286c00`.
- `GET D/workspace.mainline.result_diagnostics[]` is validated before presentation. The Http Adapter accepts only the seven documented stable codes, exact source identity, `automatic_retry:false`, `preserves_existing_results:true`, and a same-origin GET safe action. It never follows that action automatically.
- The single current-task selector presents only a diagnostic tied to the current Sample, required capability request or failed/in-doubt model-call receipt. Historical failures remain in execution details after a later usable Sample result.
- Legal empty detection remains distinct from an explicitly reviewed negative image. Projection failure never creates a replacement box. When another image still has a valid terminal candidate, that candidate and its review stay visible beside the per-image diagnostic.
- A server-issued unique resume checkpoint now outranks a stale running projection. The checkpoint identifier is passed back verbatim and revalidated by the Http Adapter; a non-resumable or unknown state never gains a continue action.
- Added a production-route A4 browser case that creates seven existing Project images, uploads three new Task images, then proves the Project contains ten images while the Send intake, one Journey approval, Sample inputs and formal processing authorization contain exactly the three new image IDs. No prefix, “latest image” or Project-wide inference is used.
- Current isolated application: `127.0.0.1:8900`; scripted TEST provider: `127.0.0.1:8901`; manifest `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-_usnhgr6/manifest.json`; backend SHA `84d5a5ae6369a19edebec32e6e1c48c0cea9dee5`. The user service on `8788` was not restarted or mutated.

### G2 close verification

- `npm test` — 119 files / 439 tests passed.
- `npm run typecheck` — passed.
- `npm run build` — passed; 143 modules transformed.
- Focused current-state/diagnostic/adapter unit suite — 3 files / 59 tests passed.
- `p0-autonomous-sample.spec.ts` — 4/4 production-route tests passed against the built React application and real Rust HTTP/SQLite service. This includes the complete Sample → formal review → real ZIP path and the ten-Project-images / three-current-task-images scope case.
- Final retained browser evidence is under `web/test-results/p0-final/`: 15 current-application PNGs cover missing images, bounded approval, server progress, desktop/dark/mobile Sample review, formal review, package Ready, ambiguous-output clarification and exact three-of-ten scope. The retained real TEST ZIP SHA-256 is `57cb5fdbd4a800c480ead18ead31c3bef1a4dea0087639f48322f109c57f3371`.
- Actual stop browser case — passed: POST first returned `stopping`, the UI and refresh settled to `远端结果未知`, the underlying receipt remained `in_doubt`, and no resume action appeared.
- Paused Batch browser case — passed on a fresh isolated fixture after seed completion: the exact server checkpoint resumed, retained prior child IDs and completed with three children. The non-resumable control exposed no continue action.
- `cargo fmt --all --check` — passed.
- `cargo test -p annotagent-application mainline_task::tests::` — 3/3 passed.
- `cargo test -p annotagent-server stop_http_trace_keeps_unknown_receipt_and_spent_budget_on_retry` — 1/1 passed.
- `cargo test -p annotagent-storage --test persistent_batches` — 6/6 passed.
- Exact A4 backend scope tests — Server 1/1 and Application 1/1 passed.

### Acceptance status after G2 close

- A4: passed. Exact non-prefix Task ownership is proven in both production-route browser evidence and backend scope/materialization tests.
- A6: protocol and frontend mapping are covered for all seven stable outcomes; mixed legal-empty/projection-failure preservation has component regression coverage. A browser fixture that independently renders all seven failure scenes has not been run, so the complete visual failure matrix remains partial.
- A7: stop → unknown, no fictional resume, exact paused-Batch resume, restart persistence and budget/receipt retention have browser or focused Rust evidence. Expired authorization and exhausted-budget remain covered by lower-level regressions but were not both rerun as current-application browser scenes; the full A7 visual matrix therefore remains partial.
- A8: passed under TEST with a real downloadable ZIP and immutable reviewed scope.
- A9: the complete P0 path has current-application desktop screenshots; Sample review additionally has dark/mobile evidence. Native browser 200% zoom remains unverified and is not claimed.
- Commercial-provider accuracy and real-person novice usability remain unverified. No Fixture or TEST result is relabeled as Live inference.

## Safety boundary

- React reads and presents server state; polling remains GET-only.
- No `useEffect`, task load or route restoration performs a paid or destructive POST.
- Fixture and preset candidates never satisfy live inference acceptance.
- Geometry gates and human review semantics remain unchanged.
