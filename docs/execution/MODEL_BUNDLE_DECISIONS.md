# Model Bundle Provisioning Decisions

Last updated: 2026-09-03 CST

## D001 — Preserve package separation

`.annotplugin` and `.annotmodel` remain separate archives and registries. A Model Instance is the
only binding between them. This prevents code upgrades from rewriting model identity and prevents a
weight update from silently changing executable behavior.

## D002 — Reuse deterministic ZIP mechanics

The first `.annotmodel` version uses deterministic ZIP because the current Plugin Host already has
reviewed ZIP safety rules, stable timestamps, normalized names, explicit file lists and per-file
SHA-256 validation. Bundle validation remains model-specific and does not reuse Plugin Manifest
types.

## D003 — Generic string file roles

Model file roles are validated lowercase identifiers, not Core enums. `image_encoder` and
`mask_decoder` are manifest data. Adding another multi-file architecture must not require a Core
release.

## D004 — One existing Registry boundary

The Model Bundle component extends the Application-owned model/plugin registry boundary and existing
Model Profiles. It does not add a Provider Registry, Plugin Host or inference protocol.

## D005 — Truthful independent states

Plugin Runtime, Bundle installation and Model Instance readiness have distinct enums. A loadable
plugin may be Ready while no model asset exists. An installed archive is not Ready until contract
and smoke evidence pass.

## D006 — Content identity over paths

Stored files use `models/sha256/<prefix>/<bundle-digest>/...`; API and workflows expose digests and
logical identities, never absolute paths.

## D007 — Human authority at trust boundaries

Only a human may accept licenses or initiate import/download/install/removal. Pipeline Builder may
inspect availability and create an unresolved setup requirement, never perform these mutations.

## D008 — Fixture honesty

The tiny offline fixture is marked Fixture and non-publishable. It validates mechanics and cannot be
presented as SAM or as accuracy evidence.

## D009 — Strict typed TOML before archive work

Manifest parsing denies unknown fields and validates every semantic reference before M2 accepts any
archive bytes. This makes a package checksum proof necessary but not sufficient: a perfectly hashed
archive with an incoherent model role, license or test contract is still invalid.

## D010 — Stream model bytes

Verifier and extractor hash model entries through bounded buffers rather than reading a potentially
multi-gigabyte Bundle into memory. Only the small Manifest, checksum file and signature are captured
for parsing.

## D011 — Sign semantics, not a recursive archive

The Ed25519 payload is the versioned concatenation of the exact Manifest bytes and exact checksum
document bytes. The signature file is outside the checksum set, avoiding recursive identity while
authenticating the complete payload list and every payload digest.

## D012 — Deny redirects and private DNS answers

Curated downloads trade convenience for a narrow trust boundary: the Catalog URL and Bundle URL
must be public HTTPS, every resolved address must be public, and HTTP redirects are rejected rather
than recursively re-evaluated. This closes local-file, loopback, private-network and redirect-based
SSRF paths.

## D013 — Installed is not Ready

M3 installation proves archive, identity, license and storage integrity and therefore produces only
`ModelBundleStatus::Installed`. It does not create an Available Model Profile; compatibility,
runtime contract inspection and smoke evidence are M4/M5 gates.

## D014 — A Model Instance profile is setup-only before smoke evidence

Compatibility and actual ONNX descriptor inspection create a deterministic Model Instance and a
stable Model Profile identity, but its status is `Preparing`, availability is `Unknown` and it is
not selectable. M5 is the only transition to `Ready`/`Available`; neither archive installation nor
matching filenames can bypass that gate.

## D015 — Inspect ONNX through the existing Rust runtime

Contract verification opens each declared role with `annotagent-model-runtime-onnx` and compares
the returned tensor descriptors. No Python exporter, filename convention or parallel ONNX parser is
used in the user installation path.

## D016 — Smoke evidence has two independent layers

Plugin package conformance proves authenticated process health, capability/model discovery and
wire Contracts. Bundle expectations prove the fixed model assets produce a valid typed result
inside declared tolerances. Both must pass; neither an ONNX load nor a non-empty response alone is
sufficient for Ready.

## D017 — Bind roles explicitly at process startup

The Host passes a verified role-to-file map in the existing startup handshake. This avoids making a
filename the compatibility Contract while retaining legacy discovery for migration. Paths must be
regular descendants of the verified content root and are never returned by product APIs.

## D018 — Garbage collection is conservative

Automatic GC removes only disabled and unreferenced installed Bundles plus abandoned staging or
download entries. Enabled assets are retained even without a current Project reference; explicit
removal remains a human action. Published Workflow references are durable blockers.

## D019 — Legacy files are migration inputs, never trusted models

An old Plugin weight-set record is projected as `LegacyUnbundledModel`. Its existing SHA-256 is
useful evidence but cannot substitute for source, license, Bundle identity, tensor Contract or a
fixed smoke test. The migration flow copies rather than moves old files, creates a normal local
`.annotmodel`, binds through the same resolver and never promotes a Contract-mismatched instance.

## D020 — Installation progress follows auditable boundaries

The GUI does not simulate a background stage machine. It displays review-only steps before the
mutation, then advances from Download to Verify only when the atomic install API returns its
persisted verification report, and advances to Smoke Test/Ready only from the exact Model Instance
test response. A synchronous download is labeled as that combined trusted operation rather than
showing fabricated byte progress.

## D021 — A Ready Fixture is still not a publishable model

The built-in prompted-segmentation Fixture executes real generated ONNX graphs in the real Rust
Plugin process, but its Manifest is `fixture=true` and `publishable=false`. A passing Smoke Test may
make the Model Instance `Ready` for lifecycle evidence; the derived Model Profile remains
`Unknown` and non-selectable. GUI copy distinguishes this from Ready for Workflows.

## D022 — Built-in Fixture packages are local Catalog content

The deterministic Fixture `.annotmodel` is generated by Rust below the Registry data root and
indexed by the built-in Catalog. Installation copies this exact local package and performs the same
verification as a downloaded package. Remote Catalog entries still require public HTTPS and never
gain file/private-network access.

## D023 — EfficientSAM is audited but live-conditional

The official project and author-owned Space provide an Apache-2.0 source trail and fixed ONNX
hashes, but the EfficientSAM split tensor/preprocessing Contract differs from the existing SAM
ViT-B Plugin and no AnnotAgent `.annotmodel` release endpoint exists. It is recorded as a concrete
candidate, not inserted as an installable Catalog entry. SAM 2 likewise remains Labs until a full
Rust ONNX package exists.
