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

## M2 Model Profile and bindings

| Requirement | Status | Evidence |
|---|---|---|
| Provider and Model are independent | PASS | `ProviderProfile` and `ModelProfile` have separate stable IDs, tables, lifecycle metadata and validation contracts. |
| Model capability contract | PASS | Modalities, protocol features, ten requested task capabilities, declaration provenance, limits and generation defaults are typed and fail-closed. |
| Pricing provenance | PASS | Exact decimal pricing supports token/cache/image/request dimensions and explicit user/provider/preset/unknown source; presets are not represented as timeless truth. |
| Semantic revision enforcement | PASS | Storage accepts revision 1, permits non-semantic metadata/price updates in place, requires `latest + 1` for semantic change, and rejects unnecessary/skipped revisions. |
| Credential rotation is non-semantic | PASS | Frozen-snapshot test rotates the Provider credential locator and reprices the Model while proving the semantic snapshot is byte-equivalent in meaning and contains neither field. |
| Project and Agent binding persistence | PASS | Migration 7 persists Project capability/role bindings and global Pipeline Builder/Vision/Text defaults; Pipeline Builder is an explicit binding role. |
| Binding priority | PASS | Core test proves explicit hierarchy implementation; duplicate same-priority bindings return `Ambiguous` rather than arbitrary selection. |
| Locked binding safety | PASS | Core and SQLite tests prove Agent replacement/deletion is rejected while user-authorized mutation remains possible. |
| Compatibility query | PASS | Provider health/enable, credential configured state, Model status/enable, modality, protocol and capability all contribute typed rejection reasons; list returns only compatible Profiles. |
| Infrastructure fallback separation | PASS | `ProviderRoute` permits only bounded infrastructure failure kinds; semantic fallback remains outside the route contract. |
| Published semantic snapshot contract | PASS | Workflow snapshot can pin Model Profile ID/revision, Provider adapter/base URL, remote ID, defaults, limits and capabilities; snapshot JSON excludes credential and price. |
| M2 migration | PASS | Transactional migration 7 creates revisioned profiles, Project bindings, and global defaults; required-table and round-trip tests pass. |
| M2 Rust validation | PASS | Focused Model tests 5/5; complete Core 60/60 and Storage 11/11; workspace all-feature check, strict all-workspace Clippy and all-workspace tests 255/255 pass. |
| M2 Web/E2E regression | PASS | TypeScript, 36 Vitest tests, production build and isolated Chromium E2E 26/26 pass. |

## M3 Provider API and GUI

| Requirement | Status | Evidence |
|---|---|---|
| Provider preset and CRUD API | PASS | Pure-data presets plus list/create/get/patch/delete routes persist `ProviderProfile`; endpoint/adapter changes fail with 409 while Models reference the Provider. |
| Write-only credential lifecycle | PASS | System credential store, environment and session actions return only configured/source state; API regression asserts a submitted sentinel and Keyring locator are absent from all DTOs. |
| Explicit legacy migration | PASS | Migration endpoint copies legacy source to system credential storage and deletes the old source only when separately requested. No startup or passive check migrates it. |
| Passive connection check | PASS | Mock and HTTP `/models` paths are non-generation requests; health and checked timestamp persist. UI labels the operation as non-billable. |
| Explicit active probe | PASS | API rejects `confirmed_billable=false`; UI presents a possible-charge confirmation; success records Profile revision, request ID, tokens, latency, currency and configured-price cost. |
| Bounded Provider transport | PASS | Lifecycle client rejects redirects, limits response bodies to one MiB, applies validated headers/timeouts and maps HTTP/transport failures to sanitized structured Provider errors. Fixture tests verify discovery authorization and usage parsing. |
| Model discovery truthfulness | PASS | Discovery returns sorted remote IDs and an explicit warning that capabilities remain unknown; it does not fabricate capability or pricing claims. |
| Model Profile lifecycle | PASS | Manual create/edit/disable/lock/delete, semantic next revisions, capability/modality/protocol/pricing inputs and Provider/capability/health/modality/enable/price filters are backed by Registry API. |
| Reference-safe deletion | PASS | Provider and Model deletion enumerate Project bindings, Drafts, published versions, Run snapshots and active-probe usage and return structured 409 plus remediation. Historical references are not cascaded. |
| Project and Agent binding API | PASS | Project GET/PUT validates every Profile and user lock boundary; Agent defaults GET/PUT uses typed global defaults and compatibility validation. |
| Settings information architecture | PASS | Providers, Models, Vision Workers, Storage and Usage are all reachable tabs. LLM/VLM credentials and HTTP Vision Workers remain visibly separate. Legacy `/models` reaches Vision Workers. |
| Usage surface | PASS | Usage page aggregates persisted confirmed Probe records; passive checks produce no fake usage rows. |
| M3 Rust validation | PASS | Provider 39/39, Server 11/11 and Storage 11/11 focused suites; full workspace 257/257 plus doc tests; strict all-workspace/all-target/all-feature Clippy, fmt and build pass. |
| M3 Web/E2E validation | PASS | TypeScript, 36/36 Vitest, production build and isolated Chromium 28/28 pass. New tests cover Provider→Model→Usage and 1024/390 px overflow. In-app browser inspection found no console errors. |

## M4 constrained Node Catalog

| Requirement | Status | Evidence |
|---|---|---|
| Full Node Definition contract | PASS | Public definitions include category, typed named ports/cardinality, object JSON Schema, optional Model capability, node cardinality, side effect, Dry Run support and expert-only state; invalid definitions fail registration. |
| Exact constrained public catalog | PASS | Registry test asserts exactly 16 requested IDs including Image Input, Existing Annotations, Resize, Tile, Crop, Detect/Classify/Segment capabilities, Select & Map, Projection, Attach, Evidence, Validate, Decision, Review and Commit. |
| Internal operation compatibility | PASS | Legacy `filter`, `map_label`, confidence/evidence gates and artifact cache stay in the executable registry for old immutable versions but are absent from `definitions()`. Advanced editor labels existing technical operations as legacy and cannot add them. |
| Runtime policies are not nodes | PASS | Cache, Replay, Retry, Timeout, Budget, Usage, Checkpoint, Run Control and History are separate `RuntimePolicyDefinition` values. API and UI tests/source checks prove Cache/Filter are absent from the public catalog. |
| Resize runtime | PASS | Core runner computes bounded aspect-preserving dimensions, rejects missing/invalid targets, preserves parent reference and root coordinate region; focused behavior test passes. |
| Tile runtime | PASS | Core runner creates bounded deterministic tiles, overlap and maximum-tile enforcement, stable item references, parent lineage and composed normalized root regions; focused behavior test passes. |
| Coordinate Projection runtime | PASS | Local DetectionSet geometry is mapped through the source Image Artifact root region; ambiguous fan-out requires explicit artifact/item lineage and fails closed. Numeric projection test passes. |
| Guided Select & Map | PASS | One public operation performs score/class/query/Project Label selection and mapping without exposing Filter + Map boxes; focused mapping/filter test and UI projection test pass. |
| Guided Decision and Evidence | PASS | One public Decision dispatches confidence/evidence/domain policy modes; one public Combine Evidence uses the typed Candidate Cluster runtime while internal gate/match identities remain compatible. |
| Capability node adapters | PASS | Public Detect/Classify aliases are accepted by the existing typed Skill runners; Segment uses the registered model-execution boundary. Static descriptors require bindings/capabilities. |
| M4 affected validation | PASS | Runtime focused suite 8/8 and constrained Application/API tests pass. Final fmt, strict all-target/all-feature Clippy, all-feature build, Rust 262/262 plus doc tests, TypeScript, Vitest 36/36, production build and isolated Chromium E2E 28/28 pass. |

## Release matrix

| Area | Status | Current evidence / remaining work |
|---|---|---|
| A. Provider | PASS | Multiple persistent Profiles, pure presets, CRUD, passive/active checks, discovery, health, reference protection and Settings lifecycle UI pass offline tests. |
| B. Secret | OPEN | Multi-source secure storage and no-auto-migration behavior pass focused tests; full API/E2E/history/source-scan release evidence remains for M8. |
| C. Model Profile | OPEN | Revisioned lifecycle/API/UI, pricing/protocol provenance, probe usage and snapshot contract pass; real publish/runtime call integration remains for M6–M8. |
| D. Project Binding | OPEN | Persistent hierarchy, Project/Agent APIs, compatibility and locks pass; Project/Node binding UI and end-to-end execution remain. |
| E. Node Catalog | OPEN | M4 exact public catalog, Runtime Policy separation and Core Resize/Tile/Projection behavior pass. End-to-end Existing Annotations/Segment/template execution is re-evidenced in M6–M8 before release PASS. |
| F. Agent | OPEN | Real bounded loop is reusable; Provider/Profile tools and revision-aware compatible binding remain. |
| G. Workflow safety | OPEN | Most grammar, immutable publication and sandbox Dry Run already have tests; must be re-evidenced with new bindings/catalog. |
| H. Product | OPEN | Registry Settings IA passes; Project Build still needs compatible Profile selection and Builder integration in later milestones. |
| I. Regression | OPEN | Existing release tests are strong; full post-migration evidence remains. |

No item is marked PASS merely because a UI control, DTO or Mock path exists.
