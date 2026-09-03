# Real Prompted-Segmentation Delivery Alpha Blockers

Last updated: 2026-09-03 CST

## Current product blocker

None for the local macOS ARM64 Developer Preview. One-click setup, Ready state, Pipeline Builder,
real Published Workflow execution, Debug/Review/Replay lineage, restart recovery and reference
protection are all evidenced.

## Remaining release limitations

- EfficientSAM-Ti is proven on macOS ARM64 CPU. Linux x86_64 is a declared build target but did not
  receive real execution evidence from this host and is labelled build-compatible only.
- Hugging Face revision URLs currently redirect to content storage. A safe bounded redirect policy
  or a separately hosted release Bundle is required for remote one-click delivery.
- The repository has no configured release signing key. M2 uses an explicitly trusted local
  development Catalog; M6 documents the exact remote release asset list without claiming upload or
  publisher verification.
- Only the current macOS ARM64 CPU host can provide real execution evidence in this environment.
  Linux support cannot be reported as a real run without a Linux host.

## Not blockers

- The lack of a SAM 2 ONNX export does not block the capability because other candidates must be
  audited.
- The absence of Python on the user machine is a requirement, not a blocker.
- The dedicated EfficientSAM Plugin, Recipe, Bundle and real smoke are complete.
- GitHub publication is an operator action prohibited by this task's no-push/no-remote-mutation
  boundary; local installation does not depend on it.
