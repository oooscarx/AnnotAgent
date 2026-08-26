# Workflow Alpha Acceptance Evidence

## Baseline — 2026-08-27 03:35 CST

Repository state before Milestone 0 edits:

- `git status --short --branch` → exit 0; `main...origin/main [ahead 6]`; clean worktree.
- `git log --oneline --decorate -12` → exit 0; HEAD was `e0e5cdf`.

Build and test evidence:

| Command | Exit | Evidence |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | No formatting diff. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warnings. |
| `cargo test --workspace --all-features` | 0 | 68 Rust unit/integration tests passed; 0 failed; doc tests passed. |
| `cargo build --workspace --all-features` | 0 | Complete workspace dev build succeeded. |
| `npm run typecheck` in `web/` | 0 | TypeScript check succeeded. |
| `npm run test` in `web/` | 0 | 7 files, 13 tests passed; 0 failed. |
| `npm run build` in `web/` | 0 | Production Vite build succeeded. |
| `cargo run -p annotagent -- doctor` | 0 | Config, workspace, 24-table SQLite schema, migrations, example, Web build, and port checks passed; API key was not set and mock mode remained available. |
| `./scripts/acceptance.sh` | 0 | Re-ran fmt, clippy, all Rust tests/builds, all Web checks/builds, and doctor as one fail-fast acceptance command. |

Observed baseline failures: none in the commands above.

Important limitation of this evidence: the current suites do not cover all Workflow Alpha requirements. Green baseline does not prove a typed DAG, checkpoint recovery, annotation-format imports, full Review editing, or the required demos.

## Release blocking matrix

Status values: `PASS`, `INCOMPLETE`, `LIVE-CONDITIONAL`, or `UNVERIFIED`.

| Area | Current status | Authoritative evidence / missing evidence |
| --- | --- | --- |
| Tool-call replay and structured results | PASS | Runtime/provider/storage tests cover assistant tool calls, ordered tool results, multi-call replay, and structured geometry. |
| Typed Artifact persistence | INCOMPLETE | Core supports the main geometry shapes but Attributes, Relations, and immutable replacement lineage still require audit/implementation. |
| Run/Task state semantics | PASS with audit pending | Empty, partial, duplicate start, active restore, and stale reconciliation tests pass; structured timeout visibility still needs explicit API/GUI evidence. |
| Strongly typed Workflow ports/edges | INCOMPLETE | No explicit `NodePort`/`WorkflowEdge` data model or port-level validator evidence. |
| Published snapshot DAG execution | INCOMPLETE | Published versions persist, but the main Run path does not select and execute them. |
| Checkpoint/restart resume/global budget | INCOMPLETE | No 100-image restart-resume gate. |
| Mixed model backends | INCOMPLETE | Mock, OpenAI-compatible, and HTTP infer adapters exist; deterministic CV, worker discovery endpoints, JSON-only fallback, and health UI remain. |
| Workflow Advisor/editor | INCOMPLETE | Suggest/save/static dry-run/publish work; node/edge lifecycle, sample-image Dry Run, clone/archive/version selection remain. |
| RoboCup hybrid release example | INCOMPLETE | Domain algorithms and an example foundation exist; required templates/evaluation CLI/complete generic DAG execution do not. |
| Review geometry editing | INCOMPLETE | Existing revision UI is not the full bbox/keypoint/polyline/polygon editor with undo/redo and before/after. |
| Annotation import/export round trips | INCOMPLETE | Export tests exist; Native/COCO/LabelMe imports and round-trip reports do not. |
| Security release gate | UNVERIFIED | Existing path/symlink and secret-redaction tests are partial; ZIP traversal, pixel limit, backend endpoint/path control, and full history/export secret scans remain. |
| Real Qwen smoke | LIVE-CONDITIONAL | No environment key was available during baseline; no live success is claimed. |
| Real external model inference | LIVE-CONDITIONAL | No model weights/path were configured; no live success is claimed. |

Milestone-specific evidence will be appended after each isolated commit.
