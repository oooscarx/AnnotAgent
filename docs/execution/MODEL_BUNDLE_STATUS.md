# Model Bundle Provisioning Alpha Status

Last updated: 2026-09-03 CST

## Current Milestone

M6 — Compatible-model installation, legacy raw-file migration and responsive GUI.

## Completed

- Preserved the governing prompt and established the milestone/evidence ledgers.
- Re-ran the all-feature workspace Rust test and build baselines successfully.
- Confirmed `.annotplugin` is a deterministic safe ZIP, while no `.annotmodel` entity exists.
- Confirmed the SAM package declares two components, enters `NeedsWeights`, exposes a
  `MissingWeights` model and cannot be selected or smoke-tested before both files are uploaded.
- Confirmed the primary GUI path is Settings → Expert Model Plugins → installed SAM → Model Setup,
  where two raw file controls currently expose `image_encoder` and `mask_decoder` provisioning.
- Confirmed plugin operational state lives in `.annotagent/plugins/plugin-registry.json`; migration
  13 creates plugin audit tables in SQLite, while normal Model Profiles and Published Workflow
  snapshots use the existing SQLite model/workflow schema.
- Added an explicit regression assertion for both required SAM components and the unprovisioned
  state. Root `dist/` remains on disk but is ignored as generated package output.
- Added the `annotagent-model-bundle` crate with the `.annotmodel` identity, full versioned
  Manifest, model-neutral roles, source/export/runtime/license/contract/test metadata, strict TOML
  validation and independent Bundle/Model Instance status enums.
- Added an explicit runtime-only status enum to the Plugin API. The existing combined legacy status
  remains temporarily for backward compatibility and is migrated by the compatibility milestone.
- Added streaming deterministic ZIP pack, inspect and verify support with exact referenced-file
  sets, bounded compressed/expanded/file sizes, bounded counts, path normalization, duplicate and
  case-conflict detection, link rejection and per-file plus whole-Bundle SHA-256.
- Manifest-declared model sizes/digests, contract digests, transform digests and license digest are
  checked against package bytes. Verification never extracts; extraction re-hashes streamed bytes,
  writes only descendants and removes partial output on failure.
- Added optional Ed25519 verification over the canonical Manifest/checksum payload. Unsigned local
  bundles remain explicit; a trusted official policy can require a valid signature.
- Added `annotagent models bundle pack|inspect|verify`; none of these commands install or convert a
  model.
- Added a curated Catalog crate with strict JSON, unique immutable entries, publisher/license/
  platform/Plugin metadata and persisted local catalogs. Configured refresh accepts only public
  credential-free HTTPS endpoints.
- Added a redirect-denying Rust downloader with DNS/IP private-network checks, bounded streaming,
  cancellation, structured progress, exact size and SHA-256 checks and partial-file cleanup.
- Added durable exact-license acceptance and atomic local/Catalog installation into shared
  `models/sha256/<prefix>/<bundle-digest>` storage. Reinstalling identical bytes is idempotent;
  reusing an ID/version for different bytes fails.
- Application and Server now expose Catalog list/refresh/detail, Bundle installed/available/detail,
  package inspect, local import, curated install, license acceptance and static verification APIs.
  Responses intentionally omit content roots.
- SQLite migration 14 creates all Catalog, Bundle, file, contract, installation, verification,
  smoke, license, instance, health, reference and event tables for the audit boundary.
- Weighted Plugin models now declare generic required file roles. SAM requires `image_encoder` and
  `mask_decoder`; the single-file YOLO, RF-DETR and PIDNet plugins require `model`.
- Plugin executable-runtime status is projected independently from legacy missing-weight state, so
  an installed Rust executable may be compatible even though it has no legacy raw weight upload.
- The compatibility resolver checks installed/enabled Plugin state, exact Plugin/model version
  range, target, execution provider, ONNX format/opset, capability set, required roles, exact Plugin
  model-contract hash and license acceptance, returning a typed reason for every failure.
- Model Contract documents support exact or aliased tensor names, dtype, static/dynamic shape and
  cross-file connections. Every ONNX role is opened through the existing Rust ORT wrapper and its
  actual descriptor is compared with the declared Contract.
- Compatible bindings persist a deterministic Model Instance with exact Plugin package, Bundle,
  file, contract and execution-provider identity. A valid contract produces only `Preparing`; it
  remains non-selectable until M5 records a passing real Plugin smoke test.
- Application/Server expose compatible Bundles, Model Instances and their setup-only Model Profile
  projections without returning local file paths.
- Bundle smoke inputs are data-only request templates; the verifier injects fresh Run/image/request
  identities and bounded image bytes. Expected Artifact kinds/counts, finite Core validation,
  non-empty Mask/coverage ranges and wall-clock tolerance are evaluated independently from Plugin
  package conformance.
- The existing Rust Plugin Host receives an explicit verified role-to-file map. Official ONNX
  plugins use these exact role bindings and retain their legacy filename discovery only for the
  folded migration path. No Python process or conversion path was introduced.
- A passing Plugin conformance report plus Bundle tolerance report is the sole transition from
  `Preparing` to `Ready`; failed/crashed tests persist evidence and stay non-selectable.
- Published Workflow Plugin snapshots may now contain a complete immutable Model Asset reference:
  Plugin package, Bundle/version/digest, Model Instance/Profile revision, every role digest,
  Contract hash and execution provider. New Runs re-hash exact model files before process startup.
- Published Workflow references protect Bundles from removal. Disable affects only new selection;
  enable restores readiness from persisted smoke evidence. GC removes only disabled, unreferenced
  content plus bounded staging/download leftovers.
- Server APIs now cover Bundle/Instance test, compatibility, enable/disable, references, removal and
  conservative garbage collection.
- Settings → Expert Model Plugins now presents Plugin Runtime, Compatible Models, Installed Models,
  Model Setup and References as separate evidence sections. The primary call to action is Install
  compatible model; raw encoder/decoder file controls are absent from the normal path.
- Added the human-controlled installation journey: Select model → Review source → Review license →
  Check compatibility → Download → Verify → Smoke Test → Ready. Catalog identity, size, publisher,
  license digest, platform roles, persisted verification and real smoke errors remain visible.
- Local `.annotmodel` import is an Advanced action that statically verifies first, requires the
  exact license acceptance and executes compatible Model Instance smoke tests after import.
- Existing raw files are projected as `LegacyUnbundledModel` and shown only below collapsed Legacy
  manual provisioning. A Rust-only Create local model bundle flow requires source, export, license
  and Model Contract metadata, packages the preserved files and fails closed before Ready when ONNX
  inspection or the Plugin smoke test fails.
- Compatible Catalog results now preserve their `catalog_id`, so the reviewed entry is the exact
  entry sent to installation rather than a guessed or UI-only source.
- Focused Browser E2E proves the new path at 1024 px and 390 px, no ONNX upload control is exposed,
  page refresh recovers persisted Registry state and the installation journey stays reachable.
- Added the built-in `org.annotagent.models.fixture-prompted-segmentation` Catalog entry. Its Rust
  generator creates a deterministic two-file ONNX Bundle with license, source notice, Contract and
  fixed Smoke Test below the Registry data root; installation resolves locally and still performs
  normal package verification.
- Smoke preparation now injects a canonical host-owned Image Artifact and rebinds packaged Artifact
  image identities, so the real Plugin sees valid image/prompt/detection lineage.
- The Fixture executes through the actual SAM-compatible Rust Plugin process and ONNX Runtime, then
  continues through Mask to BBox and Geometry Quality. Its Ready instance remains non-selectable
  because the Bundle is explicitly Fixture/non-publishable.
- Settings distinguishes Fixture evidence from workflow-ready models in setup completion, summary,
  installed status and primary-action logic.
- Audited EfficientSAM's official repository, Apache-2.0 terms, official exporter Contract and the
  author-owned ONNX Space. Exact Ti encoder/decoder SHA-256 values are recorded in the model card.
  The assets remain live-conditional because their Contract is not the SAM ViT-B Plugin Contract
  and no reproducible hosted AnnotAgent Bundle exists. SAM 2 remains Labs.

## In progress

- Final M7 release checks and milestone commit.

## Next

M8 — Agent/CLI/TUI model provisioning, migration, full regression and release evidence.

## Latest verification

| Gate | Result |
| --- | --- |
| Rust workspace tests | PASS — `cargo test --workspace --all-features`; 385 active, 5 explicit external/billable ignores in the established baseline |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle tests | PASS — 9 manifest/package/signature/security tests |
| Catalog/provisioning tests | PASS — 9 tests including deterministic built-in Fixture, real ORT Contract, smoke tolerance/Ready gate and referenced-GC protection |
| Plugin conformance | PASS — Registry, SAM and YOLO tests; 15 active and 2 explicit external-weight ignores in the focused command |
| Fixture model smoke | PASS — real Rust Plugin/ORT process plus Mask→BBox and Geometry Quality lifecycle |
| Real model smoke | LIVE-CONDITIONAL — EfficientSAM source/hash/Contract audited; no compatible published `.annotmodel` |
| Server tests | PASS — 21, including legacy migration failure/preservation |
| Web tests | PASS — 44 unit plus production TypeScript/Vite build |
| E2E | PASS — focused desktop/mobile verified-Bundle installation journey, 2 scenarios |
| Focused Clippy | PASS — Server, Bundle and Catalog, all targets/features, warnings denied |
| Local commit | `77e3acb` — M0; `399d89b` — M1; `8adb8b4` — M2; `3994af0` — M3; `e8f6ae4` — M4; `045a9e5` — M5; `3a1bb64` — M6; M7 pending |

## Release-blocking remainder

M8 and every unchecked item in `MODEL_BUNDLE_ACCEPTANCE.md`.

## Live-conditional

- An EfficientSAM-specific Rust Plugin Contract, fixed real-image smoke vector and hosted,
  reproducible `.annotmodel` built from the audited official ONNX identities.
- External HTTPS curated-catalog hosting and signatures.
- GPU execution-provider coverage and non-macOS real-model smoke.
- SAM 2 remains Labs until a complete verified Rust ONNX package exists.

## Real blockers

No external model asset may be published until code license, weight license, redistribution terms,
official asset identity, ONNX tensor contract and a real Rust smoke test are all independently
verified.
