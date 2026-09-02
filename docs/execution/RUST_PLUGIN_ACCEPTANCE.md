# Rust Expert Model Plugin Alpha — Acceptance Evidence

Only commands actually executed are recorded here.

## M0 baseline — 2026-09-02

- `git status --short --branch`: clean `main`, 17 commits ahead of `origin/main`.
- `git log --oneline -20`: latest pre-task commit `cb17d7b`.
- `cargo test --workspace --all-features`: PASS — 339 passed, 1 billable provider smoke ignored.
- Active boundary inventory found no Rust Plugin API, SDK, Host, Registry, package lifecycle or
  official Rust plugin process before this task.
- Existing protocol: versioned loopback HTTP Vision client supports health, capability, infer and
  cancel; the SAM reference additionally exposes model and contract discovery.
- Existing model selection: `ModelProfile`, capability matching and immutable
  `ModelProfileSnapshot`; no plugin package/checkpoint identity is frozen yet.
- Existing geometry safety and typed Artifact lineage tests pass.

Milestone evidence will be appended after the corresponding tests pass.
