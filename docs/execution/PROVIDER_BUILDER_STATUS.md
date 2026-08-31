# Provider Registry + Pipeline Builder Alpha — Status

## 当前 Milestone

Milestone 7 — Project Guided UX and TUI.

## 已完成内容

- Full task preserved in `PROVIDER_BUILDER_MASTER_PROMPT.md`.
- Git baseline inspected: `main`, initially clean and one commit ahead of `origin/main`.
- Current Provider, singleton Settings, workspace-file Secret Store, runtime Model Registry,
  Workflow/Dry Run/Artifact, Pipeline Builder, API, GUI, TUI and migrations inspected from source.
- Migration design and milestone plan recorded.
- Rust baseline passed with the exact fmt, strict Clippy, all-feature test and build commands;
  238 unit/integration tests passed and doc tests passed.
- Web baseline: typecheck passed; 36 Vitest tests passed; production build passed.
- E2E baseline passed 26/26 Chromium tests against an isolated temporary workspace.
- Added a reusable, persistent `ProviderProfile` with adapter kind, validated endpoint, safe
  metadata headers, connection policy, enable state, health and provider-scoped credential
  reference. Multiple same-vendor profiles have independent identities.
- Added the asynchronous `SecretStore` contract and native Keyring, environment-variable,
  session-only, in-memory and legacy-workspace-file implementations behind one router.
- New GUI compatibility writes use the native system credential store. The old workspace key file
  is read as `LegacyWorkspaceFile` only and is never copied or deleted automatically.
- Added transactional SQLite migration 6 and Provider Profile save/get/list/delete persistence;
  stored JSON contains only the opaque credential reference.
- Updated server startup and Settings compatibility API to resolve credentials by reference while
  keeping secret values out of responses, settings TOML and SQLite.
- Updated security/product/API documentation and current Web copy from the superseded plaintext
  workspace-file behavior to the system credential-store boundary.
- Added revisioned `ModelProfile` contracts for input modalities, protocol features, task
  capabilities/provenance, limits, semantic generation defaults, pricing/provenance, health state,
  enable state and lock state.
- Added semantic revision enforcement: semantic changes require the next revision, while pricing,
  health, display metadata and credential rotation keep the current revision.
- Added `ProjectModelBinding`, Pipeline Builder/Inference role bindings, explicit Node Profile
  binding, deterministic Node > capability > role > global resolution and Agent lock enforcement.
- Added fail-closed compatibility queries covering Provider enable/health, credential state, Model
  enable/status, modalities, protocol features and task capabilities.
- Added bounded infrastructure-only `ProviderRoute`; semantic fallback remains a Workflow Decision.
- Extended immutable Workflow snapshots with Model Profile revision and Provider endpoint/adapter
  semantic snapshots that exclude credentials and pricing.
- Added transactional migration 7 and persistence for all Model Profile revisions, Project/Agent
  bindings and global defaults, with revision and lock enforcement at the storage boundary.
- Documented the Provider/Profile/runtime descriptor/Skill/Node/Agent Tool boundaries in
  `docs/PROVIDER_MODEL_REGISTRY.md`.
- Added safe Provider lifecycle HTTP operations: pure-data presets, CRUD, write-only credential
  save/remove/explicit legacy migration, passive connection check, explicitly confirmed active
  probe, `/models` discovery and reference-protected deletion.
- Added bounded Provider lifecycle transport: no redirects, one MiB response limit, safe-header
  allow-list, timeout policy, static sanitized errors and no raw remote body/error echo.
- Added Model Profile lifecycle API, semantic revision creation, filters/compatibility query,
  Project bindings, Agent defaults and immutable active-probe usage persistence in migration 8.
- Rebuilt Settings navigation as Providers, Models, Vision Workers, Storage and Usage. Provider and
  Model forms use the persistent Registry; credential inputs remain write-only; active probes show
  an explicit possible-charge confirmation; Vision Workers retain their independent protocol UI.
- Added model/provider filters, manual capability/protocol/pricing authoring, Model revision edits,
  Provider edit/disable/delete controls, discovery results and truthful empty/unverified states.
- Preserved legacy `/models` as a redirect to Vision Workers while `/settings/models` is the new
  revisioned Model Profile surface.
- Browser validation verified Provider → Model → Usage against an isolated server, no console
  errors, and no page overflow at 1024 px or 390 px.
- Split the executable operation registry from the public `NodeDefinition` catalog. Existing
  Published Workflows retain legacy operation compatibility, while people and the Builder see only
  the 16 constrained Annotation Workflow nodes.
- Added typed Node categories, input/output port cardinality, JSON configuration schema, required
  Model capability, node cardinality, side-effect, Dry Run and expert-only metadata.
- Added executable Core Resize, Tile, Select & Map, Coordinate Projection, Combine Evidence and
  Decision operations. Resize/Tile preserve parent identity and normalized root-image regions;
  Coordinate Projection fails closed on ambiguous lineage and maps local detections to root space.
- Moved cache, replay, retry, timeout, budget, usage, checkpoint, run control and history into a
  separate `RuntimePolicyDefinition` catalog returned by the Workflow Catalog API and rendered as
  non-node runtime behavior in the advanced editor.
- Added generic `capability.detect`, `capability.classify` and `capability.segment` authoring
  identities while retaining Skill/runtime operation adapters behind the execution boundary.
- Updated Guided UI vocabulary and the Label Pipeline node chooser to use Select & Map, Decision
  and Combine Evidence as one product concept each; hidden technical nodes are not addable.
- Replaced the legacy 31-item Builder catalog with the exact 39-tool Alpha contract across Project,
  Skill/Node/Provider/Model discovery, persistent Draft mutation, validation, cost estimation,
  sandbox Dry Run inspection and human-approval/session completion groups.
- Added an explicit permission to every Builder tool. The catalog contains no publish, full-dataset
  Run, Provider/credential mutation, Shell, Python, model-download, arbitrary-URL or whole-JSON
  replacement capability.
- Added credential-safe `PipelineBuilderProviderProfile` summaries and revisioned Model Profiles to
  `WorkflowAdvisorInput`; Provider credential references, locators, safe headers and secret values
  never enter the Builder model context.
- Added passive-only Provider assessment, compatible Model Profile filtering, profile inspection and
  declared-pricing estimation. None of these tools sends a generation request.
- Added real public Node Definition inspection and generic node instantiation with typed ports. New
  nodes can be configured and connected after creation because Rust validates dynamic IDs rather
  than freezing the initial graph into the model schema.
- Added durable explicit `WorkflowModelBinding`, capability validation, Runtime Policy storage
  outside the graph, Workflow-level policy Diff support and a bounded 32-entry session undo journal.
- Added real persisted create/add/remove/connect/disconnect/configure/bind/policy/undo operations;
  every successful live-loop mutation is saved through the normal Draft store boundary.
- Added aggregate Dry Run node statistics alongside bounded sample/artifact inspection. Approval
  still requires a valid static report and a non-committing Dry Run and never publishes or starts a
  formal Run.
- Added security and integration tests proving the exact catalog, forbidden-tool absence, credential
  locator redaction, compatible Profile binding, Runtime Policy undo, persistent Draft mutation and
  no Published Version/formal Run side effects.
- Added Registry-backed Pipeline Builder model resolution with deterministic explicit Profile >
  Project capability/role > global default priority. Selection fails closed unless the Provider and
  Model are enabled/available, the credential reference exists, and the Model declares text input,
  TextGeneration, ToolCalls and StructuredOutput.
- Cut the server `advisor=llm` path over to the selected Provider/Profile. The Server resolves the
  credential only at adapter construction; no secret enters Agent state or model context.
- Added credential-free Agent model selection and per-request audit records containing
  Provider/Profile identity, semantic revision, request ID, usage source, tokens, declared-price
  cost, duration, retry count and status.
- Added complete-exchange context compaction that never leaves an Assistant Tool Call without its
  matching Tool result. Native requests disable parallel Tool Calls and the loop accepts exactly one
  registered Tool Call per turn.
- Added stable structured errors for incompatible/unavailable Model Profiles and retained all
  turn/tool/token/cost/Dry Run budgets, persisted cancellation and human-approval stopping.
- Added a stable Scripted Mock scenario for incompatible Detect binding, compatible rebind, static
  validation repair, high-review Dry Run inspection, crop-classification revision, second Dry Run
  and human approval without publication or a formal Run.
- Added an ignored, explicitly billable opt-in real OpenAI-compatible end-to-end smoke test. Normal
  CI never reads a real key or sends its requests.

## 正在进行内容

- Adding the selected Agent model and compatible Project bindings to Guided Project UX, then
  exposing the same safe identity, progress and trace in TUI.

## 下一步

- Milestone 7: Project model selection, inline Provider setup, Agent model selector, progress,
  proposal comparison, human approval and TUI inspection without exposing credentials.

## 最近 Rust 测试

- `cargo fmt --all --check`: PASS after M1.
- Strict Clippy on Core, Provider, Storage, Server and CLI, all targets/features: PASS after M1.
- Provider Profile focused tests: 5 passed.
- Secret Store focused tests: 4 passed.
- Provider Profile SQLite persistence test: 1 passed.
- Server Keyring-reference restart and legacy no-auto-migration tests: 2 passed.
- Full affected library suites: Core 55, Provider 38, Server 10 and Storage 10 passed (113 total).
- M2 focused Core tests: 5 passed; complete Core suite: 60 passed.
- M2 Storage revision/binding test: PASS; complete Storage suite: 11 passed.
- `cargo check --workspace --all-features`: PASS after M2.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS after M2.
- `cargo test --workspace --all-features`: 255 passed, 0 failed after M2; doc-test groups passed.
- M3 affected library suites: Provider 39, Server 11, Storage 11 passed (61 total).
- M3 strict Clippy for Provider, Storage and Server, all targets: PASS.
- M3 full `cargo test --workspace --all-features --quiet`: 257 passed, 0 failed; doc-test
  groups passed.
- M3 full-workspace all-target/all-feature strict Clippy, fmt check and build: PASS.
- M4 focused Core Runtime tests: 8 passed, including Resize/Tile lineage, coordinate projection and
  combined Select & Map.
- M4 constrained catalog and Runtime Policy Registry test: PASS.
- M4 final workspace validation: fmt, strict all-target/all-feature Clippy and all-feature build
  passed; full Rust suite 262/262 passed and doc-test groups passed.
- M5 focused Core Builder tests: 8 passed, covering exact permissions/forbidden tools, typed Model
  Profile binding, Draft Diff, Runtime Policy, undo, grammar, budgets and human stop.
- M5 complete Application library suite: 38/38 passed, including a real scripted Tool Loop that
  persists node creation, revision-aware binding, Runtime Policy mutation and undo without publish
  or formal Run side effects.
- M5 final workspace validation: fmt, strict all-target/all-feature Clippy and all-feature build
  passed; full Rust suite 265/265 passed and every doc-test group passed.
- M5 secret-pattern scan across application/source/Web/docs (excluding preserved master prompts):
  no API-key-shaped value found.
- M6 focused Application tests pass for Registry priority/requirements, complete Tool-history
  compaction, Profile-priced request audits and the exact incompatible-binding/Dry Run revision.
- M6 final workspace validation: fmt, strict all-target/all-feature Clippy and all-feature build
  passed; full Rust suite 267/267 passed, one explicitly billable smoke ignored, and every doc-test
  group passed.
- Full pre-change baseline: 238 tests and doc tests passed; all-feature build passed.

## 最近 Web 测试

- `npm --prefix web run typecheck`: PASS after M2.
- `npm --prefix web test -- --run`: 36 passed after M2.
- `npm --prefix web run build`: PASS after M2.
- M3 TypeScript check, 36 Vitest tests and production build: PASS.
- M4 TypeScript check and affected Label Pipeline UI tests: 8 passed.
- M4 final Web validation: TypeScript, 36/36 Vitest tests and production build passed.
- M5 Web compatibility validation: TypeScript, 36/36 Vitest tests and production build passed.
- M6 Web compatibility validation: TypeScript, 36/36 Vitest tests and production build passed.

## 最近 E2E 测试

- `npm --prefix web run test:e2e`: 28 passed, 0 failed after M3 in an isolated workspace.
- M4 isolated Chromium E2E: 28 passed, including Crop Artifact lineage with Cache absent from the
  graph and Registry compact-layout coverage.

## 最近本地提交

- Before M0: `5c63a6c fix(web): preserve hero heading spacing on narrow screens`.
- M0: `39af089 docs: establish provider registry and builder baseline`.
- M1: `5be1bf3 feat(provider): add reusable provider profiles and secure credentials`.
- M2: `f8b4437 feat(models): add reusable model profiles and capability bindings`.
- M3: `5a750ef feat(settings): manage llm and vlm providers from one registry`.
- M4: `f3cb72b refactor(workflow): expose a constrained annotation node catalog`.
- M5: `6a2a389 feat(agent): let the builder inspect providers and edit real drafts`.
- M6 commit pending at this status write; its hash is filled by the next milestone.

## Release Blocking 剩余项

- A now passes offline Provider lifecycle/API/UI acceptance. B is implemented offline but awaits
  final M8 security/source-scan/native evidence. C and D now have lifecycle API/UI, persistence,
  revision/compatibility/lock tests and snapshot support, but remain open until publication/runtime
  integration and migration are proven. M4 closes the public Node Catalog contract, but E remains
  open until later runtime/template milestones prove every public node end to end. M6 closes the
  real constrained Agent Tool Loop and Profile-based accounting; Project/TUI product integration
  and G–I remain open for M7–M8.

## Live-conditional 项

- Real Qwen, OpenAI, OpenRouter and Gemini-compatible requests. An ignored opt-in smoke exists and
  requires explicit billable environment configuration.
- Native system Keychain interaction on supported desktop environments.
- External network, DNS, certificates, rate limits and provider-specific `/models` behavior.
- Manual native browser confirmation after automated E2E.

## 真实 Blocker

- None for offline implementation, in-memory/keyring-abstraction testing, migration, API, GUI,
  TUI, scripted Agent loop or automated regression work.
