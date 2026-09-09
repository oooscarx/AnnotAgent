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

## Remaining / next

F1: interaction correctness, dialogs/focus, scoped drafts, refresh/navigation guards, image edit controls and fixture semantics. F2: six-page error/save flows, model/provider consistency. F3: browser matrix, screenshots and handoff. No Live tests, no human usability tests, no native browser 200% zoom verification yet.
