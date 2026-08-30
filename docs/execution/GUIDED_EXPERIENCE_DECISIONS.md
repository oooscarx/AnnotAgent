# Guided Experience Alpha Decisions

Updated: 2026-08-30

## GE-001 — Guidance is an Application projection, not a new persisted lifecycle

`ProjectStage` and `ProjectGuidance` are derived deterministically from persisted Project, Dataset, Workflow, Model, Run, Review, and Export state. They are not a second mutable status column. This prevents drift and makes refresh/restart recovery automatic.

## GE-002 — The server chooses one primary action

The Guidance Engine returns exactly one `primary_action` plus optional secondary/repair actions. React and TUI consume the same action DTO. The client may choose layout but may not reorder business priority.

## GE-003 — Guided and Expert modes share one definition

Automation Recipe, label lanes, and Expert Graph are projections/editors over the same `WorkflowDraft`. No conversion copy or separately persisted guided graph is allowed.

## GE-004 — Journey completion needs persisted evidence

Sample testing and activation cannot be inferred from a valid Draft alone. Guidance will use actual Dry Run and published-version evidence available from storage. A successful Dry Run remains a sandbox action and never writes formal Annotations.

## GE-005 — Global pages are not silently scoped

Global Runs and Review default to all Projects. Project context may be an explicit filter or deep link, but active Project local state cannot silently hide global records.

## GE-006 — Results precede execution internals

Dry Run and Run Detail first answer what was annotated, what was missed, what needs review, time, and cost. Node state, payloads, IDs, and trace remain available in Debug/Inspector and remain URL-addressable.

## GE-007 — Existing working capabilities are evolved in place

The typed DAG, immutable versions, Artifact checkpoint, Replay, geometry editor, provider presets, and export protocol remain authoritative. Guided Experience wraps these capabilities in task-oriented APIs and presentations rather than creating disabled or simulated substitutes.

## GE-008 — Offline evidence is the release baseline

Mock and deterministic backends must complete the full journey. Live VLM/SAM/YOLO configuration is conditional evidence and is never required to prove product-state correctness.

## GE-009 — API keys stay outside product DTOs and repository history

Provider settings may expose only whether a workspace-local credential exists. The API never returns the secret; logs, Guidance, Run summaries, evidence documents, and commits contain no credential material.

## GE-010 — Sample tests are durable evidence, not a Draft-status guess

Migration v5 stores the complete sandbox `WorkflowDryRunReport` per Draft. Guidance reads this report after restart and distinguishes not tested, passed, and needs attention. Publishing counts as activated evidence for historical versions; a Dry Run still creates no formal Run or Annotation.

## GE-011 — Project scope on global pages is URL-only

The remembered Project helps the Project switcher but never filters `/runs` or `/review`. Global lists use all Projects by default; `?project_id=...` is the explicit, shareable filter. A Run or Review detail derives its Project from the persisted record, not from local storage.

## GE-012 — Guided creation composes existing product writes

The four-step creation wizard orchestrates the existing Settings, Project creation, image import, and registry-bounded Advisor endpoints. It does not persist a parallel onboarding record or simulate completion. Project creation remains successful if a later import or recommendation fails; the resulting Project opens with the exact warning and Guidance derives the remaining repair step from server state. Generated IDs and YAML are available under Advanced controls, while user-facing intent and Label names are the default vocabulary.

## GE-013 — Journey steps travel with server Guidance

The Application Guidance projection includes the ordered Data, Labels, Automation, Sample Test, Activation, Full Run, Review, and Export steps. Each carries a semantic state, user-facing detail, and destination. React renders this projection and never reconstructs the primary action from Project fields. A downstream record may remain complete after an upstream configuration changes, but only the backend-selected next step is `current`, `needs_attention`, or `ready`. Project Overview requests the combined summary so header, readiness, blockers, Journey, and actions are one coherent server snapshot.

## GE-014 — Build routes are addressable but prerequisites are server-gated

Data, Labels, Automation, and Test & Activate retain stable URLs for refresh and back/forward navigation. Their availability comes from the Journey included in the server summary: earlier complete steps remain editable, the next valid step is reachable, and later steps render the current Guidance blocker instead of their editor. Data and Label writes are immediate Project mutations; Automation changes use the existing debounced Draft PATCH autosave. `Test samples` and `Activate automation` are presentation names for the unchanged sandbox Dry Run and immutable publication lifecycle.

## GE-015 — Imported data reports quality before it enters the Project

Workspace imports enumerate regular files without following symlinks, accept PNG/JPEG extensions, perform bounded full decode validation, hash content for deduplication, and copy only valid unique images. The API returns discovered, duplicate, corrupt, unsupported, source, and supported-format facts. Removing an image deletes only the selected Project dataset copy after canonical containment checks; it never operates on the original import source.

## GE-016 — Sample Test outcomes are derived at the Artifact boundary

A Sample Test reports business outcomes by de-duplicating the latest Candidate, Classification, or Detection Artifact state for each stable item identity. Candidate output takes precedence over Classification, which takes precedence over raw Detection, so downstream enrichment does not double-count one result. An image with no outcome and no execution error is a valid empty result, not a failed image. Node traces stay intact for Diagnostics but do not define annotation success.

## GE-017 — Full Run estimates are explicit projections, not guarantees

The Application scales measured sample duration, exact decimal cost, and Review count to the real Project image count. The DTO records the sample evidence and projected Dataset scope separately; the UI labels the result as an estimate and explains that Provider and image variation can change it. Activation still publishes only the tested immutable Workflow Version and never starts the Dataset automatically.

## GE-018 — Results and Debug are two projections of one immutable Run

Results reads a server-owned outcome summary derived from persisted formal Annotations, with typed checkpoint Candidate, Classification, and Detection Artifacts as the fallback for Pipelines that have not emitted a formal Annotation. Debug reads the same Run history and checkpoint but exposes execution state, lineage, payloads, Provider context, errors, and Replay. `view=debug` is explicit URL state; legacy Node or Artifact links infer it for compatibility. This keeps Guided and technical presentations consistent without persisting a second Run model or rerunning the Workflow.

## GE-019 — Inbox advancement is a server decision result

Review progress and the item after a human decision are derived from persisted Annotation statuses and stable queue order. Accept-and-next and reject-and-next apply the decision first, then return the exact remaining item and updated progress; React does not remove an optimistic local row and guess what comes next. Project scope is explicit in the queue request and URL. Generic reasons belong to Core Review UX, while additional taxonomy values come only from Skills enabled by the Project. Correction evidence is saved against a real enabled Skill when available, but a generic human decision cannot be blocked merely because a Project has no correction taxonomy.

## GE-020 — Export is a readiness-gated Project snapshot

Export never chooses an arbitrary historical Run. The Application selects the newest terminal Run per Project image, includes only accepted Annotations and their revisions, records processed negative images, and blocks while an image is unprocessed or a Review is unresolved. Format support and recommendation come from the configured exporters against that exact Project snapshot. A successful report is persisted beside the output with a deterministic source fingerprint and is restored only while the current exportable snapshot still matches it. This makes Export the durable end of the guided journey without treating an old output as current after data or annotation changes.
