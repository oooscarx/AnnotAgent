# Detection Backends Blockers

Updated: 2026-08-30

## In-repository blockers

None at M2. Core, Runtime, Provider, Storage, Server, Web, TUI, Mock, protocol, and documentation
work can proceed locally. Candidate Match/Evidence Gate and model workers are scheduled work, not
environment blockers.

## Live-conditional external requirements

### LocateAnything-3B

- Current host is Darwin arm64 and has no `nvidia-smi`/NVIDIA CUDA runtime.
- No LocateAnything model path is configured, and automatic download is prohibited.
- The official released model license restricts use to non-commercial research/evaluation. A live
  run must use a compatible environment and retain this restriction in its Model Descriptor.

Result: Worker contract and Mock behavior are Release Blocking; real five-image GPU smoke is
`LIVE-CONDITIONAL` until a legal, explicitly configured NVIDIA environment is available.

### RF-DETR

- No RF-DETR checkpoint path, checkpoint SHA-256, or training dataset version is configured.
- The intended RoboCup specialist model is a fine-tuned external artifact, not a tracked file.
- Official licensing differs by model variant, so the concrete registered checkpoint must identify
  its package/weight terms before a real result is reported.

Result: Worker contract and Mock behavior are Release Blocking; real five-image smoke is
`LIVE-CONDITIONAL` until an explicitly configured checkpoint and metadata are available.

## Manual browser requirement

Native browser 200% zoom requires a browser environment where browser chrome zoom can be changed
and observed. Automated 1024px and compact reflow tests remain required but are separate evidence.
