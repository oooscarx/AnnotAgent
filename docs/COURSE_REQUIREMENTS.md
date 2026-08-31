# Course Requirements Matrix

This matrix maps the course [requirements](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/requirements/), [quick start](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/quick-start/), and [architecture guidance](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/agent-architecture/) to concrete code and commands.

The Agent + Skill release adds two explicit Rust-controlled specializations beyond the generic
annotation loop: (1) iterative, registry-bounded Workflow Advisor planning and (2) risk-triggered
Annotation Recovery. The `robocup.ball` Domain Skill is a third specialization built from generic
classification/detection capabilities, deterministic image evidence, domain validators, policy and
project-scoped correction memory. These are workflows and data sources that a generic chat Agent
cannot safely substitute because publication, geometry lineage and annotation commits require
typed, auditable application state.

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

The TUI and GUI both expose persisted Workflow Advisor/Recovery sessions, observable Tool steps,
usage/cost, stopping state and scoped cancellation. The GUI additionally exposes layered Skill
configuration and Correction Memory impact. See `docs/DEMO_AGENT_SKILL.md` for the stable
five-minute path.

The TUI Models panel and `/models test <id>` expose real Worker availability, capability, latency
and missing-score semantics. `/artifacts` and `/replay [node]` inspect typed lineage and cache-aware
sandbox Replay. The GUI adds independent detector evidence and source-box revision controls.

## R3 — Configurable model

`OpenAiCompatibleConfig`, `config/default.toml`, `config/qwen3.7-flash.example.toml`, and the Settings page cover endpoint, environment key name, workspace-local write-only key, model, default run provider, protocol, output/context control, reasoning mode, temperature, timeout, capabilities, headers, extra fields, pricing, and budgets.

Versioned Detection Worker entries additionally configure model ID, architecture/version,
checkpoint SHA-256, training-data version, label space, score semantics, request cost, timeout,
license and explicit remote opt-in. Capability discovery is checked before use.

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

## Course-specialized Agent behavior

The course deliverable contains more than a generic chat loop. Three concrete RoboCup Ball
specializations are implemented and tested in Rust:

1. deterministic white-shoe and white-sock hard-negative evidence;
2. penalty-mark, line-intersection, duplicate-box and unusual-geometry ball checks;
3. a bounded Ball Recovery policy that selects crop evidence, Reject or Human Review and adapts to
   strictly scoped Correction Memory.

The separate Workflow Advisor Agent iteratively inspects registry state, proposes a Draft, consumes
Static Validator and Dry Run reports, revises the Draft and stops for human publication approval.
Neither Agent can bypass the Rust-controlled publish/review/commit boundaries.

The domain-neutral Detection Recovery Agent supplies the mixed-model specialization: it takes a
specialist fast path or makes at most one Registry-bound open-vocabulary call after evidence and
budget checks, then persists the decision and stop condition without hidden reasoning.

Offline demonstration:

```bash
cargo run -p annotagent -- demo generic-classification
cargo run -p annotagent -- demo generic-detection-crop
cargo run -p annotagent -- demo robocup-ball
```

Manual: start the TUI or GUI, begin a run, use pause/resume/cancel, and inspect persisted `/api/runs/{id}/events`.

Expert Vision extension evidence includes a model-brand-neutral Manifest, Python Worker SDK,
health/capability/model/contract discovery, explicit Detection→Prompt→Mask→BBox Runtime execution,
selected-image onboarding and evidence-driven Advisor revision. Deterministic fixtures are labelled
as fixtures; real model accuracy without legal configured weights remains live-conditional.

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
