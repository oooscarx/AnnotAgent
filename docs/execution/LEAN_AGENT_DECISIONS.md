# Lean Agent Alpha Decisions

## D001 — Preserve runtime, narrow the product

The existing Published DAG executor, Artifact model, Batch coordinator, Review and Replay are the
baseline. Lean work changes authoring vocabulary and Agent ownership; it does not introduce a
parallel executor.

## D002 — Capability Skills are generic

Classification, Detection and Segmentation are public Capability Skills. Qwen, YOLO, RF-DETR,
LocateAnything, SAM and Mock are Model Backends selected through Model Descriptors. Legacy
brand-specific Skill crates remain compatibility adapters until persisted references are migrated.

## D003 — One visible Agent

Pipeline Builder Agent is the only user-visible Agent in Alpha. Existing runtime Detection Recovery
is presented and evolved as deterministic Fallback Policy because it only executes published,
bounded conditions.

## D004 — Tool calls own mutations

The model may select only Registry-defined Pipeline Builder tools. Rust Application services
validate arguments and perform mutations. The model cannot write a Workflow JSON directly, access
SQLite, execute code, invoke Shell or open an arbitrary URL.

## D005 — Human approval remains explicit

The Agent may create/revise/test a Draft and submit it for approval. It cannot Publish or start a
formal Run. Published Versions remain immutable.

## D006 — Unavailable backends are Labs

Model configuration and health determine recommendation eligibility. A registered but unhealthy or
unconfigured Worker is visible only in Labs/alternatives and blocks publish if left unresolved.

## D007 — Compatibility aliases are Registry-only

Pre-Lean IDs remain registered so stored Projects and immutable versions resolve, but their
manifests are marked `compatibility` with a canonical replacement. The public `/api/skills` catalog
filters them. New examples and authoring use generic Capability IDs.

## D008 — Segmentation can be unavailable without being fake

The generic Segmentation Capability is a real semantic contract but publishes no runnable node or
template until a compatible Model Backend is healthy. SAM remains a Labs Model Binding and the
existing RoboCup adapter is not presented as a general ready backend.
