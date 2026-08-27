# Development Log

This file records work and commands actually performed in the implementation session on 2026-08-26. It does not estimate human hours or invent model conversations.

## Baseline and Git

- The workspace began empty. Git was initialized on `main` and `origin` was set exactly to `git@github.com/AnnotAgent.git`.
- Work was committed by coherent stages: repository, Core contracts, Runtime/storage vertical slice, RoboCup algorithms, CLI/TUI/history/export, real-provider hardening, shared application/server, Web GUI, and server flow tests.
- No API key, `.env`, database, build output, node modules, or live workspace image was added to Git.

## Implementation sequence

1. Read the three course pages and wrote the plan/requirements matrix.
2. Added checked geometry, typed annotations/revisions/IDs, strict Project/Skill schema, extension traits and the `DummySkill` proof.
3. Added Mock/OpenAI-compatible providers, task-focused context, Tool/Skill registries, the actual Agent Loop, control, budget, events and SQLite migration/history.
4. Added the RoboCup DAG, field containment, field-line pixel refiner and validators, ball hard-negative rules, team-color tool, attributes, correction memory and review policy.
5. Added Native/COCO/YOLO Detection/YOLO Segmentation/LabelMe exporters, CLI commands, history import/export and Ratatui.
6. Refactored CLI/TUI/HTTP onto `LocalApplication`, added bounded `DatasetCoordinator`, Axum APIs/SSE/review/settings, and React/Vite pages with SVG editing.
7. Hardened real-provider behavior after live evidence: task-scoped tools, normalized coordinate schemas, dynamic label enum, malformed candidate feedback, dependency failure handling, one evidence-call limit, safe cancellation and secret redaction.
8. Added HTTP integration coverage, docs, CI, demo script, and final acceptance checks.
9. Added product-style settings persistence: non-secret values are atomically stored per workspace, provider keys use the native system credential store, and GUI runs inherit the saved default provider.
10. Integrated AnnotAgent Visual System 1.0 across formal Web/PWA assets, canonical tokens, responsive GUI pages, generic annotation slots, accessible state language, and a shared truecolor/ANSI-256 Ratatui theme while preserving application behavior.
11. Added a curated Vision Provider Catalog for DashScope/Qwen, OpenAI, Google Gemini, and OpenRouter. Provider selection now fills the compatible endpoint, protocol, key environment, and recommended vision model; custom gateways and unlisted model IDs remain available as explicit fallbacks.

## Problems found and fixed

- A server DTO serialized an `OsStr` platform object, crashing React while rendering project images. It now serializes a lossy string at the HTTP boundary.
- A real compatible model used pixel/xyxy coordinates, unrelated tools and task IDs as labels. Tool applicability, schemas and task prompts now enforce normalized typed values and allowed labels.
- Malformed candidate parsing originally terminated a run. Runtime now records the issue, feeds back a precise correction and retries within bounds.
- A real response emitted multiple expensive refinement calls. Runtime executes at most one evidence/refinement call per task and records skipped calls.
- TUI and HTTP previously composed runtimes separately. Both now use the same application service and event stream.
- Browser testing found a stale React closure that reset the selected project after an SSE terminal refresh. `projectId` now uses a functional state update; the selected synthetic project remained selected after a completed seven-request run.
- Provider setup previously exposed transport-level fields as the primary workflow. The Settings page now leads with provider and model selectors, keeps transport and tuning fields under Advanced settings, and warns before replacing a saved key from another provider.

## Real image/provider smoke test

- One user-authorized RoboCup frame was copied over SSH into the ignored workspace. It decoded as a 544×448 RGB PNG with SHA-256 `b0b470a93ed334a52462316029d1cf8794ce4287eaba37a2f551af3848043cfd`.
- A Qwen-compatible endpoint/model was invoked with the key injected silently into a process; the key was not written to config, SQLite, source, or command output.
- Verified: authentication, vision upload, tool calls, task scoping, structured candidate retry, usage/events, SQLite persistence, and review decisions for scene and field-region tasks.
- The field-line remote request became slow and the run was cancelled safely. The six-task real-provider DAG therefore did not complete. The last reported run totals were 9,926 input tokens, 30,148 output tokens and four recorded requests; configured pricing was zero/unknown for this smoke test.

## Commands and observed results

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- `cargo test --workspace --all-features` — passed: 43 Rust tests, 0 failed.
- `cargo build --workspace --all-features` — passed.
- `npm --prefix web run typecheck` — passed.
- `npm --prefix web test -- --run` — passed: 1 test file, 1 test.
- `npm --prefix web run build` — passed; Vite production assets generated.
- `cargo run -p annotagent -- doctor` — passed; config, writable workspace, 17 SQLite tables, migration, Skill, key presence check, example, Web build and port check reported.
- Project validation — passed for six tasks.
- Historical pre-Workflow-Alpha `demo robocup` — completed with 7 committed annotations, 0 reviews, 2 recorded retry issues, 1,260 input / 315 output Mock tokens, 7 requests, cost `0.00252`. The current release commands are `demo generic-workflow` and `demo robocup-hybrid`; see `docs/DEMO.md`.
- Mock run without `--limit` — completed through `DatasetCoordinator` with the same per-image result.
- History show/export/import — succeeded; import remapped conflicting IDs and emitted a missing-image warning.
- COCO export — exported 5 annotations and explicitly skipped 2 incompatible values with warnings.
- TUI — entered the alternate-screen UI and exited cleanly with `q`.
- Server — started on loopback; health, Skills and Projects APIs returned valid JSON.
- In-app browser — Dashboard, Project, Review, Skills and Settings rendered; the synthetic project run streamed SSE to `run completed` with 7 requests and cost `0.00252`; project selection remained stable after the stale-closure fix.

The generated databases, live frame, Web build directory, node modules and temporary export/history files remain ignored or outside the repository.

## 2026-08-27 product hierarchy refactor

- Recast AnnotAgent as the global product shell and scoped RoboCup to a registered Skill, example Project, domain visual profile, and retained example lockups.
- Added the product entity/LLM boundary design in `docs/PRODUCT_HIERARCHY.md`; added honest compatibility DTOs for Projects, Workflows, Model Bindings, Skills, and Run summaries without claiming arbitrary graph execution.
- Rebuilt Web information architecture around Dashboard, Projects, Workflows, Models, Skills, Runs, Review, and Settings. The Workflow page renders the actual configured task graph and keeps suggestion/edit/dry-run/publish controls disabled with explicit limitations.
- Split Vite assets into `brand/core` and `brand/skills/robocup`, removed domain identity from HTML/PWA metadata and the Core OG card, and synchronized the canonical visual-system checksum manifest.
- Isolated the domain mapping behind `SkillVisualProfile`; Project, stable Skill-id, schema, and stable-hash fallback priority is covered by tests.
- Made the TUI Project optional, gave its no-Project state a generic product title, and loaded Project/Workflow/Skill context dynamically after `--project` or `/open`.
- Rust formatting, Clippy with warnings denied, all 49 Rust tests, all-feature build, Web typecheck, 12 Web tests, and production build passed.
- In-app browser acceptance used a disposable workspace: the no-Project Dashboard contained no domain wording or badge; all eight navigation entries rendered; an isolated example Project showed its Skill, active Workflow, six real nodes, model binding, dependencies, validators, fallback, and review gates. Roadmap actions were visibly disabled. The browser tab, server, SQLite database, and temporary Project were removed afterward.
