# Provider Registry + Pipeline Builder Alpha — Status

## 当前 Milestone

Milestone 1 — Provider Profile and Secret Store.

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

## 正在进行内容

- Closing the M0 local commit, then implementing Provider Profile and Secret Store contracts.

## 下一步

- Milestone 1: introduce reusable Provider Profile contracts and the secure multi-source Secret
  Store; remove the automatic Keychain-to-plaintext migration and retain explicit legacy access.

## 最近 Rust 测试

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo test --workspace --all-features`: 238 passed, 0 failed; doc tests passed.
- `cargo build --workspace --all-features`: PASS.

## 最近 Web 测试

- `npm --prefix web run typecheck`: PASS.
- `npm --prefix web test -- --run`: 36 passed, 0 failed.
- `npm --prefix web run build`: PASS.

## 最近 E2E 测试

- `npm --prefix web run test:e2e`: 26 passed, 0 failed in an isolated workspace.

## 最近本地提交

- Before M0: `5c63a6c fix(web): preserve hero heading spacing on narrow screens`.
- M0 commit pending at this status write; its hash is filled after commit by the next milestone.

## Release Blocking 剩余项

- A–I remain OPEN for this Alpha until the new Registry/Profile/Secret semantics and regression
  evidence are implemented. Existing Lean Agent components are reuse candidates, not automatic
  acceptance.

## Live-conditional 项

- Real Qwen, OpenAI, OpenRouter and Gemini-compatible requests.
- Native system Keychain interaction on supported desktop environments.
- External network, DNS, certificates, rate limits and provider-specific `/models` behavior.
- Manual native browser confirmation after automated E2E.

## 真实 Blocker

- None for offline implementation, in-memory/keyring-abstraction testing, migration, API, GUI,
  TUI, scripted Agent loop or automated regression work.
