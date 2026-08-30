# Detection Backends Blockers

Updated: 2026-08-30

## In-repository blockers

None at M8. Capability Skills and Worker adapters, Candidate Match, Evidence Gate,
capability-driven Advisor, bounded Recovery Agent, persisted structured Trace, capability-bound
RoboCup Ball hybrid policy and exact published Runtime execution are complete. Guided
mixed-evidence UX and cache/replay release proof remain scheduled repository work, not environment
blockers.

## Live-conditional external requirements

### LocateAnything-3B

- Current host is Darwin arm64 and has no `nvidia-smi`/NVIDIA CUDA runtime.
- No LocateAnything model/code path is configured, and automatic download is prohibited. The
  tracked Worker was started without them and correctly reported `unavailable` while continuing to
  serve capability discovery.
- The official released model license restricts use to non-commercial research/evaluation. A live
  run must use a compatible environment and retain this restriction in its Model Descriptor.

Result: Worker contract, Mock behavior, startup isolation, Settings and capability discovery pass;
real five-image GPU smoke is `LIVE-CONDITIONAL` until a legal, explicitly configured NVIDIA
environment is available.

### RF-DETR

- Current host is Darwin arm64 and has no `nvidia-smi`/NVIDIA CUDA runtime.
- No RF-DETR checkpoint path, checkpoint SHA-256, architecture/model version, training dataset
  version, exact label space, or weight-license metadata is configured. Automatic download is
  prohibited. The tracked Worker was started without them and correctly reported `unavailable`
  while continuing to serve capability discovery.
- The intended RoboCup specialist model is a fine-tuned external artifact, not a tracked file.
- Official licensing differs by model variant, so the concrete registered checkpoint must identify
  its package/weight terms before a real result is reported.

Result: Worker contract, Mock/runtime behavior, startup isolation, Settings and capability
discovery pass; real five-image GPU smoke is `LIVE-CONDITIONAL` until an explicitly configured
checkpoint, metadata and compatible CUDA environment are available.

## Manual browser requirement

Native browser 200% zoom requires a browser environment where browser chrome zoom can be changed
and observed. Automated 1024px and compact reflow tests remain required but are separate evidence.
