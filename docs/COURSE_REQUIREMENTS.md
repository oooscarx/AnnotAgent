# Course Requirements Matrix

This matrix maps the course [requirements](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/requirements/), [quick start](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/quick-start/), and [architecture guidance](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/agent-architecture/) to concrete code and commands.

## R1 — Rust core logic

Evidence:

- checked domain model and extension contracts: `crates/annotagent-core/src`;
- real model/tool/validator/refiner loop and state control: `crates/annotagent-runtime/src`;
- deterministic RoboCup algorithms: `crates/annotagent-skill-robocup/src`;
- SQLite transactions and repositories: `crates/annotagent-storage/src/lib.rs`;
- shared application service and Axum routes: `crates/annotagent-application`, `crates/annotagent-server`.

Verify:

```bash
cargo test -p annotagent-core
cargo test -p annotagent-runtime --test skill_extension
cargo test -p annotagent-storage --test vertical_loop
```

## R2 — TUI and Web GUI

Ratatui/crossterm TUI is in `apps/annotagent/src/tui.rs`. React/TypeScript/Vite pages and SVG editor are in `web/src`. Both call the same `LocalApplication` behavior; React does not implement validation or state decisions.

```bash
cargo run -p annotagent -- tui --project examples/robocup/project.yaml
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace
```

## R3 — Configurable model

`OpenAiCompatibleConfig`, `config/default.toml`, `config/qwen3.7-flash.example.toml`, and the Settings page cover endpoint, environment key name, process-only key, model, protocol, output/context control, reasoning mode, temperature, timeout, capabilities, headers, extra fields, pricing, and budgets.

```bash
cargo run -p annotagent -- doctor
cargo run -p annotagent -- run --project <project.yaml> \
  --provider openai_compatible --config <safe-config.toml> --limit 1
```

## R4 — Live progress and interruption

Versioned `RunEvent`, Runtime `EventBus`, application broadcast, and `/api/events` SSE carry task/model/tool/refinement/retry/usage/state progress. `RunControl` centrally enforces pause/resume/cancel and provider cancellation.

```bash
cargo test -p annotagent-runtime control
cargo test -p annotagent-server project_sse_review_revision_and_budget_flow_works_over_http
```

Manual: start the TUI or GUI, begin a run, use pause/resume/cancel, and inspect persisted `/api/runs/{id}/events`.

## R5 — Context and history

`ContextManager` loads current task resources, usage and allowed tools without hidden reasoning. SQLite persists run schema snapshot, events, calls, issues, annotations, revisions, review queue, corrections and usage. History JSON is versioned, redacted, conflict-remapped, and warns about missing images.

```bash
cargo run -p annotagent -- history list
cargo run -p annotagent -- history show <run-id>
cargo run -p annotagent -- history export <run-id> --output run.json
cargo run -p annotagent -- history import run.json
cargo test -p annotagent-storage
```

## R6 — Usage, cost, and budgets

Every completed external/Mock call stores input/output/total token source, image/request counts, duration, request ID, retry count, exact-decimal cost breakdown and aggregate totals. Budget checks stop new calls with `BudgetExceeded`; TUI and GUI render live usage/cost.

```bash
cargo test -p annotagent-core usage
cargo test -p annotagent-storage budget
cargo test -p annotagent-server project_sse_review_revision_and_budget_flow_works_over_http
```
