# Workspace Integrity Defect Ledger

Status vocabulary: `reproduced`, `test-added`, `implementing`, `fixed`, `accepted`, `deferred-with-reason`. A fix is not accepted until its regression and milestone checks pass.

## P0 — Local service security boundary

| ID | Defect / impact | Evidence and reproduction | Root cause / planned repair | Regression | Commit | Status |
| --- | --- | --- | --- | --- | --- | --- |
| P0-01 | Cross-origin pages can invoke privileged localhost operations; health leaks paths; plugin install/credentials/billable actions lack layered confirmation | M0 probes reproduced malicious preflight 200 and path disclosure | Strict Origin/Host, local session, CSRF, one-time privileged grants, safe health DTO, and native-plugin Web trust policy implemented | Server security suite plus `web/e2e/security.spec.ts` | M1 | accepted |
| P0-02 | Large bodies, request storms, and unbounded event clients can exhaust the local service | M0 audit found no uniform limits | JSON body, mutation rate/concurrency, expensive-operation concurrency, SSE client/backpressure limits, request IDs and structured limit errors implemented | `integrity_limits_json_bodies_and_sse_clients_with_structured_errors` | M1 | accepted |

## P1 — Identity, data integrity, and backend truth

| ID | Defect / impact | Evidence / location | Repair and regression | Commit | Status |
| --- | --- | --- | --- | --- | --- |
| P1-01 | Run stable Project ID is dropped from API, causing name-based ownership | server `RunSummary`; Web `HistoryRun`; `workspaceContext.ts` | Required Run owner identity is carried end-to-end; duplicate-name and rename API/Web regressions pass | M2 | accepted |
| P1-02 | Every Project can display the same global model bindings | server project summary cloned registry bindings | Project-scoped persisted binding query and API isolation regression pass | M2 | accepted |
| P1-03 | Run artifact may overlay an image from another Project | Run/image selection lacks authoritative owner validation | Server requires one Run-owned stable Image ID for annotations and every Pipeline Artifact; canvas URL and route canonicalization use only that ID | M4 | accepted |
| P1-04 | Results can flatten intermediate artifacts into final output | result projection consumes broad artifact collections | Explicit `RunResultProjection` plus E2E proving candidate-cluster evidence stays Debug-only while one final Review candidate appears in Results | M4 | accepted |
| P1-05 | Project-scoped Review can include another Project | global review source plus client filtering | Project-scoped list/detail/navigation/decision/revision endpoints reject foreign owners; focused Server and browser regressions pass | M7 | accepted |
| P1-06 | Annotation creation on an empty Run can accept a foreign Image | ownership validation was conditional on prior annotations | Authoritative Run–Image write barrier and empty-Run regression pass | M2 | accepted |
| P1-07 | Annotation import selects Run by `project_name` | legacy name lookup in import path | Import now selects only exact stable Project ownership | M2 | accepted |
| P1-08 | Publish does not require a persisted test for the exact current Draft | publication contract lacked an evidence gate | Publication selects only passing exact revision/hash evidence and rejects missing, stale, failed, incomplete, or legacy-unverified records; Application/Server publication regressions pass | M6 | accepted |
| P1-09 | Timestamp freshness and one-row UPSERT overwrite Sample Test history | legacy `workflow_sample_tests` primary key/UPSERT by Draft | Migration 16 creates immutable IDs plus revision/content/input/model hashes; out-of-order completion regression proves stale evidence cannot displace current evidence | M6 | accepted |
| P1-10 | Autosave is last-write-wins | Draft UPSERT had no expected revision | Atomic compare-and-swap, HTTP `If-Match`/409, request cancellation/generation, and two-tab browser recovery regression pass | M6 | accepted |
| P1-11 | Selecting a source box overwrites generic annotation confidence | review editor conflates provenance with quality score | Geometry selection preserves generic confidence and stores the typed source score only in evidence provenance; E2E verifies `not_provided` remains truthful | M7 | accepted |
| P1-12 | Local review edit state leaks between items | edit state is not keyed/reset by review item | Review identity atomically resets all edit/reason/history fields; discard/cancel E2E proves isolation | M7 | accepted |
| P1-13 | Run Detail downloads the global Review queue | client finds run item after global list fetch | Final-result projection supplies Run Review IDs and a bounded Run-scoped Review endpoint replaces global lookup | M7 | accepted |
| P1-14 | Every image displays the aggregate Run status | UI projects Run status over all images | Latest stable Image/Run status query, Batch image summaries, and Project image-card regression | M4 | accepted |
| P1-15 | Image identity is a mutable sorted index | DTO/UI derived image key from ordering | Migration 15, UUID APIs, hash-guarded delete, and duplicate-content batch regression pass | M2 | accepted |
| P1-16 | Top-level association still falls back to `project_name` | `workspaceContext.ts` and related selectors | Name fallback is migration-only; source scan and rename/duplicate regressions pass | M2 | accepted |

## P2 — Routing, recovery, performance, and feature truth

| ID | Defect / impact | Evidence / location | Repair and regression | Commit | Status |
| --- | --- | --- | --- | --- | --- |
| P2-01 | Project Runs/Review are filtered global pages, not child routes | `navigation.ts`, App links | Typed Project child routes, Project-context shell state, and browser regressions pass | M3 | accepted |
| P2-02 | Starting a dataset Run lands on a list, not the new execution | start handler uses `/runs?project_id=` | Dataset and single-image starts return stable identities and navigate directly to their detail routes | M4 | accepted |
| P2-03 | Batch Detail has no deep link | route model lacks batch detail | Stable Project Batch route and real persisted-summary view pass unit/E2E coverage | M3 | accepted |
| P2-04 | Run/Review/back links lose Project scope | links use global paths/query filters | Owner-aware typed links and Back/Forward regression pass | M3 | accepted |
| P2-05 | Run canonicalization drops `project_id` | current canonicalizer reconstructs partial route | Owner resolution replaces the alias with a canonical nested route and preserves typed context | M3 | accepted |
| P2-06 | Legacy `/models` and `/skills` redirect to Vision Workers | `navigation.ts` | Redirects now target `/settings/models` and `/settings/plugins` | M3 | accepted |
| P2-07 | Unknown URLs silently become Home | navigation fallback | Explicit NotFound route preserves the invalid URL | M3 | accepted |
| P2-08 | Canonicalizer drops query state not typed in Route | route parser/builder | Draft, version, Agent Session, Improvement Session, Sample Test, Run and Review identities are typed and round-trip | M3/M5 | accepted |
| P2-09 | Image/node/artifact selection repeatedly moves focus to H1 | route change focus effect treats query changes as page changes | Page-level focus key excludes in-page durable selections; unit/E2E keyboard coverage passes | M5 | accepted |
| P2-10 | URL and localStorage compete as active Project truth | workspace selection bootstrap | Removed localStorage reads from owner selection; only a renamed write-only preference remains | M5 | accepted |
| P2-11 | Run image status filter is a dead control | control state is not applied to list | Removed from single-image Run; real status filtering now lives on Dataset Batch images | M4 | accepted |
| P2-12 | Selecting image/node/artifact forces Debug | coupled selection/view state | Explicit Results/Debug view is URL-owned; Run child selection preserves its compatible view | M5 | accepted |
| P2-13 | Image query lacks full type/ownership validation | loosely parsed query | Stable string Image IDs are parsed and canonicalized, then replaced by the Run-owned server identity | M4/M5 | accepted |
| P2-14 | Debug artifacts load only when checkpoint flag exists | conditional data fetch | Debug endpoint is queried by selected view regardless of checkpoint hint; Results does not fetch intermediate artifacts | M5 | accepted |
| P2-15 | Pipeline URL omits selected Draft/version | route model has no identity query | Typed builder preserves exact Draft or combined immutable `workflow@version`, plus Agent/Improvement session | M3/M5 | accepted |
| P2-16 | Stale async responses overwrite newly selected pages | effects lack cancellation/request identity | Route cache tests prove abort and generation guards; route components cancel the old owner key | M5 | accepted |
| P2-17 | SSE invalidation and frequent polling duplicate synchronization | independent refresh loops | Run SSE precisely invalidates affected resources; reconnect resyncs once; Agent recovery uses 1–10 second exponential backoff | M5 | accepted |
| P2-18 | Advanced graph editor can create unsafe graphs | editor permits invalid connections/states | Technical graph is read-only and explicitly directs edits to the guided editor; E2E proves no mutation controls are exposed | M8 | accepted |
| P2-19 | “Review uncertain result” only scrolls | button has no durable review action | Renamed to “Inspect uncertain samples”; it truthfully navigates inside sandbox evidence and never claims to create Review work | M8 | accepted |
| P2-20 | Sample count ignores dataset size | fixed count/control bounds | Count is clamped to the actual stable Project image set and zero-data state links to Data | M8 | accepted |
| P2-21 | Overview duplicates Build editing | duplicated mutation entry points | Overview configuration is read-only and links to the unique Automation/Labels editors | M8 | accepted |
| P2-22 | Run progress is fabricated from event count | UI derives percentage from event volume | Dataset progress uses persisted completed/total counts; active Image Run uses an honest indeterminate indicator | M4/M8 | accepted |
| P2-23 | Native disabled steps hide their explanation from focus users | disabled controls cannot receive focus | Blocked journey controls use focusable `aria-disabled`, descriptive labels, and visible prerequisite copy | M8 | accepted |
| P2-24 | Errors are global and recovery reloads the whole page | App-level error/reload handling | Errors are keyed to route focus; targeted Retry remounts/refetches only the current view | M5/M8 | accepted |
| P2-25 | Unsaved Review edits have no navigation/close protection | editor lifecycle lacks dirty guard | Item/Project/Run/Improve/Export navigation prompts before discard and browser close uses `beforeunload`; E2E covers cancel/confirm | M7 | accepted |
| P2-26 | Revision History uses `alert(JSON)` | Review UI handler | Scoped revision endpoint and accessible in-page audit dialog with structured before/after fields | M7 | accepted |
| P2-27 | Generic UI hard-codes model/refiner brands | labels/help text contain product-specific names | Detection/refinement labels resolve Registry/evidence display metadata, then humanize identifiers without product branches | M8 | accepted |
| P2-28 | Generic role filtering can reject valid VLM detection models | selector uses broad role instead of node contract | Node type maps to a capability contract used for compatible Model selection; focused unit coverage passes | M8 | accepted |
| P2-29 | New Project recommendations conflict with Geometry Safety | recommendation path omits safety constraints | Only Ready compatible models are proposed; semantic scores never auto-approve bbox geometry and all bbox proposals require Review | M8 | accepted |
| P2-30 | Browser UI asks for server-local file paths | import UI exposes filesystem-path text | Until an upload protocol exists, the advanced source is explicitly labeled server-local and says it is not a browser picker | M8 | accepted |
| P2-31 | Existing tests encode the wrong information architecture | tests expect query-scoped global pages | Old assertions now require nested ownership routes; full Web and E2E suites pass | M3 | accepted |
| P2-32 | Summary/list APIs perform N+1 and unbounded loads | `run_summary` loaded full history per row; Project and Review aggregation scanned unrelated records | Purpose-built bounded Project/execution/Project-Run/Batch-image/Review queries, stable pagination, migration 17 indexes, malformed-History fixture, and 100/1000/1000 performance regression pass | M9 | accepted |
| P2-33 | Huge frontend/server/application files amplify regression risk | App/server/application modules were oversized and coupled routing to business aggregation | Extracted Project workspace summary application service, owned workspace route registry, and Not Found route feature behind existing contracts; full Rust/Web/E2E suites pass without a rewrite | M9 | accepted |
| P2-34 | Terminal paginated lists still expose a load-more action | Manual browser verification showed server `next_offset: null` passed a client `undefined` check, so one-Run Project history displayed “Load older Runs” | `PageMetadata.next_offset` is explicitly nullable; Run and Review state/visibility use `null` as terminal, with Project Run E2E coverage | M10 | accepted |
