# Rust Expert Model Plugin Alpha — Status

- Current milestone: M5 — official YOLO ONNX plugin
- Completed: M0 baseline; M1 Plugin API; M2 Rust SDK/Dummy; M3 deterministic package, isolated Host, durable Registry, database migration, exact Workflow identity and CLI lifecycle; M4 model-neutral image/geometry tools and native ONNX Runtime
- In progress: official YOLO package, preprocessing/postprocessing, typed DetectionSet and Workflow E2E
- Next: M5 YOLO Rust plugin and Workflow E2E
- Latest Rust tests: cargo test --workspace --all-features — PASS, 346 passed, 1 explicitly billable test ignored
- Latest plugin conformance: installed-process Dummy authentication/discovery/typed inference/crash suite PASS
- Latest native runtime test: real ONNX Identity graph on CPU PASS; this is a runtime fixture, not an expert-model accuracy claim
- Latest Web tests: pending M8 baseline
- Latest E2E: isolated Dummy process handshake/infer/crash PASS
- Latest local milestone commit: 6cafaf2 (M3); M4 commit is created after this evidence update
- Release-blocking remaining: M5–M9 acceptance work
- Live-conditional: accelerator providers and production weights for complex expert models
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`, with a clean worktree. No remote is
modified and no push is permitted by this task.
