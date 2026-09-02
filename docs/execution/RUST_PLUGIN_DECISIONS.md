# Rust Expert Model Plugin Alpha — Decisions

## D001 — Process boundary

Official plugins are separate Rust executables. The host extends the existing loopback HTTP Vision
Protocol instead of loading a Rust dynamic library or defining another inference wire format.

## D002 — Core remains capability-oriented

Plugin API may reuse domain-neutral Core capabilities and typed artifacts. Core receives a generic
plugin-backed model identity; model brands stay in plugin packages and product metadata.

## D003 — Truthful model readiness

`Ready` requires runtime discovery, contract parity, a configured immutable checkpoint identity and
a passed smoke/conformance test. Contract-only packages remain `NeedsWeights`, `UnsupportedPlatform`
or `FailedSmokeTest`; no fixture promotes a production model.

## D004 — Alpha isolation claim

The Alpha promises process isolation, loopback/token authentication, environment and filesystem
minimization, bounded logs/responses, cancellation and crash containment. It does not claim an OS
security sandbox on every target.

## D005 — Deterministic packages

`.annotplugin` uses a deterministic ZIP profile: sorted paths, fixed timestamps, normalized modes,
manifest/checksum validation and traversal rejection. Large weights are provisioned separately.
