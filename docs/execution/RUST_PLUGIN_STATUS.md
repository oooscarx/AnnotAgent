# Rust Expert Model Plugin Alpha — Status

- Current milestone: M7 — advanced detector feasibility
- Completed: M0 baseline; M1 Plugin API; M2 Rust SDK/Dummy; M3 deterministic package, isolated Host, durable Registry, database migration, exact Workflow identity and CLI lifecycle; M4 model-neutral image/geometry tools and native ONNX Runtime; M5 YOLOX Nano package, real process inference and Detection Skill/Core Filter Workflow; M6 SAM/PIDNet Rust processes, multi-component weights, embedding cache, MaskSet/SemanticMask and geometry integration
- In progress: M7 RF-DETR and LocateAnything Rust feasibility and truthful live-conditional states
- Next: complete M7 advanced detectors, then M8 product lifecycle surfaces
- Latest Rust tests: cargo test --workspace --all-features — PASS, 362 passed, 4 explicit external/billable tests ignored in the ordinary run
- Latest plugin conformance: installed-process Dummy authentication/discovery/typed inference/crash suite PASS
- Latest real-model test: official YOLOX Nano ONNX checkpoint c789161e… on an upstream sample image PASS in 1.41 s through isolated process, conformance, Detection Skill and Core Filter
- Latest Web tests: pending M8 baseline
- Latest E2E: isolated Dummy process handshake/infer/crash PASS
- Latest local milestone commit: f4e202b (M5); M6 commit is created after this evidence update
- Release-blocking remaining: M7–M9 acceptance work
- Live-conditional: accelerator providers plus real SAM and PIDNet checkpoints/process smokes
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`. No remote is modified and no push
is permitted by this task.
