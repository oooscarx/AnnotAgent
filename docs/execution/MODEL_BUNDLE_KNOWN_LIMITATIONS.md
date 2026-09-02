# Model Bundle Provisioning Known Limitations

Last updated: 2026-09-02 CST

- Before M1–M6, SAM setup still uses two raw ONNX upload controls and overloads `NeedsWeights` for
  the model-asset gap.
- Plugin Registry operational state is a durable JSON document; SQLite migration 13 currently
  provides plugin audit/reference tables but is not the runtime source of truth.
- Existing plugin manifests use weight component metadata rather than a Bundle compatibility
  requirement. M4 migrates this without removing historical metadata.
- Existing Published Workflow plugin snapshots freeze plugin/checkpoint identity but not a distinct
  Bundle/Model Instance identity. M5 adds those fields with backward-compatible deserialization.
- No Model Catalog, `.annotmodel` import, safe downloader or content-addressed model store exists at
  M0. These exist after M3, but the repository-owned Fixture entry is added in M7.
- Catalog signature metadata is structurally validated in M3; pinned Catalog signing-key
  verification remains part of official publishing infrastructure. Bundle Ed25519 verification is
  already cryptographic.
- Provisioning operations provide real stage/progress events internally; the persistent operation
  feed and install wizard consume them in M6.
- No real SAM/EfficientSAM asset is stored in Git. Fixture evidence will test system behavior, not
  segmentation accuracy.
- Model provisioning guarantees repeatable execution identity, not geometry correctness; calibration
  or Human Review remains mandatory under existing Geometry Safety rules.
