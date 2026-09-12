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

### G0 verification

- `npm test -- --run src/agent-ui/currentTaskPresentation.test.ts src/agent-ui/App.modelSetup.test.ts src/agent-ui/mainline.test.ts` — 14 passed.
- `npm run typecheck` — passed.
- Focused P0 UI unit checks after review-focus hardening — 16 passed.
- Current integrated Web regression at `767718b`: 119 test files / 418 tests passed; production build passed.
- Task usage now defaults to a compact per-task summary, deduplicates physical attempts across owned pages, prevents late pages crossing Task ownership, and reports read failures as unknown rather than zero or raw transport text.
- Frontend 2's isolated component harness is 5/5 under the Vite UI Preview runner only. It is not production-page evidence and is excluded from P0 acceptance; the same `/@fs` harness cannot run under `annotagent serve` (5/5 fail by construction), so a real packaged-route HTTP E2E is still required.
- Review focus is now keyed to the stable server work item, human request or Sample instead of the changing read-model revision. Polling no longer repeatedly steals focus, and the visible review action is only a secondary recovery control because the result opens automatically.
- Real HTTP G1 remains unverified until the isolated upload-to-review trace passes; no production success is claimed from the UI selector or Backend unit tests.
- The remaining A1 blocker is earlier than Builder: uploaded image identities are not yet frozen into the natural-language Send, and the Backend has not yet delivered automatic DeliveryIntake materialization from a complete request.
- A fresh isolated HTTP baseline attempt at integration `ee7a6cd` did not pass: the TEST provider-backed Schema request settled `in_doubt` (`provider_request/provider_error`) and the fixture seed timed out before Builder. The preserved trace is under `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-20zqo0g9`; this is tracked as `P0-F1-HTTP-001` and is not reported as a successful run.

## Safety boundary

- React reads and presents server state; polling remains GET-only.
- No `useEffect`, task load or route restoration performs a paid or destructive POST.
- Fixture and preset candidates never satisfy live inference acceptance.
- Geometry gates and human review semantics remain unchanged.
