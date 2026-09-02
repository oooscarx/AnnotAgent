# AnnotAgent Model Bundle Provisioning Alpha — Master Plan

Last updated: 2026-09-02 CST

The governing specification is preserved verbatim in
`docs/execution/MODEL_BUNDLE_MASTER_PROMPT.md`. This plan is the implementation index for that
specification.

## Product boundary

AnnotAgent keeps four separate identities:

1. `.annotplugin` supplies reviewed Rust executable code, model loading, transforms and typed
   capability contracts.
2. `.annotmodel` supplies immutable model assets, generic file roles, provenance, licenses,
   checksums, tensor contracts and smoke-test vectors.
3. A Model Instance binds one exact installed plugin package to one exact verified Model Bundle and
   one execution provider.
4. An existing Model Profile is the user-facing, revisioned selector and may become Available only
   when its Model Instance is Ready.

No Project or Published Workflow may select a filesystem path. Published versions freeze the
plugin, bundle, file, contract, Model Instance and Model Profile identities.

## Milestones

| Milestone | Deliverable | Commit |
| --- | --- | --- |
| M0 | Reproduce installed SAM runtime with two missing assets; record GUI, persistence and test baseline | `test(models): reproduce unprovisioned sam plugin experience` |
| M1 | Versioned `.annotmodel` API, manifest, generic roles, contracts, provenance and validation | `feat(models): define versioned installable model bundles` |
| M2 | Deterministic pack/inspect/verify, safe ZIP handling, hashes, signatures and staging | `feat(models): verify model bundles before installation` |
| M3 | Built-in/HTTPS curated catalogs, safe transfer, content-addressed installation, storage and APIs | `feat(models): install curated bundles into content-addressed storage` |
| M4 | Plugin role requirements, compatibility resolver, ONNX inspection, Model Instances and Model Profiles | `feat(models): bind verified bundles to rust model plugins` |
| M5 | Fixed smoke tests, immutable workflow asset pins, references, removal protection and garbage collection | `feat(models): validate and pin model assets for reproducible workflows` |
| M6 | Replace primary raw-ONNX setup with compatible-model installation and preserve an honest legacy migration | `feat(ui): replace raw onnx uploads with verified model installation` |
| M7 | Ship the fixture and the first legally audited real prompted-segmentation catalog entry/bundle evidence | `feat(models): ship the first verified prompted-segmentation bundle` |
| M8 | Agent discovery, CLI/TUI, migration, documentation, complete regression and release matrix | `test(release): validate model bundle provisioning alpha` |

## Verification cadence

Every milestone updates the five execution ledgers, runs focused Rust/Web checks and receives one
local commit. M8 runs Rustfmt, strict workspace Clippy, the complete Rust/Web/E2E suite, Rust-only
boundary scans and offline fixture flows. External catalog hosting, third-party weights, accelerator
coverage and license-dependent real-model runs remain explicitly live-conditional.

## Non-goals

There is no marketplace, Python converter/runtime, shell installer, automatic Agent installation,
automatic license acceptance, unknown publisher trust, mutable Published Workflow binding or
bundling of executable plugin code into `.annotmodel`.
