# Mainline Frontend 1

## Baseline

- Common baseline: `d3220cb54bd50589efed86b6c0753bf4ea8db0db`.
- Integration worktree: `/Users/oscar/Documents/my_workspace/AnnotAgent-mainline-integration`.
- Branch: `codex/mainline-integration`.
- The source worktree contained only user-owned untracked design/product directories; none were copied, removed or committed here.

## F1-0 in progress

Frontend 1 owns App/http/adapter/public route and type composition. Frontend 2 owns review/image/package modules. Frontend 3 owns model/plugin/settings modules. Backend owns Rust, migrations and `docs/contracts/mainline-v1`.

The first seam defines a server-backed `MainlineTaskView`, typed real message projections, complete frozen `VisualSelection`, capability setup return object and domain composition slots. It introduces no guessed HTTP route and no browser state machine. A task is complete only when the server-backed training package is Ready and downloadable.

Communication:

- `ML-001` queued to Frontend 2 UUID `01a094ad-5715-75d0-88cf-33b5eb7fa3e8`.
- `ML-002` queued to Backend UUID `01a0855e-9c39-7c33-9f18-93e084d14816`.
- `ML-003` resolved exact Frontend 3 UUID `01a094ad-337c-7bc0-99ea-b3d18acb7d2f` and was queued successfully.

No Provider call, true workspace write, push or main merge.
