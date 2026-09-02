# Rust Expert Model Plugin Alpha — Status

- Current milestone: M3 — Host, Registry and lifecycle
- Completed: M0 baseline; M1 Plugin API; M2 authenticated SDK server, standalone Dummy Rust process and shared conformance
- In progress: deterministic package verification, isolated process Host, installations, versions, weights, references, persistence and CLI
- Next: M4 common model helpers and Rust ONNX runtime
- Latest Rust tests: `cargo test --workspace --all-features` — PASS, 339 passed, 1 explicitly billable test ignored
- Latest plugin conformance: Dummy process authentication/discovery/typed inference suite PASS
- Latest real-model test: none in this task; existing external model claims are not inherited
- Latest Web tests: pending M8 baseline
- Latest E2E: pending M2/M3
- Latest local commit: `cb17d7b` before this task
- Release-blocking remaining: M0–M9 acceptance work
- Live-conditional: accelerator providers and production weights for complex expert models
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`, with a clean worktree. No remote is
modified and no push is permitted by this task.
