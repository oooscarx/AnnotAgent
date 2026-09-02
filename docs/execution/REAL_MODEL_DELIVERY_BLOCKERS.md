# Real Prompted-Segmentation Delivery Alpha Blockers

Last updated: 2026-09-03 CST

## Current product blocker

The only configured Catalog entry is a non-publishable Fixture. A normal user cannot install a real
prompted-segmentation model. This is an implementation gap, not an accepted external blocker.

## Candidate risks under investigation

- EfficientSAM-Ti assets are revision-pinned and hashable, but their graph must be inspected and a
  dedicated Rust Plugin must pass real inference before acceptance.
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
- The need to implement a dedicated EfficientSAM Plugin or Recipe is planned work, not a blocker.

