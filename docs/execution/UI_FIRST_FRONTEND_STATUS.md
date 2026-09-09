# UI First Frontend — Agent A

Current: **F0–F3 UI Preview delivered for visual review; no HTTP integration authorized or started.** Final source commit `452ea027d1c87488ba6d34c392ade996681d979b`.

## Baseline and boundaries

- Base: `c41b281b49252d520117029d39611865133798af`.
- Branch: `codex/agent-ui-frontend`; separate worktree `AnnotAgent-frontend`.
- Read Shared Contract, Frontend Prompt, design spec, token and asset attribution; inspected all eight reference screenshots. The new user request supersedes the no-sidebar design. Original prototype is reference only, not application evidence.
- No Rust, backend contracts, production workspace, credentials or remote changes. Production 8787 remains untouched. Existing production App and all management routes remain intact.

## F0 — visible isolated baseline

- Separate Vite entry on 5174, no proxy, rejects `/api`, CSP restricts external connections. Production build does not import the Fixture simulator.
- Transport-neutral WorkspaceAdapter; preview-only state and commands persisted under `annotagent.ui-preview.v1`.
- Actual React project sidebar, new task, General and Providers pages; initial versions of remaining settings/thread/artifact components included for continued F1/F2 refinement.
- Canonical generated Paper & Graphite tokens reused without changes. Demo still-life images are user-provided original illustrations, not real model output.
- Typecheck passed. Browser opened and captured new task, General and Providers at 1440×960, light theme, UI Preview. Initial screenshot showed excessive separation of welcome/composer; corrected before F1.
- Screenshots: `web/ui-preview-results/f0-*.png` (local baseline, not reference screenshots).
- Preview build initially found missing CSS module declaration in isolated tsconfig; added Vite client declaration and rerunning.

## F1 — workspace interaction

- Single 216px project tree, 52px header, centered 720px Composer, optional 54/46 image workspace. Mobile uses conversation/data switching rather than three columns.
- Preview Plan approval binds task revision; explicit scope/model/destination/unknown-cost confirmation. Queue remains a truthful simulated pending list, not automatic dispatch.
- Stop passes through stopping before interrupted; resume only from an eligible state. Unknown does not silently retry. Model switching leaves in-flight model frozen. Same message retry preserves command identity.
- Native dialog focus containment and Escape/return focus; searchable model popover with keyboard navigation; IME and Shift+Enter retained.
- Images use original illustrative pixels and thin editable boxes. Draft geometry is stored per task and image in the Fixture Adapter; save failure preserves it. Original/compare/list/resize/undo controls available.
- Settings navigation and sidebar transitions consult an unsaved-edit guard. Local file previews stay in memory and are scoped to the selected task.
- 7 new unit tests passed; targeted layout and pane/model browser tests passed. Full interaction browser rerun follows an IME-test correction: synthetic composition does not emulate native OS editing, so its no-send assertion is separate from Shift+Enter newline assertion.
- Preview build passed; preview assets moved to its own public directory so a built preview retains images. No production asset or entry replacement.

## F2 — six settings pages

- General: immediate theme preview/cancel, language navigation preference, font/density, initial project-collapse preference, shortcut guidance.
- Providers: add/edit validation, simulated credential slot only, no secret input, explicit probe success/failure/unknown and deletion-impact confirmation.
- Agent models: shared registry snapshot with Composer, tool incompatibility and removed-account availability. Default affects new tasks only.
- Vision/plugins: ready/missing/disabled/error rows, compatibility disclosure, simulated install approval, progress, failed verification and retry.
- Privacy: scoped simulated external-data preference, protected cache cleanup preview; no real files touched. Usage: decimal budget, scope, unknown costs, near/exceeded states.
- All six pages have explicit loading/empty/read-error/recovery scenarios. Save failure preserves editing; unsaved exit guard also covers project sidebar. Settings save and cancel are consistent controls, not old production form wrappers.
- Fixed a browser-discovered immutable snapshot defect: probe/install mutated the editor's baseline, incorrectly making remote updates look like local edits. Fixture now copies before writes; both failure and unknown results render.
- Validation: **15/15 preview browser tests passed**, including six state matrices, settings persistence, dirty guard, install verification, privacy protection, network isolation and core workspace controls. Earlier 2 failing assertions exposed the snapshot defect and now pass.
- English currently translates navigation and new-task entry; detailed demo narratives/settings descriptions remain Chinese. This is a known UI localization limitation, not a claim of complete English localization.

F3 follows with responsive/artifact coverage, actual screenshots and handoff. No HTTP integration starts before visual approval.

## F3 — visual and interaction verification

- Actual React screenshot audit found expanded historic Plan displaced the current human question and resume control. Plan now collapses after approval; current controls stay prominent. Provider editing has one set of save/cancel controls.
- Seventeen application captures cover all requested scenes and Settings, plus mobile canvas and stopping. Every capture is UI Preview/Fixture, never a prototype passed off as an application screenshot. Final source-SHA captures will be regenerated after the UI source commit.
- 21 preview E2E tests passed (including screenshot capture, 1440×900 / 1280×720 / 1024×768 / 390×844, long queue, geometry draft recovery and failed human save). Full Web typecheck, **252 unit tests**, production build and separate preview build passed. Production has the pre-existing >500kB bundle warning. Source-polish final rerun pending below.
- Production bundle scan found no preview storage namespace or simulated-install UI. Built preview includes its own images/brand assets; no real Provider requests in browser network audit.
- Geometry pointer conversion uses SVG screen transform, not viewport dimensions, so aspect-ratio letterboxing does not skew drag coordinates. Numeric edits are clamped to original image dimensions. Candidate reference can be attached to the next Composer command.
- Not executed: real Provider/HTTP integration, real model inference/accuracy, Rust regression (no Rust edits), native OS IME input, native browser 200% zoom, screen-reader session, human usability study. Synthetic composition/keyboard and viewport layout checks are not substitutes for those.

### Final verification

- Source `452ea02`: `npm run typecheck` passed; `npm test` **252/252**; `npm run test:ui-preview` **21/21**; `npm run build` and `npm run build:ui-preview` passed. `git diff --check` clean.
- Reviewed actual light/dark workspace, model picker, human question/continue visibility, six Settings pages and mobile screenshots after layout corrections. Screenshot manifest records exact source SHA/URL/viewport/theme. Gallery: `/evidence/compare.html`, actual images under `web/ui-preview/evidence/`, references clearly labeled separately.
- Preview is serving current source at **127.0.0.1:5174**, not user 8787 or an embedded stale production dist. Separate preview build also includes illustration assets. No production simulator namespace found in production dist scan.
- Handoff: `docs/handoff/UI_FIRST_FRONTEND.md`, including visible controls, simulated/real boundary, backend connection requests, limitations and phase commits.
- Branch `codex/agent-ui-frontend`; only Agent A-owned paths changed. No push, remote mutation, Rust/backend edits, original workspace cleanup or historical version mutation. Remaining work is native/human validation and post-approval integration, not additional Rust work in this branch.
