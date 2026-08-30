# Lean Agent Alpha Blockers

## Offline release blockers

None at M0. The repository builds and all 221 Rust tests pass.

## Live-conditional

- Qwen requires an operator-provided workspace credential and successful provider request.
- SAM, LocateAnything, RF-DETR and YOLO require separately installed code/weights and healthy local
  Workers. Ports 8790, 8791 and 8792 were unavailable during the M0 audit.
- No conversation credential or model weight will be read, restored, copied or committed.
- Native browser zoom and hardware-specific model behavior remain manual checks.

