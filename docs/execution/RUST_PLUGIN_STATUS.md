# Rust Expert Model Plugin Alpha — Status

- Current milestone: M8 — product lifecycle surfaces and Agent discovery
- Completed: M0 baseline; M1 Plugin API; M2 Rust SDK/Dummy; M3 deterministic package, isolated Host, durable Registry, database migration, exact Workflow identity and CLI lifecycle; M4 model-neutral image/geometry tools and native ONNX Runtime; M5 YOLOX Nano package, real process inference and Detection Skill/Core Filter Workflow; M6 SAM/PIDNet Rust processes, multi-component weights, embedding cache, MaskSet/SemanticMask and geometry integration; M7 RF-DETR official ONNX path plus truthful LocateAnything unsupported contract
- In progress: M8 Settings/CLI/TUI/Agent lifecycle and discovery integration
- Next: complete M8 product surfaces, then M9 migration and release validation
- Latest Rust tests: cargo test --workspace --all-features — PASS, 370 passed, 5 explicit external/billable tests ignored in the ordinary run
- Latest plugin conformance: installed-process Dummy authentication/discovery/typed inference/crash suite PASS
- Latest real-model test: official YOLOX Nano ONNX checkpoint c789161e… on an upstream sample image PASS in 1.41 s through isolated process, conformance, Detection Skill and Core Filter
- Latest Web tests: pending M8 baseline
- Latest E2E: isolated Dummy process handshake/infer/crash PASS
- Latest local milestone commit: 9e8cee6 (M6); M7 commit is created after this evidence update
- Release-blocking remaining: M8–M9 acceptance work
- Live-conditional: accelerator providers plus real SAM, PIDNet and RF-DETR checkpoints/process smokes
- Unsupported: LocateAnything until a verified complete Rust-callable model runtime is available
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`. No remote is modified and no push
is permitted by this task.
