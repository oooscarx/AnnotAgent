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

## M1 Provider Profile and Secret Store

| Requirement | Status | Evidence |
|---|---|---|
| Reusable Provider Profile | PASS | Core profile owns independent identity, adapter, URL, safe headers, connection policy, enable state, health and credential reference; same-vendor identity test passes. |
| Safe connection metadata | PASS | HTTPS/loopback policy, embedded credentials, schemes, fragments, connection limits and header allow-list are fail-closed in `provider_registry` tests. |
| Native Keyring default | PASS | Production server routes new GUI writes to `KeyringSecretStore`; server no longer depends directly on `keyring` or writes new credential files. |
| Environment and session sources | PASS | Read-only environment and process-local session implementations pass focused tests. |
| CI/test secret source | PASS | Public `InMemorySecretStore` is used by server and storage tests; no desktop credential service is required. |
| Legacy source is explicit | PASS | Legacy file store rejects writes; startup reads the exact registered old path without copying/deleting it. Server regression test verifies no implicit migration. |
| Secret value redaction | PASS | `SecretValue` has no serialization/display contract, zeroizes on drop and its custom Debug emits only `[REDACTED]`. Store errors contain safe generic messages. |
| SQLite stores references only | PASS | Transactional migration 6 and persistence test round-trip `CredentialReference`; raw profile JSON contains the locator and no secret value. |
| Compatibility Settings API is write-only | PASS | Restart test sends a sentinel key, verifies no returned `api_key`, no TOML occurrence, Keyring-reference reuse and explicit deletion. |
| M1 Rust quality gate | PASS | fmt passed; focused tests passed 5 + 4 + 1 + 2; complete affected library suites passed 113/113; strict Clippy passed for Core, Provider, Storage, Server and CLI with all targets/features. |
| M1 Web regression | PASS | TypeScript check and all 36 Vitest tests passed after credential copy updates. |
| M1 browser regression | PASS | Production Web build passed; isolated Chromium E2E passed 26/26 after the server credential-path change. |
| Native desktop Keychain call | LIVE-CONDITIONAL | Requires an available unlocked OS credential service; production adapter is exercised through an injected mock backend offline. |

## Release matrix

| Area | Status | Current evidence / remaining work |
|---|---|---|
| A. Provider | OPEN | Reusable Profile contract and SQLite persistence exist; CRUD API, health operations and deletion protection remain for M3. |
| B. Secret | OPEN | Multi-source secure storage and no-auto-migration behavior pass focused tests; full API/E2E/history/source-scan release evidence remains for M8. |
| C. Model Profile | OPEN | Rich runtime descriptors exist; persistent revisioned Profiles and price/protocol provenance remain. |
| D. Project Binding | OPEN | Draft binding strings and default-vision exist; persistent compatible/locked hierarchy remains. |
| E. Node Catalog | OPEN | Typed Registry and guided projection exist; exact Alpha catalog and Resize/Tile/Projection remain. |
| F. Agent | OPEN | Real bounded loop is reusable; Provider/Profile tools and revision-aware compatible binding remain. |
| G. Workflow safety | OPEN | Most grammar, immutable publication and sandbox Dry Run already have tests; must be re-evidenced with new bindings/catalog. |
| H. Product | OPEN | Settings and Project guided flows exist but use singleton Provider/model configuration. |
| I. Regression | OPEN | Existing release tests are strong; full post-migration evidence remains. |

No item is marked PASS merely because a UI control, DTO or Mock path exists.
