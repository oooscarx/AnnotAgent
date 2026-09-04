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
| P1-01 | Run stable Project ID is dropped from API, causing name-based ownership | server `RunSummary`; Web `HistoryRun`; `workspaceContext.ts` | Carry `project_id` end-to-end; duplicate-name and rename tests already added | M2 | test-added |
| P1-02 | Every Project can display the same global model bindings | server project summary clones registry bindings | Query Project bindings by stable ID; API isolation test | M2 | reproduced |
| P1-03 | Run artifact may overlay an image from another Project | Run/image selection lacks authoritative owner validation | Bind artifact/image/run/project on server and reject mismatch | M4 | reproduced |
| P1-04 | Results can flatten intermediate artifacts into final output | result projection consumes broad artifact collections | Store/query explicit final projection; test intermediate artifacts stay Debug-only | M4 | reproduced |
| P1-05 | Project-scoped Review can include another Project | global review source plus client filtering | Scoped Review endpoint and server owner check | M7 | reproduced |
| P1-06 | Annotation creation on an empty Run can accept a foreign Image | ownership validation is not uniform | Require image-run-project relation on every write | M4/M7 | reproduced |
| P1-07 | Annotation import selects Run by `project_name` | legacy name lookup in import path | Require stable target IDs; reject ambiguous legacy payload | M2 | reproduced |
| P1-08 | Publish does not require a persisted test for the exact current Draft | publication contract lacks exact-test gate | Enforce passing content-hash/revision record | M6 | reproduced |
| P1-09 | Timestamp freshness and one-row UPSERT overwrite Sample Test history | `workflow_sample_tests` primary key/UPSERT by draft | Append immutable executions with content hash | M6 | reproduced |
| P1-10 | Autosave is last-write-wins | `save_workflow_draft` UPSERT has no expected revision | Server revision + optimistic concurrency/409 | M6 | reproduced |
| P1-11 | Selecting a source box overwrites generic annotation confidence | review editor conflates provenance with quality score | Separate source/model/reviewer scores and provenance | M7 | reproduced |
| P1-12 | Local review edit state leaks between items | edit state is not keyed/reset by review item | Item-keyed draft state and unsaved guard | M7 | reproduced |
| P1-13 | Run Detail downloads the global Review queue | client finds run item after global list fetch | Run-scoped review summary/detail endpoint | M7/M9 | reproduced |
| P1-14 | Every image displays the aggregate Run status | UI projects Run status over all images | Persist/serve per-image execution status | M4 | reproduced |
| P1-15 | Image identity is a mutable sorted index | DTO/UI derive image key from ordering | Stable persisted `ImageId` plus migration | M2 | reproduced |
| P1-16 | Top-level association still falls back to `project_name` | `workspaceContext.ts` and related selectors | Remove fallback after migration; rename/duplicate tests | M2 | test-added |

## P2 — Routing, recovery, performance, and feature truth

| ID | Defect / impact | Evidence / location | Repair and regression | Commit | Status |
| --- | --- | --- | --- | --- | --- |
| P2-01 | Project Runs/Review are filtered global pages, not child routes | `navigation.ts`, App links | Add ProjectShell route kinds/builders; expected-failure tests added | M3 | test-added |
| P2-02 | Starting a dataset Run lands on a list, not the new execution | start handler uses `/runs?project_id=` | Return IDs and navigate directly to Batch Detail | M4 | reproduced |
| P2-03 | Batch Detail has no deep link | route model lacks batch detail | Add stable batch route/API/view | M3/M4 | test-added |
| P2-04 | Run/Review/back links lose Project scope | links use global paths/query filters | Owner-aware typed route builders | M3 | reproduced |
| P2-05 | Run canonicalization drops `project_id` | current canonicalizer reconstructs partial route | Canonical nested route after owner resolution | M3 | test-added |
| P2-06 | Legacy `/models` and `/skills` redirect to Vision Workers | `navigation.ts` | Redirect to `/settings/models` and `/settings/plugins` | M3 | test-added |
| P2-07 | Unknown URLs silently become Home | navigation fallback | Explicit NotFound route | M3 | test-added |
| P2-08 | Canonicalizer drops query state not typed in Route | route parser/builder | Typed query schema and round-trip tests | M3/M5 | test-added |
| P2-09 | Image/node/artifact selection repeatedly moves focus to H1 | route change focus effect treats query changes as page changes | Focus on pathname/page identity only | M5 | reproduced |
| P2-10 | URL and localStorage compete as active Project truth | workspace selection bootstrap | URL/server owner first; localStorage preference only | M5 | reproduced |
| P2-11 | Run image status filter is a dead control | control state is not applied to list | Implement server/client filtering or remove control | M8 | reproduced |
| P2-12 | Selecting image/node/artifact forces Debug | coupled selection/view state | Preserve explicit view and validate query combinations | M5 | reproduced |
| P2-13 | Image query lacks full type/ownership validation | loosely parsed query | Typed parse plus owner validation | M5 | reproduced |
| P2-14 | Debug artifacts load only when checkpoint flag exists | conditional data fetch | Load inspector artifacts from capability/endpoint truth | M4/M5 | reproduced |
| P2-15 | Pipeline URL omits selected Draft/version | route model has no identity query | Add `draft`/`workflow`/`version` URL state; tests added | M3/M5 | test-added |
| P2-16 | Stale async responses overwrite newly selected pages | effects lack cancellation/request identity | Route-aware query cache and AbortController | M5 | reproduced |
| P2-17 | SSE invalidation and frequent polling duplicate synchronization | independent refresh loops | SSE invalidates cache; one bounded recovery poll | M5 | reproduced |
| P2-18 | Advanced graph editor can create unsafe graphs | editor permits invalid connections/states | Move to read-only/Labs until graph-safe editing is complete | M8 | reproduced |
| P2-19 | “Review uncertain result” only scrolls | button has no durable review action | Create/open real scoped review item or remove | M8 | reproduced |
| P2-20 | Sample count ignores dataset size | fixed count/control bounds | Clamp to available stable image set | M8 | reproduced |
| P2-21 | Overview duplicates Build editing | duplicated mutation entry points | Overview becomes status/next-action surface | M8 | reproduced |
| P2-22 | Run progress is fabricated from event count | UI derives percentage from event volume | Serve completed/total/failed/review counters | M4/M8 | reproduced |
| P2-23 | Native disabled steps hide their explanation from focus users | disabled controls cannot receive focus | Accessible wrapper/description and honest CTA state | M8 | reproduced |
| P2-24 | Errors are global and recovery reloads the whole page | App-level error/reload handling | Route/mutation-scoped errors and targeted retry | M5/M8 | reproduced |
| P2-25 | Unsaved Review edits have no navigation/close protection | editor lifecycle lacks dirty guard | Dirty-state blocker and save/discard flow | M7 | reproduced |
| P2-26 | Revision History uses `alert(JSON)` | Review UI handler | Proper accessible drawer/dialog | M7 | reproduced |
| P2-27 | Generic UI hard-codes model/refiner brands | labels/help text contain product-specific names | Render registry metadata/capabilities | M8 | reproduced |
| P2-28 | Generic role filtering can reject valid VLM detection models | selector uses broad role instead of node contract | Capability-contract compatibility query | M8 | reproduced |
| P2-29 | New Project recommendations conflict with Geometry Safety | recommendation path omits safety constraints | Feed enforced contracts and block unsafe draft | M8 | reproduced |
| P2-30 | Browser UI asks for server-local file paths | import UI exposes filesystem-path text | Upload/chooser protocol with explicit server-source alternative | M8 | reproduced |
| P2-31 | Existing tests encode the wrong information architecture | tests expect query-scoped global pages | New expected-failure route tests added; replace old assertions in M3 | M3 | test-added |
| P2-32 | Summary/list APIs perform N+1 and unbounded loads | `run_summary` loads full history; lists unpaginated | Summary SQL, pagination, indexes, 1000-Run query budget | M9 | reproduced |
| P2-33 | Huge frontend/server/application files amplify regression risk | App/server/application modules are oversized | Covered incremental feature extraction | M9 | reproduced |
