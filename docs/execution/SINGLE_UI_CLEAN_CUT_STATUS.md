# Single UI Clean Cut

## Baseline and boundaries

2026-09-09: `main`, baseline `0c3e396a17280c50e23aed301d89248b0ace1e2c`, initially clean, six local commits ahead of origin. No AGENTS.md found in this checkout or applicable ancestors. No remote or real workspace changes. No paid calls, downloads, service restarts, or publication changes.

The new task supersedes legacy URL compatibility. Stable business IDs, backend APIs and management capabilities must survive. The old root cannot be removed until its exclusive capabilities have new working controls.

## Capability inventory (verified in current source)

| Capability | Current implementation | Migration requirement |
| --- | --- | --- |
| Agent conversation, task tree, model preference, receipts, queue, human edit | `agent-ui/App.tsx`, `http.ts`, `ArtifactPane.tsx` | Preserve current protocol and canvas geometry |
| Basic settings, providers, agent preference | `agent-ui/Settings.tsx`, `http.ts` | Real standalone section routes, typed return context |
| Plugin package inspection/install, permissions, weight provisioning, tests, enable/uninstall | old `App.tsx` ExpertModelPluginsPage; reusable `api.ts` methods | New controls; never embed old page |
| Model Bundle catalog, import/license, install operations, references, compatibility, smoke test | old ExpertModelPluginsPage; `/api/model-bundles`, `/api/model-installations` | New native settings panels and explicit confirmations |
| Vision profiles and HTTP workers | old ModelRegistryPage/VisionWorkersRegistryPage | Preserve actual capabilities and saved bindings |
| Storage configuration | old SettingsPage; `api.settings/saveSettings` | New storage UI; separately verify statistics and cleanup endpoints before claiming support |
| Project create/import, schema, export | old ProjectsPage/ProjectPage/ProjectExportPage | New management content and scope-preserving actions |
| Workflow drafts, versions, static checks, sample tests, publish | old WorkflowsPage | Migrate complete operations; do not create new identity type |
| Run/Batch, Review, source/debug/Replay | old RunsPage/BatchDetailPage/ReviewPage | Retain owner verification, geometry, editing and controls |
| Lifecycle impact, delete/restore/purge/trash | old management actions and ProjectTrashPage | New management UI before removing old entry |

Confirmed split in `main.tsx`: unmatched routes dynamically import old `App` and `styles.css`. Confirmed `managementLinks` in new Settings points to old controls. Existing route tests explicitly require this fallback. These are unresolved M2 cutover blockers, not completed work.

## M0 — in progress

- Added shared native `Disclosure`, one decorative 14px SVG indicator, scoped marker suppression, reduced-motion support. Applied to Plan/history, receipts, queue, secondary actions, compatibility and editable annotation list. Project tree now uses the same rotating 14px indicator.
- Updated tracked production and Preview sprite copies with the requested softly curved right chevron. No new icon library. Original current brand inventory does not contain this UI sprite; tracked copies remain synchronized by existing tests.
- Added typed clean-cut route contract and exhaustive new-section/legacy-rejection tests. This is a **cutover target**, not yet wired to production. Current `routes.ts` remains until management migration; do not claim two production routers or successful legacy removal.
- Explicit pending test records production-entry cutover instead of marking it passing prematurely.

### Verification

- Web typecheck and production build passed (build output `/tmp/annotagent-single-ui-m0-dist`, not user `web/dist`).
- Unit: 278 passed, 1 explicitly pending cutover test.
- Existing Preview E2E: 27 passed, including IME, single send, queue, dirty settings, canvas transform and responsive layouts. Fixture only, not real inference/install verification.
- New disclosure-specific keyboard/browser test passed: native Enter/Space toggle, one SVG, no nested button, hidden marker, reduced motion. Total browser coverage: 28 tests passed across two invocations.
- Actual React screenshots: `/tmp/annotagent-single-ui-m0-evidence/`. Existing screenshot manifest records baseline SHA plus current working-tree changes; these are not pristine baseline screenshots. No prototype screenshot used as application evidence.
- Isolated Preview port 5182. User 8787/8788 services untouched. Rust and real HTTP migration E2E not run in M0.

Committed visual evidence: `single-ui-clean-cut/m0/disclosure-expanded.png` (1440×900) and `disclosure-mobile.png` (390×844), DPR 1, light theme, reduced motion enabled, actual React Fixture app at `http://127.0.0.1:5182/ui-preview?task=new`. Source is baseline `0c3e396` plus the source changes in this M0 commit, not baseline-only screenshots. Expanded chevron verified as `matrix(0, 1, -1, 0, 0, 0)` before capture. No real workspace access or model invocation.

## Remaining

Complete M0 browser evidence/inventory, then M1 native plugin/bundle/storage operations; M2 all remaining management, standalone Settings and typed return, remove old root/routes/styles only afterward; M3 isolated real HTTP and production-module-manifest regressions. The requested single-UI migration is **not complete**.
