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

