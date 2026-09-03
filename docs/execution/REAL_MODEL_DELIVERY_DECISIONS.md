# Real Prompted-Segmentation Delivery Alpha Decisions

Last updated: 2026-09-03 CST

## RMD-D001 — Capability first, model brand second

The release target is `PromptedSegmentation`, not SAM 2 branding. A smaller audited model with a
direct Rust-executable asset is preferred to an unavailable newer checkpoint.

## RMD-D002 — A different tensor family gets a different Plugin

EfficientSAM is not forced into `org.annotagent.sam-onnx`. If selected, it receives a dedicated
Plugin that exposes the common capability and owns only its own preprocessing/postprocessing.
Core remains model-neutral.

## RMD-D003 — Real assets stay outside Git

Candidate and final model bytes are downloaded only to temporary or generated `dist/` locations.
Git contains the Recipe, expected hashes, Contracts, test vectors that are legally distributable,
Catalog metadata, and code—not large weights.

## RMD-D004 — User installation consumes a finished Bundle

Conversion/export is a maintainer supply-chain concern. The user path accepts an immutable
`.annotmodel`; it does not run Python, clone a repository, execute downloaded code, or ask for
separate model files.

## RMD-D005 — Current persisted Fixture evidence is not production evidence

A Fixture instance may be `Ready` for protocol tests while remaining non-selectable. M0 preserves
this distinction and treats the current absence of a non-Fixture Catalog entry as a regression.

## RMD-D006 — Redirects require explicit supply-chain handling

The audited EfficientSAM assets are served from revision-pinned Hugging Face URLs that redirect to
content storage. Recipe fetching may support bounded public-HTTPS redirects only if every hop is
revalidated and final bytes still match fixed size and SHA-256. Ordinary Catalog download policy
must remain explicit and tested rather than silently following redirects.

## RMD-D007 — EfficientSAM-Ti is accepted for delivery, not yet Supported

EfficientSAM-Ti is the first production candidate because the author-linked, revision-pinned
encoder and decoder are compact Apache-2.0 ONNX files, support box prompts, and both load through
the current Rust ORT CPU runtime on macOS ARM64. Its status remains under construction until real
smoke inference, geometry, installation, Workflow and Replay gates pass.

## RMD-D008 — SAM 2.1 remains Labs

The official SAM 2.1 Tiny distribution is a PyTorch checkpoint and its documented user path
requires Python/PyTorch. It is not exposed as an installable or selectable Rust model. The existing
local `.pt` file is preserved but is not evidence for this release.

## RMD-D009 — Trusted local Catalogs are explicit, verified sources

A maintainer must explicitly add a canonical Catalog root. AnnotAgent verifies `catalog.json`, the
size and digest of every local Bundle, its archive checksums, Manifest identity, publication flags,
capabilities, Plugin compatibility and license digest before persisting the source. Local bundle
resolution is confined to that root's `bundles/` directory; refresh is transactional and fails
without replacing the last verified Catalog.

## RMD-D010 — The first Catalog artifact is unsigned but not publisher-verified

The generated Developer Preview Bundle is deterministically hashed and fully verified, but this
local build has no release signing key. Its Catalog card therefore records
`publisher_verified=false`. A future remote release may change this only when signature
verification is implemented and evidenced; local trust never masquerades as publisher signing.

## RMD-D011 — EfficientSAM-Ti becomes Supported only on evidenced targets

The installed real Bundle passed its fixed official-image smoke through
`org.annotagent.efficientsam-onnx@1.0.0` and Rust ORT CPU, so it is Supported on macOS ARM64. The
Linux x86_64 manifest target remains build-compatible rather than run-verified until M6 has a real
Linux host result. SAM 1 and SAM 2 identities are not attributed to this EfficientSAM model.

## RMD-D012 — Registry paths are identity-derived absolute paths

CLI callers are allowed to pass a relative workspace, but persisted Plugin installation and Model
Bundle content roots must never depend on a later process working directory. Both Registries
canonicalize their data root and reconstruct owned paths from immutable IDs/digests when opened.
This is also a confinement rule: stored state cannot redirect a model process to arbitrary paths.

## RMD-D013 — A real smoke report includes semantic and performance evidence

Session loading alone is insufficient. The smoke gate validates input-prompt lineage, finite typed
artifacts, non-empty mask coverage, Core mask-to-bbox geometry, and consistent encoder/decoder
timing. Cache reuse is reported explicitly as zero encoder compute rather than being confused with
a missing encoder execution.

## RMD-D014 — GUI installation progress is server-owned evidence

The one-click flow starts a typed server-side installation operation instead of guessing progress
around one long HTTP request. The operation records the exact Catalog, Bundle and Plugin identity,
download bytes, current lifecycle stage, Model Instance IDs, structured failure and remediation.
A browser refresh reloads the active operation from the server; a process restart reloads the
durable Bundle, Smoke Test, Model Instance and Model Profile state from the Registries. In-memory
operation history is diagnostic only and is not treated as the durable source of Ready status.

## RMD-D015 — A Ready Model Instance is the Workflow selection boundary

Plugin discovery describes executable capability, but a production Workflow binds
`model-instance:<uuid>`. The projected model inherits the Plugin Contract while adding exact
Bundle, component-file, smoke, profile-revision and provider evidence. Plugin-only Workflows do
not require or recover an unrelated remote Provider credential.

## RMD-D016 — Existing geometry is an explicit typed source

Geometry refinement may begin from an exact prior Run's persisted bounding boxes through
`core.existing_annotations`. The node is Project- and Run-scoped, emits a DetectionSet with coarse
geometry semantics and source Annotation lineage, and fails if no matching box exists. It is not a
hidden database fallback and does not add any detector-specific branch to Core.

## RMD-D017 — Local model execution preserves source pixels

The prior shared model thumbnail could silently resize pixels while the Image Artifact retained
the original dimensions. This is invalid for dense mask lineage. Workflows containing an immutable
Plugin/Model Instance binding therefore pass the original image pixels to that local runtime;
small images are never upscaled. Remote VLM paths retain their bounded thumbnail behavior.

## RMD-D018 — Local immutable Replay is credential-free but not fixture-enabled

Replay and post-Review resume reuse the Run's provider mode. A `core` Run can re-execute an exact
frozen local Model Instance because its Bundle and Plugin identities are persisted and rehashed.
Remote Provider model nodes still refuse Replay without an explicit current credential binding;
the implementation does not switch production history to `mock` merely to make Replay proceed.
