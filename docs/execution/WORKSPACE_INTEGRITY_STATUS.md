# Workspace Integrity Status

Last updated: 2026-09-04

## Current position

- Completed milestone: M4 — execution and Results integrity.
- Next milestone: M5 — recoverable frontend state.
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

## Next exit

M5 makes every durable frontend selection URL/server reconstructible, prevents stale async responses from replacing a newer route, and turns SSE into cache invalidation rather than competing state.
