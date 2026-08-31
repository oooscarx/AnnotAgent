# Expert Vision Blockers

Updated: 2026-09-01

## Active release blockers

The offline release is blocked on M1–M8 implementation and verification. There is no external
dependency preventing that work.

## Live-conditional constraints

- No SAM/RF-DETR/LocateAnything/YOLO/PIDNet/Grounding DINO checkpoint or GPU environment is
  assumed or downloaded.
- Worker adapters and manifests may therefore be fully implemented while real inference health and
  accuracy remain conditional on explicit user configuration.
- External Provider tests remain opt-in and billable; normal CI must not read user credentials.

## Not blockers

- An unavailable Worker must not prevent AnnotAgent startup.
- Missing weights must remain visible and non-selectable, not be hidden or treated as a setup
  success.
- Mock/conformance evidence is sufficient for offline protocol and pipeline acceptance but not for
  live accuracy claims.
