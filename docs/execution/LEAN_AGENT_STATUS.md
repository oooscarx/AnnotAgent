# Lean Agent Alpha Status

Updated: 2026-08-31

- Current Milestone: M1 — Capability Skill and Model Backend convergence.
- Completed: master prompt saved; Git/remote baseline recorded; repository surface inspected; full
  Rust baseline executed.
- In progress: generic Capability IDs, compatibility aliases and backend availability grouping.
- Next: migrate Registry/UI authoring without rewriting stored Published Versions.
- Recent Rust tests: `cargo test --workspace --all-features` — 221 passed, 0 failed.
- Recent Web tests: inherited baseline only; not rerun in M0 yet.
- Recent E2E: inherited baseline only; not rerun in M0 yet.
- Recent commit: `ca54f07 test(release): validate open-vocabulary and specialist detection alpha`.
- Release Blocking remaining: all Lean Agent Alpha A–G items until evidenced in
  `LEAN_AGENT_ACCEPTANCE.md`.
- Live-conditional: real Qwen request; SAM, LocateAnything, RF-DETR and YOLO inference with explicit
  external weights; manual native browser checks.
- Real blocker: none for offline implementation. External credentials/weights are not required for
  ScriptedMock, RuleBased, protocol, UI or test work.
