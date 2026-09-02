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
- [x] Only a smoke-tested Ready Model Instance produces an available Model Profile.
- [x] Published Workflows freeze exact plugin, bundle, file, instance and profile identity.
- [x] Referenced bundles are protected and unreferenced content can be garbage-collected.
- [x] Primary GUI path installs a compatible Bundle rather than raw ONNX components.
- [x] Pipeline Builder discovers readiness/setup requirements but cannot mutate model assets; Retry
      reuses the persisted Draft after a separately authorized setup action.
- [x] Fixture end-to-end lifecycle passes offline and is visibly non-publishable.
- [x] One real prompted-segmentation bundle passes legal, contract and Rust inference evidence, or is
      recorded as an explicit external blocker without a false supported claim.
- [x] Geometry Safety, workflow validation, lineage, replay, batch, review, export and Providers do
      not regress; the full Rust/Web/E2E acceptance suite passes.
- [x] Active installation/inference paths contain no Python, pip, uv, conda or venv process launch;
      both enforced boundary scans pass.

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

## M5 evidence

- [x] Fixed smoke suites load a bounded image and one data-only request template from verified
      Bundle content; package data cannot choose local paths or host Run identities.
- [x] Smoke evaluation requires a matching request, no typed error, Core-valid finite Artifacts,
      required kinds/counts/items, optional non-empty Mask coverage and bounded duration.
- [x] Plugin health/capability/model/Contract conformance and Bundle output tolerances are separate
      checks; either failure prevents Ready.
- [x] Host/SDK startup carries an explicit model-neutral role-to-file map whose files must be regular
      descendants of the verified Bundle root. SAM and single-file official plugins consume it.
- [x] Passing smoke evidence alone makes the Model Instance `Ready` and its stable Profile
      `Available`/selectable; a dedicated regression proves the transition.
- [x] Published Model Asset identity includes exact Plugin package, Bundle/version/digest,
      Instance/Profile revision, every file-role digest, Contract hash and execution provider.
- [x] New Run admission validates the exact Ready instance; process startup re-hashes every frozen
      file before handing the role map to the Plugin.
- [x] Publication creates durable Plugin and Bundle references. Referenced removal and GC are
      blocked with the concrete referencing Workflow location.
- [x] Disable/enable, explicit removal and conservative GC are stateful and content-address aware.
- [x] Focused Rust tests pass: 99 Core, 57 Application plus one explicit billable ignore, 8 Catalog,
      9 Bundle, 20 Server and Plugin Host/SDK/Registry suites. Focused strict Clippy passes.

## M6 evidence

- [x] Expert Model Plugins separates Runtime, Compatible Models, Installed Models, Model Setup and
      References instead of presenting a combined weight-upload card.
- [x] A Plugin without a Bundle says `No compatible model installed` and exposes one primary
      `Install compatible model` action.
- [x] Catalog entries preserve their source catalog ID and show publisher, pinned Bundle digest,
      disk size, license name/digest, redistribution, commercial-use and platform requirements.
- [x] The eight-step installation journey advances only after the corresponding API boundary:
      license acceptance, pinned download/install verification and fixed Model Instance smoke.
- [x] Errors identify the stopped stage and never claim Ready; only a returned Ready instance closes
      the journey and increments selectable model count.
- [x] `.annotmodel` inspect/import is available only as Advanced setup and performs the same static
      verification and post-import smoke gate.
- [x] Default UI contains no `.onnx` file input and gives no instruction to search for encoder or
      decoder filenames.
- [x] Existing raw files are explicitly projected and rendered as `LegacyUnbundledModel`; they are
      not counted as installed Bundles or Ready Model Instances.
- [x] Create local model bundle requires exact legacy roles, source/export metadata, license text and
      acceptance plus a versioned JSON tensor Contract. Rust pack/verify/install/smoke is used and
      the original raw files remain after failure.
- [x] Server regression proves malformed legacy ONNX bytes stop at `ContractMismatch`, create no
      selectable profile and preserve both original files.
- [x] Browser E2E at 1024×900 and 390×844 proves no horizontal overflow, one-primary-action setup,
      labeled controls and Registry-state recovery after reload.
- [x] Verification passes 21 Server tests, 44 Web unit tests, Web production build, two focused
      Chromium scenarios, Rustfmt and strict focused Clippy.

## M7 evidence

- [x] The built-in Catalog materializes
      `org.annotagent.models.fixture-prompted-segmentation@1.0.0` entirely in Rust and installs the
      local package through the same identity/verification boundary as remote Bundles.
- [x] The Fixture includes two legal, tiny, deterministic ONNX graphs, exact role hashes, a complete
      tensor Contract, MIT license/source notice and fixed image/request/expectation/tolerance data.
- [x] `offline_fixture_runs_bundle_plugin_smoke_geometry_and_removal_lifecycle` proves Pack → Verify
      → Install → Contract → Bind → real Plugin/ORT inference → Smoke → Ready Instance → Mask to
      BBox → Geometry Quality → Disable → Remove.
- [x] Smoke preparation now injects one canonical host-scoped Image Artifact and rebinds every
      packaged input Artifact to the fresh image identity, preserving detection/prompt subject
      lineage instead of trusting package-selected Run identities.
- [x] Fixture Ready does not imply publishability: the Model Instance retains auditable Ready
      evidence while its derived Profile remains `Unknown` and non-selectable. The GUI labels it
      Fixture and never says Ready for Workflows.
- [x] EfficientSAM's official Apache-2.0 repository/exporter and author-owned ONNX Space were
      audited. Encoder/decoder SHA-256 identities and the incompatible upstream tensor Contract are
      recorded in `docs/MODEL_CARDS.md`.
- [x] EfficientSAM remains `live-conditional` because the current SAM Plugin Contract is different
      and no reproducible hosted AnnotAgent `.annotmodel` exists. No raw pair is mislabeled as a
      supported Bundle. SAM 2 remains Labs for the same complete-package reason.
- [x] Verification passes 9 Catalog tests, 7 active SAM Plugin tests with one legal-weight test
      explicitly ignored, 21 Server tests, 44 Web tests, production build, two responsive Chromium
      tests, Rustfmt and strict focused Clippy.

## M8 evidence

- [x] `annotagent models` covers Catalog discovery/search/detail, exact-license install/import,
      installed-model listing/testing, instance enable/disable/doctor, references/remove/GC and
      developer Bundle/Catalog inspection without creating another Registry.
- [x] TUI model commands are read-only projections over the same Registry; mutations remain
      explicit CLI/GUI operator actions.
- [x] Pipeline Builder exposes four bounded model-readiness tools and no install, download,
      license-acceptance, import, delete or billable-probe authority.
- [x] The setup recovery integration proves missing model → persisted blocked Draft → separately
      authorized setup → Retry of the same Draft → validation → Dry Run → human submission, while
      preserving manual nodes and configuration.
- [x] `scripts/acceptance.sh` passes both architecture boundary scans, Rustfmt, strict all-target/
      all-feature Clippy, 407 active Rust tests with five explicit external/billable ignores, the
      all-feature workspace build, 44 Web tests, TypeScript, production build, 38 Chromium E2E
      scenarios, doctor and four offline demos.
- [x] `cargo test -p annotagent-plugin-sam-onnx --test fixture_bundle_workflow --all-features --
      --nocapture` separately passes the real Bundle → Plugin process → Rust ORT → Mask → BBox →
      Geometry lifecycle.
- [x] The active source boundary contains no Python, pip, uv, conda or venv launcher; no external
      model asset, credential, remote mutation or push was used.
