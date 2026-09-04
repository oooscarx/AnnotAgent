# Workspace Integrity Status

Last updated: 2026-09-04

## Current position

- Completed milestone: M0 — baseline and failing regressions.
- Next milestone: M1 — localhost security boundary.
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

## Confirmed implementation facts

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

## Next exit

M1 makes the security probes normal passing tests and adds session, CSRF, privileged-action, native-plugin, request-body, concurrency, and SSE-limit coverage.
