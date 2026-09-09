# UI First Frontend — Agent A

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

## Remaining / next

F1: interaction correctness, dialogs/focus, scoped drafts, refresh/navigation guards, image edit controls and fixture semantics. F2: six-page error/save flows, model/provider consistency. F3: browser matrix, screenshots and handoff. No Live tests, no human usability tests, no native browser 200% zoom verification yet.
