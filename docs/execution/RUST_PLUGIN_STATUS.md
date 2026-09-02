# Rust Expert Model Plugin Alpha — Status

- Current milestone: M6 — official SAM and PIDNet plugins
- Completed: M0 baseline; M1 Plugin API; M2 Rust SDK/Dummy; M3 deterministic package, isolated Host, durable Registry, database migration, exact Workflow identity and CLI lifecycle; M4 model-neutral image/geometry tools and native ONNX Runtime; M5 YOLOX Nano package, real process inference and Detection Skill/Core Filter Workflow
- In progress: prompted-segmentation and semantic-segmentation contracts, Rust ONNX paths and geometry-safety integration
- Next: complete M6 SAM/PIDNet and then M7 advanced detector feasibility
- Latest Rust tests: cargo test --workspace --all-features — PASS, 351 passed, 2 explicit external tests ignored in the ordinary run
- Latest plugin conformance: installed-process Dummy authentication/discovery/typed inference/crash suite PASS
- Latest real-model test: official YOLOX Nano ONNX checkpoint c789161e… on an upstream sample image PASS in 1.41 s through isolated process, conformance, Detection Skill and Core Filter
- Latest Web tests: pending M8 baseline
- Latest E2E: isolated Dummy process handshake/infer/crash PASS
- Latest local milestone commit: 640e1e5 (M4); M5 commit is created after this evidence update
- Release-blocking remaining: M6–M9 acceptance work
- Live-conditional: accelerator providers and production weights for complex expert models
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`, with a clean worktree. No remote is
modified and no push is permitted by this task.
