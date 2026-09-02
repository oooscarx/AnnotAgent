# Model Bundle Provisioning Alpha Status

Last updated: 2026-09-03 CST

## Current Milestone

M4 — Plugin compatibility, ONNX Contract inspection and staged Model Instances.

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

## In progress

- Final M4 evidence update and milestone commit.

## Next

M5 — Rust Plugin smoke tests, immutable Workflow asset snapshots, references and garbage collection.

## Latest verification

| Gate | Result |
| --- | --- |
| Rust workspace tests | PASS — `cargo test --workspace --all-features`; 385 active, 5 explicit external/billable ignores in the established baseline |
| Rust workspace build | PASS — `cargo build --workspace --all-features` |
| Bundle tests | PASS — 9 manifest/package/signature/security tests |
| Catalog/provisioning tests | PASS — 5 tests including resolver failures and a real Rust ORT tensor-descriptor comparison |
| Plugin conformance | PASS — Registry, SAM and YOLO tests; 15 active and 2 explicit external-weight ignores in the focused command |
| Real model smoke | Not run; no legally verified SAM assets are installed |
| Web tests | Last established release baseline: 44 unit, 37 Chromium E2E |
| E2E | M0 records the current route; installation UX changes begin in M6 |
| Local commit | `77e3acb` — M0; `399d89b` — M1; `8adb8b4` — M2; `3994af0` — M3; M4 pending |

## Release-blocking remainder

M5–M8 and every unchecked item in `MODEL_BUNDLE_ACCEPTANCE.md`.

## Live-conditional

- A legally redistributable, officially sourced prompted-segmentation ONNX bundle.
- External HTTPS curated-catalog hosting and signatures.
- GPU execution-provider coverage and non-macOS real-model smoke.
- SAM 2 remains Labs until a complete verified Rust ONNX package exists.

## Real blockers

No external model asset may be published until code license, weight license, redistribution terms,
official asset identity, ONNX tensor contract and a real Rust smoke test are all independently
verified.
