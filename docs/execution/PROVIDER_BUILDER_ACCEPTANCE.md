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
| Native Keyring support | PASS | `KeyringSecretStore` is available as an explicit advanced source and is contract-tested through an injected backend. The normal Provider UI defaults to environment-variable or session-only references and writes no credential file. |
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

## M5 Pipeline Builder Agent tools

| Requirement | Status | Evidence |
|---|---|---|
| Exact bounded Tool Catalog | PASS | Core exposes the requested 39 tools in four groups with an explicit permission per tool; contract test asserts the exact count and names. |
| Forbidden capabilities absent | PASS | Registry and live-catalog tests reject/omit publish, full-dataset Run, API-key, Provider CRUD, Shell, Python, model download, arbitrary URL and whole-Workflow JSON replacement tools. |
| Credential-safe Provider discovery | PASS | Builder input uses a dedicated summary with no credential reference/source/locator or headers. Sentinel regression proves the locator is absent from serialized model context. |
| Provider check is passive | PASS | Builder availability result uses the bounded Registry health/endpoint summary, explicitly reports `passive_registry_snapshot` and `billable_request_sent=false`; no active-probe tool exists. |
| Compatible Model Profile discovery | PASS | Listing filters enabled/Available Profile, Provider state, credential configuration and required Node capability; inspection returns revisioned Profile metadata and no secret object. |
| Cost estimation | PASS | Model/Pipeline estimates use exact declared decimal request/image/token pricing and report that no model request was sent. Unknown pricing remains zero with `unknown` provenance rather than an invented price. |
| Node Definition discovery | PASS | List and inspect tools expose only the public 16-node catalog with typed ports/schema/capability/side-effect metadata; runtime policies remain a separate catalog. |
| Real persisted Draft mutation | PASS | Scripted live Tool Loop creates a Draft, adds public nodes, binds a Model Profile, changes Runtime Policy, undoes it and reloads the resulting Draft from SQLite. Core validates duplicate nodes, ports, artifact types, cycles, capabilities and immutable states. |
| Runtime Policy is not a node | PASS | `WorkflowDraft.runtime_policies` stores registered cross-cutting configuration; Diff has a dedicated Workflow policy change and Tool results assert no graph node was added. |
| Compare and undo | PASS | Structured Draft Diff includes nodes/edges/config/legacy+typed model bindings/node policy and Workflow Runtime Policy. A bounded 32-entry session journal restores and persists the exact prior Draft identity. |
| Validation and Dry Run | PASS | Existing Rust static grammar, 1–10 image non-committing Dry Run, summary, failed/review samples and bounded node artifact inspection remain; M5 adds aggregate node statistics. |
| Human approval boundary | PASS | Submission requires a valid static report and completed sandbox Dry Run, saves `Suggested`, stops in `WaitingForHuman`, and tests prove zero Published Versions and zero formal Runs. |
| Audit and budgets | PASS | Every success/failure is recorded through `AgentSession.record_tool` with arguments, sanitized model payload and display summary; existing turn/tool/Dry Run/cost/cancellation limits remain enforced and persisted. |
| M5 focused validation | PASS | Core Pipeline Builder suite 8/8 and complete Application library suite 38/38 pass, including exact-catalog, redaction and persisted-mutation integration tests. Final fmt, strict all-target/all-feature Clippy, all-feature build, Rust 265/265 plus doc tests, TypeScript, Vitest 36/36 and production Web build pass. API-key-shaped source scan is empty. |

## M6 real LLM Tool Loop

| Requirement | Status | Evidence |
|---|---|---|
| Registry model selection | PASS | Application resolves explicit Model Profile, then Project capability/role binding, then global Pipeline Builder default. Focused test proves priority, lock propagation and unresolved failure. |
| Builder model requirements | PASS | Resolution requires enabled/Available Provider and Model, configured credential reference for non-Mock, text input, TextGeneration, ToolCalls and StructuredOutput. Incompatible explicit selection fails before a request. |
| OpenAI-compatible execution | PASS | Server `advisor=llm` resolves the credential from the selected Provider reference, constructs the adapter from Provider/Profile semantics and runs the same constrained loop. The key is never stored in Agent state. |
| Correct Tool Call history | PASS | Existing Provider serialization test proves Assistant tool calls and matching Tool messages are emitted in follow-up requests. M6 compaction test proves only complete Assistant + Tool groups are removed and no orphan result remains. |
| Context management | PASS | Conversation size is bounded by Model Profile context metadata (or a conservative default), while policy/Project messages and four latest complete exchanges remain. Rust state remains authoritative after compaction. |
| One action per turn | PASS | Requests set `parallel_tool_calls=false`; the Agent loop rejects zero or multiple Tool Calls and resolves every name through the closed 39-tool Registry. |
| Profile-priced usage audit | PASS | Agent Session persists safe Provider/Profile ID, model revision and binding source plus per-call request ID, tokens, usage source, duration, declared-price cost, currency, retry count and success. Sentinel serialization contains no credential reference or secret. |
| Budget, cancellation and stop | PASS | Existing persisted turn/tool/token/cost/Dry Run budgets and cancellation tests remain green. Submission requires valid static validation and sandbox Dry Run, saves Suggested and stops at `WaitingForHuman`. |
| Validation repair | PASS | Scripted integration attempts a text-only Profile on Detect and receives structured `incompatible_model_capability`, queries compatible Models, rebinds a compatible Image/ObjectDetection Profile and reaches valid static validation. |
| Dry Run revision | PASS | The same integration inspects bounded high-review evidence, adds the controlled crop-classification revision, validates again, runs a second sandbox Dry Run and submits for human approval. No Published Version or formal Run is created. |
| Scripted Mock | PASS | Stable 20-turn integration exercises discovery, failed and successful binding, graph repair, three validations, two Dry Runs, bounded review inspection, revision and approval. |
| Real Provider smoke | LIVE-CONDITIONAL | Ignored opt-in test requires an explicit billable flag plus base URL/model/key environment variables. Normal CI never reads a real key or sends a request. |
| M6 quality gate | PASS | fmt, strict all-target/all-feature Clippy and all-feature build pass. Full Rust suite passes 267/267 with one explicit billable test ignored; all doc-test groups pass. TypeScript, Vitest 36/36 and production Web build pass. |

## M7 Guided Project UX and TUI

| Requirement | Status | Evidence |
|---|---|---|
| Compatible Project choices | PASS | Guided Build queries the typed compatibility endpoint for Pipeline Builder, Detection, Classification and Verification and persists user choices as locked Project role bindings. Isolated E2E creates an image-capable Profile, selects it for Classification and proves API/UI restoration after reload. |
| Agent model selector | PASS | The primary Agent card names Model Profile and Provider, displays Available/capability/binding state, persists the Project choice and sends `agent_model_profile_id`; incompatible or absent live Profiles cannot start `advisor=llm`. |
| Global defaults | PASS | Settings exposes Pipeline Builder, Vision Language and Text defaults using compatible Available Profiles. Isolated E2E selects a default and proves refresh persistence. |
| Inline Provider setup | PASS | The no-model state embeds preset, model ID and environment/session-only credential setup, preserves the Draft, performs a passive check and gates the separate active probe behind explicit possible-charge confirmation. No browser/workspace/Keychain plaintext path is used. |
| Agent progress and audit | PASS | Persisted Running/Waiting-for-Human sessions recover on Project load; trace shows stage, budgets, Provider/Profile/revision/binding lock, safe Tool results and per-request tokens/cost/duration/retries/errors without hidden reasoning or credentials. |
| Draft approval UX | PASS | Structured node/edge/model/policy Diff, selective apply/all, reject and exact undo remain attached to the persisted Draft. Agent cannot publish or start a formal Run. |
| TUI Registry commands | PASS | `/providers`, show/check, `/models`, show/compatible, `/bindings`, `/bind`, Advisor status/cancel and safe model audit are implemented. TUI tests prove actionable no-profile states and no Authorization output. |
| Responsive and accessible | PASS | Controls use fieldsets, labels, textual status and ARIA progress/live regions. Real-browser inspection passed 1280/1024/390 px without overflow; the isolated compact/keyboard/reduced-motion suite passes. |
| M7 validation | PASS | Full fmt, strict all-target/all-feature Clippy and build pass; Rust passes 269/269 with one explicit billable smoke ignored and all doc tests green. TypeScript, 38/38 Vitest and production build pass. Isolated Chromium E2E passes 29/29 including new binding/default recovery. |

## M8 migration, runtime cutover and release

| Requirement | Status | Evidence |
|---|---|---|
| Explicit legacy data migration | PASS | Migration 9 records an immutable non-secret import fingerprint/report. Preview + confirmed apply atomically create the Provider, revision-1 Model Profile and non-conflicting locked Project bindings; collision rollback and repeat no-op behavior pass Storage/Application/Server tests. |
| Secret and history preservation | PASS | Registry import creates only a `CredentialReference`, never reads/copies/deletes the value, and reports zero historical Run changes. Compatibility GUI writes default to session-only and an explicit rotation removes the replaced Keyring value. Application/Server tests prove history preservation and secret-source behavior; Web E2E proves no secret/history movement or key echo. |
| Publication freezes real Profile semantics | PASS | Publication resolves typed and migrated `default-vision` bindings, writes exact Profile revisions into `WorkflowSnapshot.model_profiles`, and includes them in the immutable content hash. Credentials and current prices remain outside the snapshot. |
| New Run lifecycle admission | PASS | New Run/Batch admission rechecks current Provider/Profile enable and health state while executing endpoint/model/defaults from the frozen revision. A disabled Provider blocks a new Run without changing the Published Version or historical Runs. |
| Draft Dry Run uses the Registry connection | PASS | Dry Run freezes the same Profile snapshots as publication and resolves the selected Provider type plus write-only SecretStore credential. Server regression proves a session credential reaches only Runtime construction and is absent from serialized Provider data. |
| Per-node Profile routing | PASS | Exact Published Runtime integration binds two Classification nodes of the same operation to different frozen Profiles under one Provider and proves each node executes/records its own remote model identity. |
| Provider lifecycle E2E | PASS | Chromium covers reference-protected delete (409), disable removal from compatibility results, health restoration, session-only key rotation without response leakage, legacy import and compact Settings layout. |
| Existing product regression | PASS | Full Rust coverage includes Generic Project, Published Run, Artifact/Review/Replay/Export, exact 100-image Batch pause/restart/resume, HTTP Vision Workers and usage. Full Chromium covers the existing Guided Project, Run, Review, Replay, Export, keyboard, recovery and responsive journeys. |
| Secret/source scan | PASS | API-key-shaped repository scan returned no finding outside preserved master inputs; `web/src` browser storage contains only active Project ID and Review panel preference. `git diff --check` passed. |
| M8 release validation | PASS | fmt, strict all-target/all-feature Clippy and all-feature build pass. Rust passes 275 tests with one explicitly billable smoke ignored and all doc tests green. TypeScript, 38/38 Vitest, production build and Chromium 31/31 pass. |
| Real Provider and native Keyring | LIVE-CONDITIONAL | A billable OpenAI-compatible smoke remains ignored unless explicitly enabled with external credentials; native Keyring requires an available unlocked desktop service. Neither is represented by Mock evidence. |

## Release matrix

| Area | Status | Current evidence / remaining work |
|---|---|---|
| A. Provider | PASS | Multiple persistent Profiles, pure presets, CRUD, passive/active checks, discovery, health, reference protection and Settings lifecycle UI pass offline tests. |
| B. Secret | PASS | Keyring/environment/session/legacy-reference stores, write-only API behavior, explicit migration, Run-history preservation, E2E rotation, browser-storage audit and source scan pass. Native desktop Keyring is separately live-conditional. |
| C. Model Profile | PASS | Revisioned lifecycle, semantic snapshots, per-node same-Provider routing, publication freeze, current lifecycle admission and usage identity pass integration tests. |
| D. Project Binding | PASS | Persistent hierarchy, compatibility, locks, unresolved states, global/Project/Node selection, migrated defaults and Published Runtime resolution pass. |
| E. Node Catalog | PASS | The exact constrained public catalog, hidden Runtime Policies, typed Resize/Tile/Crop/Detect/Classify/Segment/Select/Projection/Attach/Evidence/Validate/Decision/Review/Commit behavior and regression suites pass. Live remote tiled-image materialization remains documented outside the catalog contract. |
| F. Agent | PASS | M6 resolves the Agent's own Registry Profile, runs the real OpenAI-compatible multi-turn loop, preserves Tool history, bounds context, records Profile-priced usage, repairs validation/Dry Run feedback and stops for human approval. |
| G. Workflow safety | PASS | Closed Tool/Node registries, typed ports, cycle/code/Shell/URL bans, Commit grammar, uncertainty routing, immutable publication and non-committing Dry Run pass full regression. |
| H. Product | PASS | Central Settings, reusable credentials, global/Project/Agent selectors, compatibility/health labels, inline first-use setup, generic product identity and responsive/accessible UI pass offline and browser evidence. |
| I. Regression | PASS | Post-migration full workspace tests and 31 Chromium journeys cover Project, Published Run, Batch controls/restart, Artifact, Replay, Review, Export, usage and Vision Workers. |

No item is marked PASS merely because a UI control, DTO or Mock path exists.
