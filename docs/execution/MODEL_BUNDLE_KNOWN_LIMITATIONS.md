# Model Bundle Provisioning Known Limitations

Last updated: 2026-09-03 CST

- Plugin Registry operational state is a durable JSON document; SQLite migration 13 currently
  provides plugin audit/reference tables but is not the runtime source of truth.
- Existing weight-component metadata remains readable only for compatibility and explicit local
  Bundle migration. The normal setup path has no raw ONNX uploader.
- Historical Published Workflow snapshots without a Model Asset identity remain readable but are
  never rewritten. A new exact Bundle/Instance identity requires a cloned Draft and new Version.
- Catalog signature metadata is structurally validated in M3; pinned Catalog signing-key
  verification remains part of official publishing infrastructure. Bundle Ed25519 verification is
  already cryptographic.
- External HTTPS transfer is implemented and safely bounded, but no official AnnotAgent Catalog
  hosting/signing service is part of this local Alpha.
- No real SAM/EfficientSAM asset is stored in Git or advertised in the Catalog. The built-in Fixture
  proves protocol/runtime behavior only and is non-publishable.
- TUI provisioning is read-only. Mutating license/install/remove actions remain GUI/CLI human
  operations by design.
- Model provisioning guarantees repeatable execution identity, not geometry correctness; calibration
  or Human Review remains mandatory under existing Geometry Safety rules.
