# Model Bundle Provisioning Acceptance Evidence

Last updated: 2026-09-03 CST

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

- [x] Plugin Package and Model Bundle are independent entities at the package API boundary.
- [x] Model file roles are generic validated manifest strings.
- [x] `.annotmodel` pack, inspect and verify are deterministic and safe.
- [x] Bundle contains source, license, contracts, checksums and test vectors.
- [x] Local persisted and HTTPS curated catalogs work without executable content; the actual
      built-in Fixture entry is added in M7.
- [x] Safe download and atomic content-addressed installation are implemented; external HTTPS live
      transfer remains live-conditional while URL, cancellation and local install paths are tested.
- [x] Plugin required roles, versions, capabilities, contracts, platform and provider are resolved.
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

## M1 evidence

- [x] `.annotmodel` extension and versioned schema constants exist in a dedicated model-asset crate.
- [x] Manifest covers identity, version, model format/variant, capabilities, compatible plugins,
      multi-file roles, file hashes/sizes, contracts, source, export, runtime, license and smoke suite.
- [x] TOML rejects unknown fields, unsafe paths, invalid version constraints, duplicate roles,
      missing roles and contradictory Fixture/publishable claims.
- [x] ONNX bundles require an opset; publishable bundles cannot prohibit redistribution.
- [x] Generic roles prove that a new `depth_auxiliary_2` role does not require a Core enum change.
- [x] Independent Bundle and Model Instance states no longer depend on the legacy `NeedsWeights`
      vocabulary.

## M2 evidence

- [x] Deterministic ZIP output has stable order, zeroed timestamps, owner-readable permissions and
      stable whole-package SHA-256.
- [x] Pack rejects unknown files, missing referenced files, links, empty/oversized files and a
      mismatch between Manifest size/hash and actual bytes.
- [x] Verify rejects traversal/absolute/mixed paths, duplicate or case-conflicting names, links,
      excessive file count/size, missing Manifest/checksums and non-exact file lists.
- [x] Every payload is stream-hashed; model assets additionally match Manifest size/hash, while
      contract, transform and model-license files match their declared hashes.
- [x] Optional Ed25519 verification uses a canonical versioned payload; required-but-missing and
      wrong-key signatures fail.
- [x] Extraction repeats hash/size checks and removes a partial destination after failure.
- [x] CLI provides Bundle pack/inspect/verify only; no conversion, script execution or installation
      side effect exists in M2.

## M3 evidence

- [x] Catalog entry fixes Bundle ID/version/digest/size/URL, capabilities, compatible Plugins,
      platform resources, license summary and Publisher identity.
- [x] HTTPS validation rejects credentials, HTTP/file schemes, localhost, `.local`, literal private,
      loopback, link-local and unspecified IPs; DNS results are rechecked before a request.
- [x] Reqwest follows no redirects and writes a bounded stream to a unique partial path while
      calculating SHA-256. Cancellation/error/hash/size failure removes the partial file.
- [x] Exact license digest, Bundle ID/version/digest and Catalog metadata must agree before install.
- [x] Installation extracts into unique staging, writes `verification.json`, atomically renames to a
      digest-derived directory and persists Registry state only after activation.
- [x] Duplicate content is idempotent; conflicting bytes cannot replace an immutable ID/version.
- [x] API omits absolute content paths and exposes separate installed versus available resources.
- [x] Migration 14 creates the complete relational audit schema in one transaction.

## M4 evidence

- [x] Every official weighted Plugin model declares generic required file roles; SAM's two roles are
      manifest data rather than Core variants.
- [x] `PluginRuntimeStatus` maps an installed executable separately from legacy weight readiness.
- [x] Resolver outcomes distinguish missing/unavailable Plugin, version, file role, Contract,
      format/opset, platform, execution provider and exact license acceptance failures.
- [x] Compatibility validates the Bundle requirement against the installed Plugin model and hashes
      the complete Plugin model contract rather than trusting filenames.
- [x] Versioned JSON Model Contracts support aliases, dtype, static/dynamic dimensions and
      cross-role tensor connections.
- [x] A generated but real ONNX Identity graph is opened by the Rust ONNX Runtime; its actual input
      and output metadata passes the correct Contract and rejects an incorrect tensor name.
- [x] Binding persists deterministic Plugin, Bundle, role digest, execution-provider, Contract,
      Model Instance and Model Profile identities.
- [x] Contract-valid instances stop at `Preparing`; the derived Model Profile is unavailable and
      non-selectable until the M5 smoke-test gate marks the instance Ready.
- [x] Model Instance and compatible-Bundle APIs expose exact identities and structured evidence but
      omit local content paths.
- [x] Focused strict Clippy passes for Bundle, Catalog, Plugin API/Registry and Server.
