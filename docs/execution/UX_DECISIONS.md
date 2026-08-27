# Guided Workspace UX Decisions

## D-001 — URL-first navigation

Use browser paths and query parameters for project, build step, run image/node/artifact, and review item. UI state mirrors the URL so refresh, history navigation, and copied links restore context.

## D-002 — Five global destinations

The only primary destinations are Home, Projects, Runs, Review, and Settings. Models and Capabilities are Settings sections. Pipeline authoring lives under a Project.

## D-003 — Server-backed project readiness

Readiness, blocking issues, image/task/review counts, active run, last run, and default workflow version are explicit project summary fields computed from persisted server state.

## D-004 — Guided view over the same workflow definition

Shared stages and label pipelines are a projection of the existing workflow draft. Advanced graph is another projection, not a copied configuration.

## D-005 — Contextual inspection

Node artifacts are opened from a run and image context. Review keeps source run, workflow version, source node, reason, confidence, and validation issue visible.

## D-006 — Progressive disclosure

Default node cards show name, binding, typed input/output, essential thresholds, and validation state. DTO fields and advanced parameters live in a drawer.

## D-007 — Honest feature states

Unavailable operations remain visibly disabled with a reason. No client-side demo metrics or fabricated runtime results are used to satisfy acceptance.

## D-008 — Replay binds only the selected subgraph

Replay preserves completed ancestors and registers only the selected node plus downstream consumers. Historical credentials are never recovered, and a Core-only replay does not require an unrelated live model binding upstream.

## D-009 — Export follows persisted annotation identity

Export works for generic Published Workflows without a Skill. It selects a Run containing annotations and resolves the corresponding source image from the persisted workflow image SHA-256 before handing the snapshot to a versioned exporter.
