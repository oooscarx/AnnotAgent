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

## D009 — Guided vocabulary is a projection, not a second graph

Select detections, Decision and Combine model evidence group adjacent technical nodes for ordinary
authoring. The persisted Workflow and Runtime keep their typed Filter, Map Label, match, attach and
gate nodes. Expert details expose those identities when debugging; Guided actions never rewrite or
silently discard them.

## D010 — Grounding assistance belongs to Detection configuration

Grid assistance is the bounded `grounding_assist` configuration of a Detection step. The provider
receives the unmodified source image first and an optional generated calibration view second. The
legacy `localization_grid` parameter is read only for published-version compatibility.

## D011 — Agent tools form a closed protocol

Pipeline Builder accepts only the versioned Rust Registry of 31 tools. Unknown names fail before
an Application action runs. Shell, code execution, Python, package installation, model download and
arbitrary URL access are not represented by the protocol and are covered by rejection tests.

## D012 — Intermediate Drafts may be invalid, mutations may not escape the Registry

An Agent is allowed to create a structurally incomplete editable Draft so static validation can
guide repair. It cannot introduce unknown node/model/Skill identities, type-invalid connections,
cycles, or mutate Published/Archived content. The ScriptedMock creates its first error by removing a
real connection through the same bounded mutation service, not by inventing a fake model ID.

## D013 — ScriptedMock is a policy, not fake inference

ScriptedMock deterministically chooses the full tool sequence and supplies labelled mock evaluation
observations for CI/course demonstrations. Rust still validates and records every tool. It is never
presented as a real visual-model result; M5 binds the same phases to real sandbox summaries.

## D014 — Provider output selects actions, never owns state

The live provider sees bounded Tool schemas and model-facing Tool results. It cannot submit a whole
Workflow document. Application services own the current Draft, Registry checks, validation, Dry
Run, persistence and stop state. A provider-requested unknown or out-of-order action becomes a
failed auditable Tool result that the next turn can repair.

## D015 — Context is loaded by need

The initial live prompt contains no full Registry, Workflow JSON, image bytes, Run history or
Artifact history. Explicit read tools reveal only the requested bounded subset. Assistant text is
transient conversation context and is not persisted, avoiding hidden chain-of-thought storage.
