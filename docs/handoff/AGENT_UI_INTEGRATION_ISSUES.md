# Agent UI integration — decisions and interface issues

## Current delta (supersedes earlier progress snapshots)

Production entry is now wired in the integration branch: `web/src/main.tsx`, `web/src/agent-ui/routes.ts`, `web/src/agent-ui/http.ts`. Management uses existing `App.tsx` via a separate code/CSS entry; Project work links return to the approved UI. `/ui-preview` is not the production adapter. Eight independent real HTTP browser cases passed; sample-to-processing extension confirms one start on double click. Web unit suite passed 260 tests/54 files before latest queue/upload additions. Final regression is pending.

UIAPI-002 verified: backend `4676652` merged at `ac54a92`; own port regression `python3 -m unittest discover -s crates/annotagent-e2e-fixture/support -p test_http_fixture_ports.py` passed 3 tests. New isolated fixture started successfully at 8796/8797 using production `web/dist`; no Origin rewrite, no fixture fallback, no 8787 takeover.

### UIAPI-003 — authentic geometry and long-running control browser evidence (P1)

Sent message `01a085e7-8460-7091-aad2-eb3423e58a58` to fixed backend UUID. Integration base `ac54a92`. Current main fixture emits classification terminal results and a classification HumanRequest; it cannot verify pixel→normalized bbox saving. Need committed TEST bbox candidate + pending HumanRequest (image dimensions, candidate ID, feedback revision) and existing delayed/failure Provider procedure for real stop transitions / unknown outcome. Asked backend to change only its fixture/support ownership, not production model behavior. Awaiting delivery; frontend continues unrelated work. Do not fabricate JSON/browser responses to satisfy this evidence.

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
