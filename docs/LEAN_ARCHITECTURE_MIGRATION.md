# Lean Architecture Migration

Status: Milestone 0 baseline, 2026-08-31.

This migration narrows AnnotAgent's public product without deleting the deterministic runtime,
typed artifacts, audit history, or extension contracts that already work. Product language becomes
Project-centric and capability-led; model brands remain backend configuration.

## Preserve

- Project, Dataset, Label Schema, editable Drafts, immutable Published Versions and exact-version Runs.
- Typed DAG validation, Artifact lineage, deterministic cache, Replay and checkpoint recovery.
- Dataset Batch, exact budgets, Pause, Resume, Cancel and duplicate-Run exclusion.
- Results/Debug, Review, revisions, Correction Memory and Native/COCO/YOLO/LabelMe interchange.
- OpenAI-compatible providers, HTTP Vision Protocol v1, Mock backends, SQLite history, Web, TUI and CLI.
- Compile-time Capability and Domain extension contracts. This release does not add a dynamic plugin loader.

## Merge in the guided product

| Existing internal concepts | Guided product concept | Internal compatibility |
|---|---|---|
| Filter + Map Label | Select detections | Existing typed nodes remain valid in Expert mode and persisted versions. |
| Confidence Gate + Evidence Gate | Decision | Existing gate nodes remain executable; the guided editor exposes a decision mode. |
| Candidate Match + Candidate Merge | Combine model evidence | Matching and merge algorithms remain separate runtime implementations. |
| Workflow + Label Pipeline | Automation | APIs and Rust types keep their existing names during the compatibility window. |
| Detection Recovery Agent | Deterministic Fallback Policy | Published behavior stays bounded and reproducible; it is not advertised as a second Agent. |

## Capability and backend boundary

The only public Capability Skills after migration are:

- `annotagent.classification`;
- `annotagent.detection`;
- `annotagent.segmentation`.

The following are Model Backends, not top-level product Skills:

- OpenAI-compatible VLM, including the current Qwen binding;
- Mock classification/detection/segmentation models;
- YOLO HTTP Worker;
- RF-DETR HTTP Worker;
- LocateAnything HTTP Worker;
- SAM HTTP Worker.

The existing brand-specific crates and node IDs are retained temporarily as deprecated compatibility
adapters for stored Projects and immutable Workflow Versions. New authoring resolves the generic
Capability Skill and binds a Model Descriptor. Removal requires a later storage/API migration and
proof that no persisted version references the legacy IDs.

`robocup.ball` remains a Domain Skill. It owns football context, validators, review reasons,
correction taxonomy, advisor resources and optional local refinement policy; it does not own model
selection or a model brand.

## Backend availability baseline

| Backend | Baseline state | Lean product placement |
|---|---|---|
| Mock | Ready | Default offline demonstration and CI. |
| `default-vision` / OpenAI-compatible | Configured, health checked only on request | Ready when credentials and provider request succeed. |
| SAM 2.1 Worker | Worker code present, port 8790 unavailable | Labs; never default-recommended while unavailable. |
| LocateAnything Worker | Registered disabled, port 8791 unavailable | Labs. |
| RF-DETR Worker | Registered disabled/unconfigured, port 8792 unavailable | Labs. |
| YOLO Worker | Reference adapter present, no repository weight | Labs until a healthy versioned Worker is configured. |
| Generic ONNX | Descriptor compatibility only, no runtime | Hidden as unavailable. |

No model weights are part of this migration.

## UI migration

The only global navigation remains Home, Projects, Runs, Review and Settings. Project navigation
remains Overview, Build, Runs, Review and Export. Build remains Data, Labels, Automation and Test &
Activate.

- The current `/workflows/:projectId` route is a compatibility route into the Project Automation
  context, not a second global Workflow product.
- Artifact inspection remains inside Run Debug and Review execution details.
- Provider editing remains in Settings. Model health and Capability availability are views over the
  same saved settings and Registry.
- The former global Skills/Models presentations are migrated to Settings Capabilities/Models; no
  second authoring state is introduced.
- Guided mode uses intent language. Node IDs, Artifact IDs, raw configuration and lineage are Expert
  or Debug-only.

## Data and API compatibility

- Existing Workflow Draft and Published Version payloads remain readable.
- Existing Skill IDs remain accepted as deprecated aliases while Project YAML and stored snapshots
  are migrated to generic Capability IDs.
- Published Versions are never rewritten in place.
- New Pipeline Builder sessions are additive SQLite records and do not mutate existing Run history.
- Agent tools call `LocalApplication`; they do not access SQLite directly.
- API compatibility routes may redirect or delegate, but must not create a second Draft state.

## Removal gates

A deprecated adapter or route can be deleted only after reference search, storage migration,
Project migration, API migration, regression tests and confirmation that no active Project or
Published Version needs it. The Alpha focuses on reducing exposed concepts rather than deleting
working execution code.

