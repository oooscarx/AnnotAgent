# Provider Registry + Pipeline Builder Alpha — Acceptance Evidence

Status values: `PASS`, `OPEN`, `LIVE-CONDITIONAL`, `NOT-IN-SCOPE`.

## M0 baseline

| Requirement | Status | Evidence |
|---|---|---|
| Git and recent history inspected | PASS | `main...origin/main [ahead 1]`; recent 20 commits inspected before edits. |
| Master task stored | PASS | `docs/execution/PROVIDER_BUILDER_MASTER_PROMPT.md`, 2711 lines copied from the supplied task. |
| Existing Provider and Secret path verified | PASS | `ServerState`, `LocalSecretStore`, `LegacySystemSecretStore`, `/api/settings`, Web Settings and README inspected. |
| Existing Model/Binding path verified | PASS | Core runtime `ModelRegistry`, application `ModelBinding`, `workflow_catalog`, Draft binding strings and Web model DTO inspected. |
| Existing Agent/Workflow path verified | PASS | Closed Tool Registry, Draft tools, Tool loop, static validation, sandbox Dry Run, approval stop and persistence inspected. |
| API/GUI/TUI/migrations inventoried | PASS | Server router, Settings UI, TUI command parser and migrations 1–5 inspected. |
| Migration plan recorded | PASS | `PROVIDER_BUILDER_MASTER_PLAN.md` and decisions D003/D005/D009. |
| Full Rust baseline | PASS | fmt and strict Clippy passed; 238 Rust tests plus doc tests passed; all-feature build passed. |
| Web baseline | PASS | Typecheck, 36 Vitest tests and production build passed on 2026-08-31. |
| E2E baseline | PASS | Isolated Chromium Playwright suite: 26 passed, 0 failed. |

## Release matrix

| Area | Status | Current evidence / remaining work |
|---|---|---|
| A. Provider | OPEN | Singleton config exists; reusable Profile CRUD, health distinction and deletion protection remain. |
| B. Secret | OPEN | API redaction exists, but new GUI secrets currently default to a plaintext workspace file and automatic reverse migration. |
| C. Model Profile | OPEN | Rich runtime descriptors exist; persistent revisioned Profiles and price/protocol provenance remain. |
| D. Project Binding | OPEN | Draft binding strings and default-vision exist; persistent compatible/locked hierarchy remains. |
| E. Node Catalog | OPEN | Typed Registry and guided projection exist; exact Alpha catalog and Resize/Tile/Projection remain. |
| F. Agent | OPEN | Real bounded loop is reusable; Provider/Profile tools and revision-aware compatible binding remain. |
| G. Workflow safety | OPEN | Most grammar, immutable publication and sandbox Dry Run already have tests; must be re-evidenced with new bindings/catalog. |
| H. Product | OPEN | Settings and Project guided flows exist but use singleton Provider/model configuration. |
| I. Regression | OPEN | Existing release tests are strong; full post-migration evidence remains. |

No item is marked PASS merely because a UI control, DTO or Mock path exists.
