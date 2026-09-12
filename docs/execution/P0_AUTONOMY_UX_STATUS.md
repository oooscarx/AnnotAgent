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
- Current integrated Web regression at `767718b`: 119 test files / 418 tests passed; production build passed.
- Task usage now defaults to a compact per-task summary, deduplicates physical attempts across owned pages, prevents late pages crossing Task ownership, and reports read failures as unknown rather than zero or raw transport text.
- Frontend 2's isolated component harness is 5/5 under the Vite UI Preview runner only. It is not production-page evidence and is excluded from P0 acceptance; the same `/@fs` harness cannot run under `annotagent serve` (5/5 fail by construction), so a real packaged-route HTTP E2E is still required.
- Review focus is now keyed to the stable server work item, human request or Sample instead of the changing read-model revision. Polling no longer repeatedly steals focus, and the visible review action is only a secondary recovery control because the result opens automatically.
- The earlier `P0-F1-HTTP-001` baseline at `ee7a6cd` remains recorded as a real pre-fix failure: its TEST provider Schema request settled `in_doubt`; it is not counted as passing evidence.
- A second clean run exposed `P0-F1-HTTP-002`: the scripted Schema response omitted required delivery semantics. Backend fixture commits `046ecd8` and `dbdd753` made the TEST response contract-complete without adding a production fallback or retrying a saved model call.

### G1 real packaged HTTP evidence

- Current integration/backend source SHA: `2f4457f7b20d5e736f7e1d61af7f0c86a1bb1845` (includes Backend exact later-upload SHA `69d8ef1` and Frontend 2 Fit SHA `5dae4db`).
- Current isolated external-model-only manifest: `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-verslz90/manifest.json`; service `127.0.0.1:8886`, scripted provider `127.0.0.1:8887`. The user service on `8788` was not restarted or mutated.
- Permanent production-route test: `web/e2e/p0-autonomous-sample.spec.ts` — both tests passed (8.5s + 3.8s) against the built React application and real Rust HTTP/SQLite service. The main request explicitly asks for cup + bottle bounding boxes for an Ultralytics YOLO target, matching the deterministic provider's saved delivery semantics.
- The browser uploaded 6 real demo-pack images. The exact 6 `{image_id, sha256}` receipts were frozen into Send; the server selected exactly 3 Sample inputs.
- The browser made one `POST .../journey-consents` and zero `POST .../execution` calls. After the page closed, the durable worker completed Schema → Builder → Sample and created exactly 3 pending HumanRequests bound to the same task and Sample operation.
- Reopening the exact task showed `检查样例结果 · 3 张` and `3 个结果需要人工判断`; reloading retained the same task/result without a mutation.
- The first Builder transport deliberately waited four seconds. Current seed consent admission returned in 13ms; the first passive status read was `dispatch=running` with `sample=null`; the original authorized Sample reached review after 4.73s. No second POST or replacement Sample was created.
- After Sample completion, the sole server action is the read-only `review_sample_results` action with the exact Sample ID and three pending HumanRequest IDs. Formal processing is not offered before those decisions.
- A second production route sends the complete requirement before uploading anything, records the missing-image state, uploads six files into that same Task through one exact delivery-intent CAS, and reaches the same single combined action without Sample execution. It issues no Journey consent or `/execution` write before approval. The adapter unit regression also proves exact replay after a lost delivery response and reload.
- The continuous TEST browser recording and six current-application screenshots are under `web/test-results/p0-autonomous-sample-one-b-38fa8-o-three-real-Sample-reviews/` (`01-ready-to-start` through `06-sample-review-mobile`, plus `video.webm`). Missing-information and recovered ready-state evidence plus a second recording are under `web/test-results/p0-autonomous-sample-descr-9cb76-k-and-exact-six-image-scope/`.
- Visual inspection confirms the whole source image and all three terminal boxes are visible on initial desktop review after the Fit correction. The same real result is readable in system dark theme and at a 390×844 viewport. The 720-CSS-pixel layout-pressure check from Frontend 2 is not relabeled as native browser 200% zoom.
- The TEST provider proves orchestration and recovery, not commercial-model accuracy. Formal processing, dataset review and package export remain separately authorized scopes.

### Acceptance status after the first G1 slice

- A1: passed. One unavoidable bounded-scope decision; zero technical relay clicks after approval.
- A2: delayed Builder path passed through real HTTP. Synchronous completion and duplicate-wakeup unit coverage are retained in Backend tests; the race-specific HTTP permutations remain to be recorded explicitly.
- A3: complete upload-then-description and description-before-later-upload both preserve one Task and avoid repeated technical forms. The ambiguous `YOLO` wording case is still open and is not claimed.
- A5: passed: three terminal candidates create three Sample-bound HumanRequests, automatically open the result area and initially fit the full image. Polling does not reset same-image zoom/pan.
- A9 sample portion: missing information, ready, approval, running and review states have current-application screenshots/recordings; review also has dark-theme and 390×844 evidence. Delivered belongs to the later formal/package slice. Native browser 200% zoom remains unverified, and the A2 race/A3 ambiguous-language cases still prevent declaring G1 complete.

## Safety boundary

- React reads and presents server state; polling remains GET-only.
- No `useEffect`, task load or route restoration performs a paid or destructive POST.
- Fixture and preset candidates never satisfy live inference acceptance.
- Geometry gates and human review semantics remain unchanged.
