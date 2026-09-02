# Real Prompted-Segmentation Delivery Alpha Blockers

Last updated: 2026-09-03 CST

## Current product blocker

The normal-user one-click model installation path is complete on the current macOS ARM64 host.
Pipeline Builder selection, real Published Workflow execution, Debug/Review/Replay lineage and
version-reference protection remain M5 implementation and verification work, not external blockers.

## Candidate risks under investigation

- EfficientSAM-Ti is proven on macOS ARM64 CPU. Linux x86_64 is a declared build target but cannot
  receive real execution evidence from this host; M6 must label that distinction truthfully.
- Hugging Face revision URLs currently redirect to content storage. A safe bounded redirect policy
  or a separately hosted release Bundle is required for remote one-click delivery.
- The repository has no project-owned remote release endpoint or signing key. M2 uses an explicitly
  trusted local development Catalog; M6 must document the exact remote release asset list without
  claiming it has been uploaded.
- Only the current macOS ARM64 CPU host can provide real execution evidence in this environment.
  Linux support may be compiled and packaged but cannot be reported as a real run without a host.

## Not blockers

- The lack of a SAM 2 ONNX export does not block the capability because other candidates must be
  audited.
- The absence of Python on the user machine is a requirement, not a blocker.
- The dedicated EfficientSAM Plugin, Recipe, Bundle and real smoke are complete.
