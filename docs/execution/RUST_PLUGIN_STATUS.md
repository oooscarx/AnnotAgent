# Rust Expert Model Plugin Alpha — Status

- Current milestone: M9 — migration and release validation
- Completed: M0 baseline; M1 Plugin API; M2 Rust SDK/Dummy; M3 deterministic package, isolated Host, durable Registry, database migration, exact Workflow identity and CLI lifecycle; M4 model-neutral image/geometry tools and native ONNX Runtime; M5 YOLOX Nano package, real process inference and Detection Skill/Core Filter Workflow; M6 SAM/PIDNet Rust processes, multi-component weights, embedding cache, MaskSet/SemanticMask and geometry integration; M7 RF-DETR official ONNX path plus truthful LocateAnything unsupported contract; M8 GUI/Server/TUI lifecycle, Agent discovery and exact publication/runtime integration
- In progress: M9 Rust-only active-path migration, release matrix and final validation
- Next: remove legacy scripting-worker release paths and complete the full acceptance suite
- Latest Rust tests: cargo test --workspace --all-features — PASS, 384 passed, 5 explicit external/billable tests ignored in the ordinary run
- Latest plugin conformance: installed-process Dummy authentication/discovery/typed inference/crash suite PASS
- Latest real-model test: official YOLOX Nano ONNX checkpoint c789161e… on an upstream sample image PASS in 1.41 s through isolated process, conformance, Detection Skill and Core Filter
- Latest Web tests: 44 Vitest tests PASS; production TypeScript/Vite build PASS
- Latest E2E: Settings Plugin lifecycle surface PASS at desktop and 390 px width; isolated Dummy process handshake/infer/crash PASS
- Latest local milestone commit: 4b8d145 (M7); M8 commit follows this evidence update
- Release-blocking remaining: M9 migration and full acceptance work
- Live-conditional: accelerator providers plus real SAM, PIDNet and RF-DETR checkpoints/process smokes
- Unsupported: LocateAnything until a verified complete Rust-callable model runtime is available
- Real blocker: none for architecture work

The branch started as `main`, 17 commits ahead of `origin/main`. No remote is modified and no push
is permitted by this task.
