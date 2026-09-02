# Model Bundle Provisioning Acceptance Evidence

Last updated: 2026-09-02 CST

## M0 evidence

- [x] Baseline git status and recent history recorded before implementation.
- [x] `cargo test --workspace --all-features` passed.
- [x] `cargo build --workspace --all-features` passed.
- [x] Existing `.annotplugin`, Host, Registry, Model Profile, workflow snapshot and SAM paths audited.
- [x] SAM install regression proves the plugin is `NeedsWeights` with exactly the
  `image_encoder` and `mask_decoder` components missing.
- [x] The projected model remains `MissingWeights` and non-selectable.
- [x] Current GUI route and raw component upload experience recorded.
- [x] Current JSON registry and SQLite migration/profile boundaries recorded.

## Release matrix

- [ ] Plugin Package and Model Bundle are independent entities.
- [ ] Model file roles are generic manifest strings.
- [ ] `.annotmodel` pack, inspect and verify are deterministic and safe.
- [ ] Bundle contains source, license, contracts, checksums and test vectors.
- [ ] Local fixture and HTTPS curated catalogs work without executable content.
- [ ] Safe download and atomic content-addressed installation pass failure/cancel tests.
- [ ] Plugin required roles, versions, capabilities, contracts, platform and provider are resolved.
- [ ] Only a smoke-tested Ready Model Instance produces an available Model Profile.
- [ ] Published Workflows freeze exact plugin, bundle, file, instance and profile identity.
- [ ] Referenced bundles are protected and unreferenced content can be garbage-collected.
- [ ] Primary GUI path installs a compatible Bundle rather than raw ONNX components.
- [ ] Pipeline Builder discovers readiness/setup requirements but cannot mutate model assets.
- [ ] Fixture end-to-end lifecycle passes offline and is visibly non-publishable.
- [ ] One real prompted-segmentation bundle passes legal, contract and Rust inference evidence, or is
      recorded as an explicit external blocker without a false supported claim.
- [ ] Geometry Safety, workflow validation, lineage, replay, batch, review, export and Providers do
      not regress.
- [ ] Active installation/inference paths contain no Python, pip, uv, conda or venv process launch.
