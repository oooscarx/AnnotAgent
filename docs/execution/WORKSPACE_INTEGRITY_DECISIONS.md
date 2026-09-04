# Workspace Integrity Decisions

## D-001 — Stable IDs are the only ownership key

Project names, file names, positions, and list indexes are presentation data. New contracts and routes carry `project_id`, `run_id`, `batch_id`, `image_id`, `review_id`, `draft_id`, and `version_id` explicitly. Legacy name fallback is migration-only and must not decide ownership.

## D-002 — Nested routes are canonical for owned work

Global `/runs` and `/review` remain cross-project discovery indexes. Project work uses `/projects/:projectId/runs`, `/projects/:projectId/runs/:runId`, `/projects/:projectId/batches/:batchId`, and `/projects/:projectId/review/:reviewId`. A canonical object URL may redirect to the real owner after server resolution.

## D-003 — URL plus server state are durable UI truth

Selected Draft/version/session/image/view/node/artifact/review item belongs in the URL when refresh or sharing must preserve it. Local storage may remember preferences but cannot establish ownership or object identity.

## D-004 — Security is layered, not a CORS-only fix

The localhost server will require a strict trusted Origin/Host policy, a local session, CSRF protection for mutation, and an additional short-lived confirmation for privileged operations. Native plugin installation also needs a trusted signature policy. Global request-body, concurrency, and SSE limits are server responsibilities.

## D-005 — Results are projections; artifacts are provenance

Results render only the workflow's final annotation projection. Intermediate model output, crops, masks, prompts, conversions, and checkpoints remain available in Debug/Inspector and cannot be flattened into final annotations.

## D-006 — Workflow tests are immutable evidence

Draft edits increment a server-owned revision. Autosave uses optimistic concurrency. Sample tests are append-only records bound to exact Draft content hash/revision, inputs, model bindings, and outputs. Publication requires a passing record for the current exact Draft.

## D-007 — Incremental extraction over a rewrite

The oversized frontend/server/application modules will be split along route and service boundaries only after behavior has regression coverage. A large rewrite is explicitly rejected because it would obscure ownership and migration failures.

## D-008 — Missing review brief is an input gap

`PRO_REVIEW_BRIEF.md` was requested as an audit input but is absent from this checkout. Work continues from repository evidence and the master prompt; no claim will be made that the missing file was reviewed.

## D-009 — Web native-plugin installation fails closed

The current `.annotplugin` verifier checks package digest, file checksums, manifest safety, and target compatibility, but its signature state is only `unsigned` or `present_unverified`. Neither is a trusted publisher identity. The Web API therefore supports safe inspection but rejects installation for both states. A user who deliberately trusts a local package can use `annotagent plugin inspect` followed by `annotagent plugin install --accept`; a future Web install path requires an actual trusted-signature state rather than another confirmation checkbox.

## D-010 — Local session security is memory-only

The session cookie is HttpOnly, host-only, `SameSite=Strict`, and expires with the browser/server session. CSRF and one-time privileged tokens are never persisted. This intentionally makes restart recovery a fresh handshake while leaving workspace data and credentials untouched.

## D-011 — Image identity is Project path identity with content revision evidence

`ImageId` is a UUIDv5 in the stable Project namespace using the persisted Project-relative path. List position is never identity. This allows two intentional files with identical bytes to remain distinct—a requirement exercised by the 100-image batch—while a content replacement at the same logical path retains the image identity and changes `content_hash`. Deletes require the caller's observed hash, and historical execution records retain their path/hash snapshot. Import still deduplicates matching content as a product policy; direct Project datasets are represented truthfully rather than collapsed.

## D-012 — Unresolved legacy ownership is explicit, never guessed

Startup reconciliation assigns a null legacy `Run.project_id` only when the legacy display name has exactly one current Project owner. Ambiguous or deleted ownership remains orphaned. Required API identity fields use a namespaced `legacy-orphan:<run-id>` resolver token and an `ownership_status` flag, which cannot be mistaken for a valid Project route.

## D-013 — Owner discovery replaces, rather than extends, browser history

Global Run and Review detail URLs remain compatibility entry points because bookmarks can outlive the route migration. Once the stable owner is present in server state, the client uses `history.replaceState` to install the canonical nested Project URL while preserving typed image, node, Artifact, and view context. A mismatched Project/object URL is treated the same way. This prevents a foreign Project shell from displaying owned work and keeps Back from returning to a transient alias.

## D-014 — Dataset Runs aggregate; Image Runs inspect exactly one image

A Dataset Batch owns ordering, aggregate progress, scheduling, budget, and child Run links. Each child Image Run owns exactly one stable Image ID and exposes that identity in Results and Debug. Image search and status filtering belong on Batch detail, never as a fake multi-image browser inside an Image Run.

## D-015 — Pass-through nodes create a new Artifact producer identity

When a Core node validates, gates, or otherwise passes through a typed Pipeline Artifact, its output reference names the current node and declared output port. Retaining the upstream port makes downstream typed routing silently skip valid data and destroys provenance. Internal subject/parent references remain unchanged so fan-out/fan-in lineage is preserved.

## D-016 — Route resources own cancellation and cache identity

Every durable frontend read is named by its stable owner/object key. A query cache, rather than a page-global refresh, owns deduplication, AbortController lifetime, request generation, stale state, and precise invalidation. SSE is an invalidation signal; reconnect performs one authoritative server resynchronization. Agent sessions temporarily use exponential recovery polling because the current event protocol has no Agent-session event.

## D-017 — Local storage is a write-only navigation preference

The browser may remember the last visited Project as `preferredProjectId`, but App startup, global indexes, and object ownership never read it. Canonical URL identity and server DTO ownership always win. This deliberately removes the prior dual-truth `activeProjectId` mechanism.

## D-018 — Sample Test freshness is identity, never wall-clock order

Every Sample Test is an append-only execution record. Publication selects evidence by exact Draft ID, request revision, and semantic content hash, then verifies stable image-set and resolved model-snapshot hashes. Completion time orders history only; it never proves freshness. Consequently, a slow test for revision N may finish after revision N+1 without replacing or authorizing N+1. Legacy timestamp-only rows are retained for audit as `legacy_unverified` and require a new test before publication.

## D-019 — Draft conflicts preserve both branches

Draft content uses an atomic expected-revision write boundary. Within one tab, superseded autosaves are aborted and late responses are generation-guarded. Across tabs, a stale write returns 409 and the editor stops autosaving until the user compares local/server snapshots and either reloads the server copy or persists local work as a new Draft. Silent last-write-wins and destructive automatic merge are both rejected.

## D-020 — Review ownership is resolved from the persisted Run

A Project ID in a URL or request body is a scope assertion, never an ownership assignment. Review reads, queue navigation, decisions, and revision history resolve the Annotation's persisted Run and its stable Project owner, then return 404 for a mismatched Project route. This keeps global Review useful for discovery while preventing a client from moving or displaying a Review under another Project.

## D-021 — Geometry evidence does not redefine confidence

Choosing a detector's source box changes only Annotation geometry. The source model, capability, score value, and score semantics remain typed evidence in provenance and attributes. Generic Annotation confidence is unchanged unless a separate explicit quality policy computes it. Review item identity is also the lifecycle boundary for every local edit field, preventing notes, reasons, and history from crossing between items.

## D-022 — Unreleased mutation is read-only, not disabled theater

The technical Workflow graph remains useful for inspection, but free-form mutation is not safe until typed ports, cycle prevention, deletion constraints, and undo ship as one coherent editor contract. The graph is therefore a read-only, explicitly qualified projection. Overview likewise links to the unique Build editors instead of duplicating mutation controls.

## D-023 — UI labels describe the real side effect

Sample Test uncertainty inspection does not create formal Review work, so it is named as inspection. Execution progress is numeric only when the server provides a real numerator and denominator; an active single-image Run is indeterminate. Errors are scoped to the route that produced them and Retry refetches that view instead of reloading the application.

## D-024 — Semantic confidence never proves bbox geometry

New-Project recommendations may use only Ready models compatible with the exact node capability. Open-vocabulary/VLM detections are coarse candidates, and a semantic confidence score cannot auto-approve their geometry. Every generated bbox proposal therefore includes Human Review unless a future geometry-specific validator produces separately typed evidence.

## D-025 — Local paths must identify their execution boundary

A browser text field cannot be presented as a native file chooser. Until a real upload/chooser protocol exists, AnnotAgent exposes the source only as an advanced server-local path and states that the local server process reads it. Unsupported operating-system actions such as “Open folder” are absent rather than simulated.
