# Project-Scoped Workspace Integrity Alpha — Master Plan

This ledger is the durable execution plan for the workspace-integrity program. The authoritative task text is preserved in `WORKSPACE_INTEGRITY_MASTER_PROMPT.md`. Work proceeds milestone-by-milestone; every milestone must have executable evidence and an independent local commit. No milestone authorizes pushing, changing remotes, using paid credentials, or deleting user data.

## Product invariants

1. Ownership is identified by stable IDs, never by display names or list indexes.
2. Project-owned work stays inside a canonical Project route; global pages are discovery indexes.
3. Durable UI state is reconstructible from the URL and server state. Local storage is only a preference cache.
4. Final results, intermediate artifacts, review state, and immutable workflow history remain distinct.
5. The UI exposes only capabilities that are implemented, recoverable, and accurately described.
6. A localhost service is a privileged boundary: cross-origin mutation, secret access, billable actions, and native plugin installation require explicit protection.

## Milestones

| Milestone | Scope | Exit evidence | Required commit | State |
| --- | --- | --- | --- | --- |
| M0 | Baseline, audit, expected-failure regressions | Full baseline plus executable ownership, routing, and security failures | `test(integrity): reproduce workspace ownership and navigation failures` | Complete |
| M1 | Same-origin security boundary | Strict Origin/Host, session, CSRF, privileged confirmation, resource limits, plugin policy | `fix(security): protect privileged localhost APIs from cross-origin access` | Complete |
| M2 | Stable identity and API truth | Stable Project/Run/Review/Image IDs, real bindings, migrations, API tests | `fix(core): preserve stable ownership across projects runs and images` | Complete |
| M3 | Project route model | GlobalShell, ProjectShell, canonical nested routes, redirects, Not Found | `refactor(web): keep project-owned work inside the project workspace` | Complete |
| M4 | Execution and result integrity | Batch/Image detail, real progress, final projection, artifact ownership | `fix(run): separate dataset execution final results and debug artifacts` | Complete |
| M5 | Recoverable frontend state | Route-aware cache, cancellation, URL selections, SSE invalidation | `fix(web): restore project run workflow and review context from URLs` | Complete |
| M6 | Workflow lifecycle | Draft revision, optimistic concurrency, immutable content-addressed sample tests | `fix(workflow): bind publication to the exact tested draft revision` | Complete |
| M7 | Review integrity | Scoped endpoints, item-keyed edits, guards, provenance, navigation | `fix(review): preserve ownership edits and provenance across review navigation` | Complete |
| M8 | Feature truth and guided UX | Truth matrix, remove dead/fake controls, accessibility and honest capability language | `refactor(product): expose only complete and recoverable workspace actions` | Complete |
| M9 | Bounded queries and modularity | Summary SQL, pagination, indexes, N+1 removal, incremental route/service extraction | `refactor(architecture): isolate workspace features and bounded summary queries` | Complete |
| M10 | Release verification | Rust/Web/Playwright/security/performance/responsive/keyboard/docs acceptance | `test(release): validate project-scoped workspace integrity alpha` | Complete |

## Working protocol

- Before each milestone: update status, acceptance, defects, blockers, decisions, and limitations.
- During each milestone: add the regression first, implement the smallest coherent vertical slice, and preserve migration compatibility.
- After each milestone: run proportional Rust and Web checks, record exact evidence, inspect the diff, and create the named local commit.
- Release claims are prohibited until every release-blocking acceptance item has evidence.

## Baseline snapshot — 2026-09-04

- Branch: `main`, 52 local commits ahead of `origin/main`; remote untouched.
- Worktree was clean before M0 test additions.
- `PRO_REVIEW_BRIEF.md` is not present in the repository; the absence is recorded as an input gap, not treated as reviewed evidence.
- Baseline commands passed: Rust format, clippy, workspace tests, workspace build, Web typecheck, Web unit tests (46), Web production build, and Playwright (38).
- Known ignored tests are external/billable/model-weight integration checks and are not silently counted as acceptance evidence.
- Vite reports a 553.82 kB JavaScript chunk; it is tracked under performance/architecture work.
