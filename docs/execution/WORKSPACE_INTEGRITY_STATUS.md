# Workspace Integrity Status

Last updated: 2026-09-04

## Current position

- Completed milestone: M7 — Review integrity.
- Next milestone: M8 — Feature truth and guided UX.
- Overall state: implementation in progress; not release-ready.
- Remote state: unchanged; no push is authorized.
- Data state: no user data, Run history, credentials, plugins, or model bundles were modified.

## M0 evidence

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --workspace --all-features` | Passed; five external/model-weight checks remain intentionally ignored |
| `cargo build --workspace --all-features` | Passed |
| `npm --prefix web run typecheck` | Passed |
| `npm --prefix web test` | Passed: 12 files, 46 tests |
| `npm --prefix web run build` | Passed; 553.82 kB chunk warning recorded |
| `npm --prefix web run test:e2e` | Passed: 38 tests |
| Targeted integrity navigation tests | Passed as 11 normal + 6 expected failures |
| Ignored Rust integrity probes run explicitly | Failed as intended: untrusted CORS preflight returned 200; health exposed workspace path |

## Baseline findings (historical)

- `CorsLayer::permissive()` protects no localhost privilege boundary.
- `/api/health` serializes absolute workspace and database paths.
- server `RunSummary` omits stable `project_id`; the frontend joins Runs to Projects by `project_name`.
- Project model-binding summaries reuse a global binding collection.
- Project Runs/Review are query-filtered global routes, not true nested routes.
- route canonicalization drops untyped query state and unknown routes resolve to Home.
- Draft autosave is last-write-wins; sample-test persistence overwrites a single row per draft.
- Run list construction loads full history per Run, establishing an N+1 baseline.

## M0 exit

All required ledgers are present, the master prompt is preserved byte-for-byte, expected failures compile and reproduce the insecure/ambiguous baseline, and clean baseline suites remain green. The milestone commit subject is `test(integrity): reproduce workspace ownership and navigation failures`.

## M1 exit

- Removed permissive CORS and reject untrusted Origin, preflight, and non-loopback Host requests.
- Added a process-local HttpOnly `SameSite=Strict` session cookie and mutation CSRF token.
- Added method/path-bound, 30-second, single-use confirmation grants for credentials, billable probes, native plugin/model operations, settings, and deletes.
- Web clients bootstrap and recover the local session without persisting security tokens.
- Unsigned and present-but-unverified native plugins may be inspected but cannot be installed by the Web API; the explicit CLI trust flow remains.
- Added a 2 MiB JSON limit, mutation rate/concurrency limits, a separate expensive action limit, an eight-client SSE cap, lagging-client disconnect, and request IDs with structured 413/429 errors.
- Security evidence: 28 server tests passed, 47 Web unit tests passed, all workspace tests passed, and 39 Playwright tests passed including malicious Origin/session/CSRF coverage.
- Milestone commit subject: `fix(security): protect privileged localhost APIs from cross-origin access`.

## M2 exit

- Added migration 15 for Project-scoped image identities and ownership indexes while preserving existing image rows.
- `ProjectSummary`, `RunSummary`, `ReviewSummary`, and `ImageSummary` now expose their stable owner/object IDs. New Run and Web associations use `project_id`; mutable display names remain display-only.
- Duplicate Project names remain isolated and renaming a Project display name does not detach its historical Runs. Legacy null ownership is reconciled only when a display name has exactly one current owner; unresolved history receives an explicit `legacy-orphan:<run-id>` API identity rather than a guessed Project.
- Formal single-image and batch execution reuse stable Project image identities. Image content and delete APIs use UUIDs; delete requires the content hash observed by the caller. Numeric image URLs are read-only compatibility references and are no longer emitted.
- Annotation writes now require an authoritative Run–Image relation even when the Run has no annotations. Annotation import, export, Project summaries, and frontend selectors no longer use `project_name` fallback.
- Project model summaries query only persisted bindings for that Project. Run model summaries are derived only from immutable published `ModelProfileSnapshot` records; the fabricated `default-vision` binding was removed.
- Evidence: Rust formatting and warning-denying clippy passed; storage/application/server suites passed (including the 100-image batch, duplicate-name/rename API isolation, Project binding isolation, stable image identity, and foreign-image write barrier); Web unit tests passed with the M2 expected failures promoted to normal tests; Web typecheck/build passed.
- Milestone commit subject: `fix(core): preserve stable ownership across projects runs and images`.

## M3 exit

- Added typed canonical routes for Project Run indexes/details, Dataset Batch details, and Project Review indexes/details. Project-owned navigation now retains the Project context switcher, Project sidebar state, and owner-specific breadcrumbs.
- Global `/runs` and `/review` remain unscoped discovery destinations. Query-scoped legacy URLs are replaced with nested Project URLs, and legacy global detail URLs are replaced with the resolved owner's canonical path without adding a duplicate browser-history entry.
- Added an honest Dataset Run detail surface backed by persisted Batch summaries, aggregate status, controls, and child Image Run links. Foreign-owner Run and Batch paths resolve to their actual Project owner.
- Unknown paths and invalid typed Build/settings routes now render Not Found at the original URL instead of silently becoming Home. Malformed encoded path segments also fail closed.
- Pipeline route parsing now retains Draft ID or immutable Workflow/version identity; typed route builders round-trip encoded Project, Run, Batch, Review, image, node, and Artifact identifiers.
- Evidence: 14 navigation unit tests passed; all 55 Web unit tests passed; Web typecheck and production build passed; all 40 Chromium E2E journeys passed, including Project hierarchy, legacy owner resolution, Back/Forward, Batch deep-link, global discovery, and Not Found coverage.
- Milestone commit subject: `refactor(web): keep project-owned work inside the project workspace`.

## M4 exit

- Dataset execution and image execution now have distinct detail surfaces. Starting a Dataset Run opens its Batch detail directly; starting an individual Project image opens the exact Image Run directly.
- Batch detail reads the durable Batch endpoint, shows aggregate counters, usage/cost, controls, filters, and one independently derived status/result/review/failure summary per stable Image ID.
- `RunResultProjection` explicitly names committed Annotation IDs, current Review-candidate IDs, valid no-target images, and failed images. Results renders only those final Annotations; intermediate detections, crops, masks, fallbacks, evidence, and validators remain in Debug.
- Image Run inspection requires exactly one authoritative Run-owned Image ID. Foreign Annotation or Pipeline Artifact image identities fail closed, and the Web canvas loads only the stable owned Image URL. The fake multi-image search/filter inside Image Run was removed.
- Fixed pass-through Artifact identity at Confidence Gate: output Artifacts are rebound to the gate's declared output port, so a valid classifier → gate → Commit pipeline now persists its final Annotation instead of reporting a false empty completion.
- Project image cards now report each image's latest persisted Run status rather than repeating an aggregate Project status.
- Evidence: 99 Core tests, the full Runtime and Storage suites, 60 Application tests (one billable check ignored), 29 Server tests, 55 Web unit tests, Web typecheck/build, and all 40 Chromium E2E journeys passed. The E2E suite explicitly proves final-vs-intermediate projection, stable Image URL restoration, mixed-status Dataset detail, and single-image Debug behavior.
- Milestone commit subject: `fix(run): separate dataset execution final results and debug artifacts`.

## M5 exit

- Added one route-resource cache with stable keys, request deduplication, stale state, retry, AbortController ownership, and response-generation guards. Late generations cannot replace current cached data.
- Project summaries, Workflow Draft lists, persisted Sample Tests, Agent/Improvement sessions, Run Results/Debug/Annotations/Artifacts, Project images, and Review queues/items now participate in route-aware loading and cancel work when their owner route changes.
- Run SSE events invalidate only the affected Run family and targeted Review/project summaries. The full dashboard is re-synchronized only after SSE reconnect or an explicit refresh; the 750 ms Agent interval was replaced by bounded exponential recovery polling.
- Pipeline URLs now preserve exact Draft, immutable `workflow@version`, Pipeline Builder Session, and Improvement Session identity. Test URLs reserve both exact Draft and Sample Test identity. Review/Run in-page selections remain URL-owned.
- Removed `activeProjectId` as browser-state truth. Local storage now records only `preferredProjectId` and is never read to establish ownership.
- H1 focus follows route-page identity, not Draft, Review item, Run view, image, node, or Artifact query changes.
- Evidence: 59 Web unit tests, Web typecheck/build, and all 40 Chromium E2E journeys passed. Added cache generation/dedup/invalidation tests and route round-trip/focus tests.
- Milestone commit subject: `fix(web): restore project run workflow and review context from URLs`.

## M6 exit

- Workflow Drafts now carry a server-owned monotonic `revision` and semantic `content_hash`. Editing writes use atomic compare-and-swap plus `If-Match`; stale tabs receive structured HTTP 409 without overwriting newer content.
- Autosave cancels superseded requests, ignores late generations, rebases same-tab edits onto the acknowledged revision, and presents a recoverable conflict panel with side-by-side snapshots, reload-latest, and save-as-new-Draft actions.
- Migration 16 replaces the one-row-per-Draft Sample Test with append-only test IDs. Each record binds request/Draft revision, Draft hash, stable image IDs and content hashes, image-set hash, exact model revisions and model-snapshot hash, status, report, and start/completion timestamps.
- Sample Test lookup distinguishes exact current evidence from historical stale evidence. An older-revision test that finishes later remains history and cannot displace exact-current evidence or authorize publication.
- Publication requires a passing or explicitly human-approved record for the exact current Draft revision/hash with non-empty input/model snapshot hashes. Legacy timestamp-only tests migrate as `legacy_unverified` and cannot authorize publication.
- Test URLs now retain both exact Draft and immutable Sample Test ID across activation and refresh. Published versions remain immutable; clones begin a new revision lineage.
- Evidence: 61 Application tests plus one ignored billable test, 30 Server tests, 20 Storage unit tests plus 16 integration tests, 60 Web unit tests, Web typecheck/build, and all 41 Chromium E2E journeys passed, including the real two-tab conflict/recovery path.
- Milestone commit subject: `fix(workflow): bind publication to the exact tested draft revision`.

## M7 exit

- Added Project-scoped Review list, detail, navigation, decision, and revision endpoints plus a Run-scoped Review endpoint. Project ownership is derived from the persisted Run; route/body Project IDs are checks and cannot assign ownership.
- Annotation revision and decision writes now reject Image ID changes and verify the Annotation Image still belongs to its source Run.
- Review item identity atomically resets draft, undo/redo, note, reject/correction reason, Skill reason, editing state, and revision-history state. Unsaved geometry/field edits guard item, Project, Run, Improve, Export, and browser-close navigation.
- Selecting detector evidence changes geometry and records typed score semantics in provenance; it no longer overwrites the Annotation's generic confidence summary.
- Revision History is an accessible, inspectable UI region with actor, time, reason, and before/after label/status/geometry rather than an alert containing raw JSON.
- Run ↔ Review links remain canonical Project-owned routes. A Project A Review route receives 404 for a Project B item, and queue advance remains inside the authoritative Project.
- Evidence: formatting passed; the focused Server ownership/decision/revision test passed; 60 Web unit tests, Web typecheck and production build passed; all 41 Chromium E2E journeys passed, including item-state isolation, unsaved-discard protection, score semantics, revision UI, and bidirectional Run/Review navigation.
- Milestone commit subject: `fix(review): preserve ownership edits and provenance across review navigation`.

## Next exit

M8 removes or repairs misleading controls, distinguishes Labs/read-only surfaces, makes progress and sample bounds truthful, and audits capability language and accessibility.
