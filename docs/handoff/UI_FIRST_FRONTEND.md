# Agent A — UI Preview handoff

## Open and run

Separate worktree: `/Users/oscar/Documents/my_workspace/AnnotAgent-frontend`.

```sh
cd /Users/oscar/Documents/my_workspace/AnnotAgent-frontend/web
npm ci
npm run dev:ui-preview
```

- React preview: `http://127.0.0.1:5174/ui-preview?task=new`
- Settings: `http://127.0.0.1:5174/ui-preview?task=new&settings=general`
- Reference/actual gallery: `http://127.0.0.1:5174/evidence/compare.html`
- Screenshot provenance: `web/ui-preview/evidence/manifest.json` — per-image source SHA, URL, dimensions, theme and Fixture label.
- User server 8787 is unchanged. Preview 5174 has its own dependencies/build and no API proxy. No real Provider credential, workspace database or model asset was copied.

## What is implemented

Actual React pages, not prototype HTML or a screenshot UI. First-version Paper & Graphite tokens and monochrome brand reused. One project/task sidebar, 52px main header, 720px thread/composer, optional resizable 54/46 image area. Production App is unchanged until visual approval.

| Surface | Default visible controls | On demand |
| --- | --- | --- |
| Navigation | New task, task search, expandable project groups, Settings | Project management explanation; original production management remains intact |
| New task | Goal input, image attachments, Plan/Execute, model, Send | Image pane and grouped model picker |
| Plan | User/assistant text, plan steps, approve scope | Exact revision/model details |
| Running | Actual fixture phase, Stop, Composer, queue summary | Execution record, historic Plan and queued text |
| Interrupted | Stopped receipt, Continue, Composer | Prior plan and image evidence |
| Human request | Current question, locate target, correction action | Compare, original, numeric annotation list, undo, selected-object reference |
| General | Theme, navigation language, font, density, collapsed-project preference | Preview load/empty/error controls |
| Providers | Account list, add/edit/probe/delete | Editor with validation and simulated credential slot; deletion impact dialog |
| Agent models | Default planner, grouped models, unavailable reasons | Same model data as Composer |
| Vision/plugins | Status/version/compatibility, install | Simulated approval, installing, verification failure and retry |
| Data/privacy | Workspace description, external-data preference, protected cache | Cleanup preview/confirmation |
| Usage/budget | Range, decimal budget, compact usage rows | Explicit empty/loading/error scenarios; unknown is not zero |

All six settings pages expose loading, empty, read error and recovery demonstrations. Settings edits support save/cancel, failed-save preservation and unsaved-navigation protection. The account editor does not show a second set of Settings save buttons.

## Fixture / real boundary

- Every page states **UI 预览 · 演示数据**. Images are original illustrations supplied in the reference kit, boxes are manual fixtures. Not photographs, inference evidence or an accuracy benchmark.
- The Fixture Adapter is imported only by `web/ui-preview/main.tsx`. Production build does not include its localStorage namespace/simulator. `/api` is rejected by the preview server, CSP limits external connections, browser tests audit requests.
- Browser storage namespace: `annotagent.ui-preview.v1`. It persists simulated tasks, drafts, settings and per-image editing drafts. Selected local files are memory-only previews, not uploaded; UI explicitly warns that refresh requires reselection.
- Send→planning→awaiting approval, revision-bound approval→running, stop→stopping→interrupted, eligible resume, queued input and human-save success/failure are local simulations. Queue does **not** automatically dispatch. No fake completion is generated from an HTTP error.
- Stopping survives a preview refresh and completes its simulated receipt. A interrupted simulated planning timer after reload becomes `outcome_unknown`; this is not represented as resumable remote work.
- Settings connection tests/install/cleanup are explicit simulations, not side effects. No real secret field exists. Detailed preview-state controls are separate from product content.
- Existing production data/automation/run/review/export/settings/trash routes and CRUD were not deleted or modified. Preview project menu does not falsely claim to perform those operations.

## Adapter and integration seam

`web/src/agent-ui/adapter.ts` defines the single ViewModel and operation interface; React reads its cached immutable snapshot through `useSyncExternalStore`. Navigation, tasks, thread, models, settings, image metadata and usage come from this adapter. `FixtureAdapter` is an isolated implementation, not a new runtime engine.

Logical list/get operations currently share the snapshot projection. Commands carry stable owner/task IDs, opaque revision, command ID and optional selected candidate reference. Planning model choice does not mutate frozen in-flight model. Plan details and privacy/usage ranges must continue to come from the normalized backend objects, never be inferred from active project names.

**Http Adapter is not implemented or connected.** The unit test checks a serialized transport projection against the same TypeScript ViewModel; it is not a real HTTP contract/conformance test. Integration is authorized only after user visual confirmation and must use a dedicated integration branch, without overwriting Agent B files.

### cross_boundary_requests

1. Map B's committed HTTP bindings to the existing UI ViewModel, including owner IDs, pagination, provider/model availability and errors. Replace bounded preview image IDs with actual server IDs at the normalization boundary.
2. Add explicit server-provided `availableActions` and reasons to normalized task operation controls before live enablement. Current fixture controls use its deterministic phase transitions; do not trust client phase alone for real authorization.
3. Bind approvals to exact Plan/Draft revision, recipients, image scope, model set, budget and expiry. The fixture task revision is sufficient for preview interaction only; it is not a substitute for backend approval grants.
4. Connect persistent command receipts, queue entry IDs/object references, checkpoint/resume and event replay/reconciliation. Preview queue is display-only and command de-duplication is process-local; don't claim durable backend exactly-once semantics.
5. Map human request ID/candidate revision and sandbox-vs-formal annotation scopes. Fixture currently demonstrates the first image's correction request; the backend must provide the actual request and artifact lineage.
6. Replace preview credential slots with a controlled write-only credential editor and backend deletion-impact/probe/install responses. Never return secret contents or auto-probe models.
7. Real management menu links should point at existing canonical owner-scoped routes; no duplicated CRUD. Preserve return task/image and dirty editing state.

## Validation and limits

- Web typecheck/token check, 252 unit tests, production build and isolated preview build passed on the final UI source rerun.
- 21 preview Playwright tests: settings state matrices; save/cancel/failure; provider validation/unknown probe; install verification; privacy protection; no business requests; sidebar/header/composer measurements; IME no-send and Shift+Enter; approval/queue/stop/resume; draft/pane/image restoration; failed human save; responsive 1440×900, 1280×720, 1024×768 and 390×844.
- 17 actual page screenshots, 1440×960 desktop / 390×844 mobile. Gallery compares 5 original reference states with actual React captures. Screenshots are not live model evidence.
- Native browser 200% zoom, native OS Chinese IME, screen reader, real inference/accuracy, multi-user/live event races, and human usability testing **not executed**. Synthetic keyboard tests and viewport tests are not equivalent. English covers navigation and new-task entry; detailed demo descriptions are still Chinese.
- Rust tests not run because no Rust was modified. Production's existing >500kB JS bundle warning remains; preview is a separate approximately 235kB JS bundle.

## Commits / ownership

- F0 `494fdd0`: visible isolated React entry/sidebar/settings foundation.
- F1 `94704f0`: controls, dialog focus, recovery, scoped drafts, fixture tests.
- F2 `8fd7f61`: six Settings flows and immutable snapshot fix.
- F3 UI `1becde8`, `ae60df4`, `452ea02`: responsive/evidence tests, current-task hierarchy, selected references, adapter-sourced assets/usage.
- Final evidence/status commit follows these source commits; capture metadata identifies the actual UI source commit independently.

Branch `codex/agent-ui-frontend`. No push, no remote changes, no Rust/B-owned files, no user data cleanup, no reset/rebase/amend. Stop at visual review; do not start live integration automatically.
