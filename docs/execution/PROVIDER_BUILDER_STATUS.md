# Provider Registry + Pipeline Builder Alpha — Status

## 当前 Milestone

Milestone 5 — Pipeline Builder Agent tools.

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

## 正在进行内容

- Closing the M4 validation and local commit, then extending the bounded Agent tool catalog in M5.

## 下一步

- Milestone 5: add Provider/Model and Node inspection, revision-aware compatible binding, real Draft
  mutation/undo/comparison, validation, Dry Run, human approval, permissions and audit tools.

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
- Full pre-change baseline: 238 tests and doc tests passed; all-feature build passed.

## 最近 Web 测试

- `npm --prefix web run typecheck`: PASS after M2.
- `npm --prefix web test -- --run`: 36 passed after M2.
- `npm --prefix web run build`: PASS after M2.
- M3 TypeScript check, 36 Vitest tests and production build: PASS.
- M4 TypeScript check and affected Label Pipeline UI tests: 8 passed.
- M4 final Web validation: TypeScript, 36/36 Vitest tests and production build passed.

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
- M4 commit pending at this status write; its hash is filled by the next milestone.

## Release Blocking 剩余项

- A now passes offline Provider lifecycle/API/UI acceptance. B is implemented offline but awaits
  final M8 security/source-scan/native evidence. C and D now have lifecycle API/UI, persistence,
  revision/compatibility/lock tests and snapshot support, but remain open until publication/runtime
  integration and migration are proven. M4 closes the public Node Catalog contract, but E remains
  open until later runtime/template milestones prove every public node end to end. F–I remain open.

## Live-conditional 项

- Real Qwen, OpenAI, OpenRouter and Gemini-compatible requests.
- Native system Keychain interaction on supported desktop environments.
- External network, DNS, certificates, rate limits and provider-specific `/models` behavior.
- Manual native browser confirmation after automated E2E.

## 真实 Blocker

- None for offline implementation, in-memory/keyring-abstraction testing, migration, API, GUI,
  TUI, scripted Agent loop or automated regression work.
