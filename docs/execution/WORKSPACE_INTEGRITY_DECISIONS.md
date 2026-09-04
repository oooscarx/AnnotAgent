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
