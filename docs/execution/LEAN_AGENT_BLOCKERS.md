# Lean Agent Alpha Blockers

## Offline release blockers

None through M7. The real Rust validation/Dry Run revision loop, Guided Diff/apply/undo path and lean
RoboCup Agent demo pass with offline Model Backends; external models are not required for M8 release
validation.

## Live-conditional

- Qwen requires an operator-provided workspace credential and successful provider request.
- SAM, LocateAnything, RF-DETR and YOLO require separately installed code/weights and healthy local
  Workers. Ports 8790, 8791 and 8792 were unavailable during the M0 audit.
- No conversation credential or model weight will be read, restored, copied or committed.
- Native browser zoom and hardware-specific model behavior remain manual checks.
