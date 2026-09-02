# Model Bundle Provisioning Decisions

Last updated: 2026-09-02 CST

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
