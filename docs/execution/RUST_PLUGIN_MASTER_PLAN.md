# Rust Expert Model Plugin Alpha — Master Plan

The authoritative requirements are in [RUST_PLUGIN_MASTER_PROMPT.md](RUST_PLUGIN_MASTER_PROMPT.md).
Execution proceeds through M0–M9 with one local commit per milestone and no push.

| Milestone | Deliverable | Release evidence |
| --- | --- | --- |
| M0 | Baseline, migration inventory, Rust-only CI | full baseline and boundary scan |
| M1 | Stable Plugin API, manifest, package identity | serialization and validation tests |
| M2 | Rust SDK, authenticated process protocol, dummy detector | protocol conformance tests |
| M3 | Host, package/registry lifecycle, persistence and CLI | install/start/infer/crash/version tests |
| M4 | Common image/tensor helpers and Rust ONNX runtime boundary | tiny deterministic ONNX fixture |
| M5 | YOLO ONNX plugin | typed detection and workflow smoke |
| M6 | SAM and PIDNet plugins | prompted/semantic segmentation contracts |
| M7 | RF-DETR and LocateAnything | truthful supported/live-conditional states |
| M8 | GUI, TUI, API and Builder discovery | Web and API lifecycle tests |
| M9 | active migration, full release verification and docs | acceptance matrix and clean Git state |

Model weights, accelerator-specific smokes, and unsupported exports are live-conditional. They may
remain outside the release blocker only when the package, contract, status, and limitation are
truthfully represented and no alternate runtime is used.
