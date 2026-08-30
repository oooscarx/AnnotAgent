# Provider Registry + Pipeline Builder Alpha — Status

## 当前 Milestone

Milestone 3 — Provider/Model lifecycle API and Settings GUI.

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

## 正在进行内容

- Closing the M2 local commit, then exposing Provider/Model lifecycle operations through safe HTTP
  APIs and the Settings information architecture.

## 下一步

- Milestone 3: Provider presets and Provider/Profile CRUD, credential actions, passive checks,
  explicit billable probes, reference-safe deletion, and Providers/Models/Vision Workers UI.

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
- Full pre-change baseline: 238 tests and doc tests passed; all-feature build passed.

## 最近 Web 测试

- `npm --prefix web run typecheck`: PASS after M2.
- `npm --prefix web test -- --run`: 36 passed after M2.
- `npm --prefix web run build`: PASS after M2.

## 最近 E2E 测试

- `npm --prefix web run test:e2e`: 26 passed, 0 failed after M2 in an isolated workspace.

## 最近本地提交

- Before M0: `5c63a6c fix(web): preserve hero heading spacing on narrow screens`.
- M0: `39af089 docs: establish provider registry and builder baseline`.
- M1: `5be1bf3 feat(provider): add reusable provider profiles and secure credentials`.
- M2 commit pending at this status write; its hash is filled by the next milestone.

## Release Blocking 剩余项

- A remains open for lifecycle API/UI. B is implemented offline but awaits final M8 security/E2E
  evidence. C and D now have contracts, persistence, revision/compatibility/lock tests and snapshot
  support, but remain open until API/UI, publication/runtime integration and migration are proven.
  E–I remain open.

## Live-conditional 项

- Real Qwen, OpenAI, OpenRouter and Gemini-compatible requests.
- Native system Keychain interaction on supported desktop environments.
- External network, DNS, certificates, rate limits and provider-specific `/models` behavior.
- Manual native browser confirmation after automated E2E.

## 真实 Blocker

- None for offline implementation, in-memory/keyring-abstraction testing, migration, API, GUI,
  TUI, scripted Agent loop or automated regression work.
