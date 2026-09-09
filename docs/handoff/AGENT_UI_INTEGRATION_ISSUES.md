# Agent UI integration — decisions and interface issues

## UIAPI-010 — Model Profile update CAS (open)

Baseline `87e97a4` plus native ModelProfileEditor migration. Actual isolated HTTP: PATCH `/api/model-profiles/:id` including `revision` returns 422. `UpdateModelProfileRequest` in server lib.rs denies unknown fields and provides no expected-revision field. Native UI now sends supported editable fields only, with a GET/revision preflight; this is explicitly not atomic CAS. Two concurrent editors remain a final acceptance limitation.

Requested optional `expected_revision`, atomic comparison with current stored revision, stale 409 response with safe revision information, two-writer regression and immutable published-binding preservation. Queued once to confirmed Backend UUID `01a0855e-9c39-7c33-9f18-93e084d14816`, accepted queue ID `01a086c4-358f-7cf0-a290-28dbca6b2cfd`. Delivery not verified. No backend files edited, no real workspace mutations or paid calls. UIAPI-009 history cutoff is also still awaiting verified delivery; queue acceptance alone is not a running process or completed work.

## UIAPI-008 — execution feedback and streaming investigation (2026-09-09)

User authorized progress/error repair and investigation of streaming. Queued request `01a08659-4bb7-7fb2-9b4a-1aa70a0e3e19` to the pinned Backend task UUID, not another project. Main baseline `83d2adb`; frontend owns Web only. Backend proposed additive nullable `started_at`, `completed_at`, `duration_ms`, `stage` and safe typed `failure {stage,category,http_status}`. No raw Provider response or credential is requested. Legacy unknown results must not be relabeled successful or automatically retried.

Frontend now displays the current receipt outside collapsed history, distinguishes local-ended/remote-unknown from running, renders available persisted timing and typed error categories, and retains IDs/history. Serial completion-based polling replaces overlapping two-second requests. An old unknown call no longer masks a newer active call. Unknown legacy timestamps remain unknown. Web unit 272/272, typecheck and production build passed; backend integration pending at this point.

Streaming audit: existing registry `bytes_stream()` only bounds response accumulation; it is not model-text streaming. Current Schema `VisionModelProvider.complete()` returns the whole structured result, with no incremental callback. No simulated token stream or chain-of-thought display added. Real stage feedback and token streaming are separate capabilities; backend delivery and final limitations recorded below when verified.

Delivery: reviewed backend `34436fd46dbc670b41fbbddd107e1552397aaf14`, integrated as `90fa84c`. Contract: `docs/contracts/agent-ui-v1/UIAPI-008_PROGRESS.md`. Own integration verification: cargo fmt, production binary build, 16 OpenAI-compatible Provider tests and one persisted-progress/legacy migration test passed. Backend delivery separately verified 563 related Rust tests (3 existing ignored), strict workspace all-targets Clippy, and an isolated 278-request HTTP seed with 42 receipt observations. Own Web: 273 unit tests, 27 Preview/browser tests, typecheck and production build passed. Full all-features Rust suite was not rerun by integration.

Deployed the new binary only to owned 8788 after verifying zero reserved calls and making an owner-only SQLite backup at `workspace/.annotagent/pre-progress-0PRiys/history.db`. 8787 was not restarted. Read-only real HTTP and desktop/mobile browser refresh prove the old call remains in_doubt, started_at is the original persisted timestamp, missing end/duration/failure stay null, and no new requests are created. Final screenshots and provenance: `docs/execution/call-progress/manifest.json`. Earlier `legacy-unknown-1440.png` is the pre-backend UI screenshot at frontend a1ec9c3, same live URL, 1440x900, DPR1, light theme, GET-only.

No new paid model calls were made during this repair. No generic retry/resume or token delta endpoint was introduced. Current-stage polling is not Provider streaming; completed transport with invalid structured output is presented as validation failure, not a successful Plan. Final UI/source commits are a1ec9c3, 7910a12, df8a0cb and integrated backend 90fa84c. No push.

## Verified delivery — 2026-09-09 (authoritative current status)

Application source: `9cbdbbf89a` (frozen-model fix); capture tooling: `34bd750172`. Approved frontend, initial backend and common base remain pinned below. Backend fixes are integrated through `334f6713adc1e9d45819cf94eff1f77a80a49a67`. Main and the original frontend branch have not moved. This is a local integration delivery, not a merge to main or a deployment to the real workspace.

- Open the real HTTP application at `http://127.0.0.1:8804/projects`.
- Actual application gallery: `http://127.0.0.1:8804/evidence/agent-ui-integration/34bd750172-1788955887602/index.html`. Its manifest records source SHA, URL, viewport, theme and TEST status per frame. Includes new task, Plan authorization, sample, saved bbox, dark/mobile canvas, stopped/unknown outcomes, model picker, six Settings pages and real running/queue capture. Initial stopping is evidenced by the actual POST response, not artificially held for a screenshot.
- Isolated persisted workspace: `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-dnlfrg0f`; external TEST provider port 8805. 8787 and the approved 5174 Preview remain untouched.

### Integration scope and remaining limitations

| Area | Verified connection / boundary |
| --- | --- |
| Project/task tree and images | Real owned navigation, thread journal, upload and image reads; refresh/Back and foreign-owner rejection |
| Settings and model picker | Six sections read real/local appropriately scoped state; Provider metadata and write-only workspace credentials, defaults, next-request CAS, future budget save. Plugin installation reuses the existing management permission/license flow; no simulated install in production |
| Goal/Plan/sample | Real Send and separately scoped Plan/Journey grants, persisted terminal results. Switching next-request preference no longer overrides a task's frozen Send model (9cbdbbf). No request failure falls back to Fixture |
| Stop/continue/queue | Actual stopping and unknown receipts; only available checkpoint resume. Supplements persist and require explicit planning authorization. No generic resume or automatic queue dispatcher promised |
| Human correction | Current classification/bbox HumanRequest saves exact sandbox feedback; normalized bbox conversion verified; formal accepted count unchanged. New UI editing is disabled without a pending supported HumanRequest. Adding missed boxes and candidate-reference-to-Agent submission are NOT wired in this adapter |
| Batch/results/export | Exact publish/start authorization and idempotency, real Native download. Formal Batch/Run results currently open retained project management detail; the new inline canvas covers sample/human evidence |
| Management | Header project menu opens existing management, pipelines/history/review/export and their deletion/recovery/trash controls. Typed same-project return task/image/pane preserved. No Runs/Review/Pipeline global feature sidebar |

Fixture simulation remains only in the separate UI Preview. The isolated integration service uses real HTTP, SQLite, CSRF and application services with a clearly marked external TEST model. The synthetic image is a geometry/transport fixture, NOT a cup detector or accuracy demonstration. There is no all-system spending total. Detailed English localization, native OS IME, native 200% zoom and real-human usability remain unverified; automated composition events are not native IME evidence. All legacy browser suites were not rerun.

### Actual final checks

- Full fresh-seed HTTP E2E: **16/16 passed** at 1bbec9d. After the frozen-model correction, affected Plan/Journey cases **2/2 passed**, stop recheck **1/1 passed**, and screenshot traversal successfully previews an old frozen task despite the changed preference.
- Latest web typecheck + unit: **54 files / 264 tests passed**. Production build passed; existing large legacy management chunk warning remains. Preview build passed; isolated Preview E2E **20/20 passed** (capture-only case excluded to avoid overwriting approved evidence).
- Rust fmt, strict all-target/all-feature clippy, workspace all-feature tests and build passed through merged 3d1892e. Seven existing ignored tests: four real-weight ONNX tests, one paid provider smoke, two foreground/subprocess harness cases. No Rust changed after that regression.
- No paid model, model install, real workspace mutation, main merge, remote change or push. Generated incomplete capture attempts were moved outside the repository to `/tmp/annotagent-capture-drafts.BBPLhW`, not deleted.

### Reproduce in an isolated workspace

From the integration worktree, run `npm --prefix web run build`, then:

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --port 8804 --provider-port 8805 --web-dist "$PWD/web/dist"
```

Use free independent ports if occupied; do not kill another service. The launcher prints its marked TEST manifest. In `web`, run the browser suite with `AGENT_UI_TEST_URL` and `AGENT_UI_TEST_MANIFEST` set to that service/manifest and `npx playwright test --config playwright.integration.config.ts`. Use a fresh seed for the full suite: paused-batch and pending-human tests intentionally consume their fixture states. Capture with `node scripts/capture-agent-http.mjs /absolute/TEST/manifest.json`, then rebuild to serve the generated gallery. This is not a command to restart or migrate the user's production workspace.

## Current delta (supersedes earlier progress snapshots)

Production entry is now wired in the integration branch: `web/src/main.tsx`, `web/src/agent-ui/routes.ts`, `web/src/agent-ui/http.ts`. Management uses existing `App.tsx` via a separate code/CSS entry; Project work links return to the approved UI. `/ui-preview` is not the production adapter. Eight independent real HTTP browser cases passed; sample-to-processing extension confirms one start on double click. Web unit suite passed 260 tests/54 files before latest queue/upload additions. Final regression is pending.

UIAPI-002 verified: backend `4676652` merged at `ac54a92`; own port regression `python3 -m unittest discover -s crates/annotagent-e2e-fixture/support -p test_http_fixture_ports.py` passed 3 tests. New isolated fixture started successfully at 8796/8797 using production `web/dist`; no Origin rewrite, no fixture fallback, no 8787 takeover.

### UIAPI-003 — authentic geometry and long-running control browser evidence (P1)

Sent message `01a085e7-8460-7091-aad2-eb3423e58a58` to fixed backend UUID. Integration base `ac54a92`. Current main fixture emits classification terminal results and a classification HumanRequest; it cannot verify pixel→normalized bbox saving. Need committed TEST bbox candidate + pending HumanRequest (image dimensions, candidate ID, feedback revision) and existing delayed/failure Provider procedure for real stop transitions / unknown outcome. Asked backend to change only its fixture/support ownership, not production model behavior. Awaiting delivery; frontend continues unrelated work. Do not fabricate JSON/browser responses to satisfy this evidence.

Resolved by backend `26720fc30b71b3d3b39c93d43cce193d86d2a170`, merged `dea1f46`. Verified new seed and actual browser bbox save + stop sequence. Frontend also fixed two contract assumptions: opaque resume_checkpoint_ref is NOT a Draft ID, and the HumanRequest kind comes from its exact terminal candidate. An early empty canvas was persisting an empty edit list and hiding the subsequent real candidate; canvas now waits for owned evidence, and edit persistence binds exact resultRevision. TEST source pixels remain unchanged.

### UIAPI-004 — preview admits a pending-human queue grant that execution rejects (P2)

Sent `01a085f1-2af1-7b10-bde2-912c23455971`, integration `a8a2c47`. With pending HumanRequest, supplement Send succeeds; queue schema-preview returns a grant scope; schema-proposals records authorized/planning_call_id but rejects with `Task is waiting for human input; no model call was admitted review_request_or_reload_scope`. No call ran. Frontend now suppresses new queue planning while a pending human question exists, retains the exact unknown/failed grant for explicit recovery and does not pretend the queue completed. Requested backend preview/admission consistency and a documented same-ID recovery contract. Fresh-task queue text planning is separately verified through actual HTTP; not an automatic dispatcher.

Backend delivered `334f6713adc1e9d45819cf94eff1f77a80a49a67`, merged `3d1892e`. Reviewed application preflight, transactional storage admission, server 409 admitted=false mapping and same-grant recovery tests. Own full Rust fmt/clippy/test/build passed. Frontend final fresh-seed HTTP regression additionally checks pending-human preview returns 409 and creates no authorization. No frontend-owned Rust edits.

### Frontend defects caught by HTTP regression

- A late evidence read spread an earlier Task snapshot over newly typed input; fixed by merging onto the current Task at emission. Added a delayed-read regression.
- Existing management canonicalization discarded typed return_task/image/pane keys; now preserves validated same-project keys across canonicalization and management navigation. Foreign owner and arbitrary external return parameters are rejected. Browser entry/Back and route unit tests cover this.
- Preview network-isolation assertion hardcoded 5174; now compares against configured same-origin baseURL, allowing independent 5176 tests without reusing the user's approved preview server.

Current test service: `http://127.0.0.1:8796`, marked workspace `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-2ul2_zxk`. Runtime backend base recorded as `ac54a92`; UI served from production build, not old dist-integration. Fixture model accuracy is not Live accuracy.

## Frozen inputs (2026-09-09)

- User approved the existing Paper & Graphite UI and explicitly authorized integration.
- Approved frontend delivery: `f0bbbc692904aef9f89392bd4cbb1ba3cd79172a`; screenshot/source baseline within that delivery: `68f2568dc2e9ef3ed3119a0401ec37bb1a2685c5` (same UI pixels/code).
- Backend delivery: `ce46c6f7e52da4a1e2fa216256351d8689d05fa5`.
- Common base: `c41b281b49252d520117029d39611865133798af`.
- Integration branch/worktree: `codex/agent-ui-integration`, `/Users/oscar/Documents/my_workspace/AnnotAgent-integration`.
- Pinned merge: `3ddaa64eef3771dd1512f6a58a273663bacc5678`; no conflicts. Neither source branch moved.
- Original main worktree contains unrelated screenshot modifications/untracked design files; preserved. Backend worktree has in-progress `lib.rs`, HTTP bindings and fixture support; not copied or edited.
- No production workspace, 8787 service, credentials or published versions touched. No push or main merge.

## Communication

UIAPI-000: default `/opt/homebrew/bin/codex queue --help` failed with missing packaged binary (ENOENT). Existing second PATH executable `/Applications/ChatGPT.app/Contents/Resources/codex queue --help` succeeded. No installation/configuration or private database access used.

Communication test and pinned-delivery request queued successfully, message `01a085be-daea-7073-b7b7-22e1ca4a2d03`. Confirmed backend thread UUID **`01a0855e-9c39-7c33-9f18-93e084d14816`**; use this UUID for future messages. No ACK loop. Backend owns all Rust changes.

## Integration sequence

1. Project/task navigation, exact owned thread and images; read-only recovery and failure tests.
2. Safe settings and next-request model CAS.
3. Send receipt → exact preview → approval → saved sample output.
4. Durable stop observation, capability-based resume and explicit queue.
5. Revision-bound human answer and continuation.
6. Processing and export; production entry only after HTTP verification.

Each step is incomplete until independently tested. Fixture behavior is not HTTP evidence. No automatic fallback, unapproved external call, global fee aggregation or speculative resume.

## Verified integration progress

- Initial adapter/read-only slice: `1cc2063`. Five synthetic transport regressions (not HTTP E2E), original Fixture adapter tests, typecheck and preview build passed.
- Backend HTTP fixture delivery `b8955cabcca72d87b1ed27a5cb132789ca535b60` plus documentation `dd98168` merged at `587eee426c65bd84e2685def984aab978509298a` after reading the committed launcher/seed. No Rust authored by frontend.
- Real browser HTTP tests passed independently: (a) owned messages/images and Back/refresh without writes; (b) six Settings reads and preference/return recovery; (c) IME, double Send, persisted task, no calls on refresh; (c) explicit text-Plan consent reaches loopback TEST provider once and survives refresh; (e) exact HumanRequest classification correction persists in Sandbox and restores on refresh. Latest run: 5/5. These do NOT establish bbox correction, Sample invocation, stop transitions, Batch or Export integration yet.
- Provider Registry read/write, default-model settings, next-request model CAS and future-budget PATCH code are wired; their write-path HTTP verification is still pending. Never count browser preference saving as proof of remote budget writes.
- Raw HTTP viewport seen at 1440×960; approved Project tree, Header, Composer and optional canvas are retained. Broken integration logo URL fixed using existing production brand asset. No Fixture imported by HTTP entry.
- Integration entry remains temporary `/agent-integration.html`; production entry is NOT replaced yet. Remaining work must include production routing and management return before declaring completion.

## UIAPI-001 — HTTP fixture / object fields

Requested existing real Router+SQLite seed and exact Plan source; backend delivered `28d5ec3`, including control scenes and docs. Verified actual `workspace.builder_operations` is `{items:[...]}` and adapted that shape (initial browser test caught `.map` on wrapper). `thread` remains only persisted user messages; structured decisions are labelled separately. No fabricated assistant success.

## UIAPI-002 — listener restart probe (P1; workaround available)

At integration `587eee4`: stop owned fixture launcher; `lsof` shows no LISTEN on 8792/8793; immediate launcher restart with the same marked TEST workspace fails at Python `socket.bind` with `Errno 48 Address already in use`. Request sent to fixed backend UUID, message `01a085d1-221b-72d1-be4e-1bb9d255907a`. Expected: recognize reusable TIME_WAIT without stealing active listeners; keep 8787 protection.

Backend delivered `4676652` with SO_REUSEADDR + actual listen probe and tests; integration verification pending. Workaround: fresh isolated fixture on 8794/8795. Original 8787 and UI Preview 5174 untouched. No existing listener killed.

## Local test hosting

Cross-port Vite proxy writes correctly failed Origin validation. Chose actual same-origin Rust hosting of `web/dist-integration` instead of relaxing security/rewriting Origin. Integration build emits both `agent-integration.html` and SPA `index.html`; missing latter was caught as real 404 and fixed in frontend build configuration. Production `main.tsx` still untouched.

Current test instance: `http://127.0.0.1:8794`, TEST workspace `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-0idu026x`. Actual requests all use original cookie/CSRF/privileged-confirmation flow. E2E refuses any server without `x-annotagent-fixture: external-model-only`. Test inference is deterministic, not commercial-model evidence.
## UIAPI-017 — bounded recoverable node Replay (P1, pending)

Integration6c4b4d8. Inspected server replay_run_from_node and application replay_run_from_node: existing POST /api/runs/{run}/replay/{node} accepts no owned preview/hash/command body, executes sandbox synchronously, and refuses live-model descendants with "Replay of live model nodes requires an explicit current binding". Native RunInspector cannot safely present a live replay action or recover a lost response using this contract. No live request attempted.

Requested read-only exact owned preview of source snapshot/checkpoint, downstream nodes/models/destinations, bounded limits and unknown cost; explicit authorized current bindings; durable command + GET receipt recovery without repeated execution; preservation of upstream checkpoint, formal annotations and original Published Version. Deliver contract, focused server/application tests and commit SHA; unsupported scopes must be explicit. Backend owns Rust; frontend cleanup continues independently. Sent to fixed authorized UUID01a0855e-9c39-7c33-9f18-93e084d14816, queue receipt01a08783-4643-73d1-8c89-831fe92f3221. No paid inference, real-data modification or remote action authorized by this request.
