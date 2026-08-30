# Provider Registry + Pipeline Builder Alpha — Master Plan

This plan implements the requirements preserved verbatim in
`PROVIDER_BUILDER_MASTER_PROMPT.md`. It is evidence-driven: an existing type, page, or test counts
only when its actual contract satisfies the new Provider/Model/Secret boundary.

## Product boundary

```text
Provider Profile      connection, protocol, credential reference, account boundary
  └─ Model Profile    selectable remote model, capabilities, limits, defaults, pricing, revision
       └─ Binding      global / Project capability or role / explicit Workflow node selection
            └─ Draft  Registry-bound graph edited by a constrained Pipeline Builder Agent
                 └─ Published Version  immutable resolved model revision and execution contract
```

Skills contribute task knowledge, templates and validation. Runtime Nodes consume typed Artifacts.
Agent Tools inspect Registries and mutate real Drafts. Vision Workers remain a separate HTTP vision
backend registry; they do not become API Providers.

## Verified starting point

- `Settings` stores one `default_provider` plus one `OpenAiCompatibleConfig`; it is not a reusable
  Provider registry.
- Web presets prefill that singleton object and combine provider endpoint with one model string.
- `crates/annotagent-server` defaults new GUI secrets to a plaintext owner-only workspace file and
  automatically moves a legacy Keychain entry into that file. This is the inverse of the new
  requirement and must be replaced without silently moving the existing file secret.
- Core `ModelRegistry` is a runtime Vision Backend registry. It has strong backend/capability and
  secret-reference validation but is not a persistent `ModelProfile` registry with Provider ID,
  revisions, protocol features, pricing provenance or Project/Agent bindings.
- Workflow Drafts, immutable Published Versions, typed ports, Dry Run, Artifact persistence,
  Replay, Review, Batch and Export already exist and must be preserved.
- The Lean Pipeline Builder already has a closed 31-tool catalog, a real OpenAI-compatible multi-
  turn Tool loop, Draft mutations, Rust validation, sandbox Dry Run, audit persistence, budgets,
  cancellation and a human-approval stop. It currently reads the runtime model catalog and lacks
  Provider/Profile discovery, revision-aware bindings and several requested Tool names.
- SQLite migrations stop at version 5 and contain no Provider/Profile/Binding tables.
- HTTP exposes `/api/settings` and `/api/models`, not the requested Provider/Profile CRUD surface.
- TUI exposes `/models` and `/advisor`; Provider and Binding commands are absent.

## Milestones

### M0 — baseline and migration design

- Preserve the master prompt and establish the six execution documents.
- Record Git, source, API, persistence, GUI, TUI and test baselines.
- Define an idempotent, transaction-safe compatibility migration.
- Commit as `docs: establish provider registry and builder baseline`.

### M1 — Provider Profile and Secret Store

- Add Core Provider IDs, Provider Profile, adapter, connection policy, health and structured errors.
- Add async Secret Store with Keyring, environment, session, legacy-file and in-memory backends.
- Store only credential references in SQLite; keep legacy file read-only until explicit migration.
- Add Provider persistence schema and security/unit tests.
- Commit as `feat(provider): add reusable provider profiles and secure credentials`.

### M2 — Model Profile and Binding

- Add revisioned Model Profiles, modalities, protocol features, capabilities, limits, generation
  defaults, pricing provenance and compatibility queries.
- Add global/Project/Agent bindings and explicit resolution order; locked bindings fail closed.
- Persist semantic snapshots without credentials and migrate legacy singleton settings idempotently.
- Commit as `feat(models): add reusable model profiles and capability bindings`.

### M3 — Provider API and GUI

- Implement presets, Provider/Profile CRUD, credential operations, passive check, explicit billable
  active probe, discovery, usage and reference-protected deletion.
- Rebuild Settings information architecture as Providers, Models, Vision Workers, Storage, Usage.
- Add HTTP and browser tests, including secret non-disclosure and 1024 px layout.
- Commit as `feat(settings): manage llm and vlm providers from one registry`.

### M4 — constrained Node Catalog

- Register the Alpha catalog: Image Input, Existing Annotations, Resize, Tile, Crop, Detect,
  Classify, Segment, Select & Map, Coordinate Projection, Attach Result, Combine Evidence,
  Validate, Decision, Human Review and Commit.
- Keep Cache, Replay, Retry, Timeout, Budget, Usage, Checkpoint, Pause/Resume/Cancel and History as
  policies/runtime services, not user-composed nodes.
- Commit as `refactor(workflow): expose a constrained annotation node catalog`.

### M5 — Pipeline Builder Agent Tools

- Expose sanitized Provider/Profile inspection and compatibility tools.
- Complete real persistent Draft mutation, validation, cost estimation, Dry Run inspection, undo,
  comparison and approval submission with explicit permissions and audit.
- Commit as `feat(agent): let the builder inspect providers and edit real drafts`.

### M6 — real LLM Tool loop

- Resolve the Agent's own Model Profile through the Registry.
- Preserve correct Tool Call history while adding Provider/Profile selection, bounded context,
  validation repair, Dry Run revision, stop conditions, cancellation and cost accounting.
- Prove the incompatible text model → compatible VLM → high Review → Crop Classification → human
  approval path with Scripted Mock. Real providers remain live-conditional.
- Commit as `feat(agent): build and revise pipelines through constrained llm tools`.

### M7 — Project Guided UX and TUI

- Add compatible grouped model selectors, lock controls, inline Provider setup, Agent model selector,
  progress and Draft Diff without exposing internal IDs in the default view.
- Add `/providers`, `/models`, `/bindings`, `/bind` and `/advisor cancel` behavior to TUI without
  secret echo.
- Commit as `feat(ui): guide provider selection and agent-built automations`.

### M8 — migration, regression and release

- Exercise legacy Provider/model/default-vision/secret migration, existing Projects and Published
  Versions, Run/Batch/Pause/Resume/Cancel, Artifact/Replay, Review, Export and Usage.
- Add `docs/DEMO_PROVIDER_BUILDER.md`, final acceptance evidence and live-conditional matrix.
- Run every required Rust, Web and E2E command and the secret/boundary scans.
- Commit as `test(release): validate provider registry and pipeline builder alpha`.

## Migration design

1. Migration 6 creates normalized Provider/Profile/Binding/Usage tables in one SQLite transaction.
2. A deterministic legacy fingerprint derived from adapter + endpoint + non-secret account fields
   creates at most one imported Provider Profile; repeated startup uses `INSERT ... ON CONFLICT`.
3. The legacy remote model string creates a revision-1 Model Profile with capability provenance
   `user_declared` or `unknown`; no capability is invented from a vendor name.
4. Each legacy Project `default-vision` binding becomes a Project binding to that Profile. Existing
   Draft/Published JSON is not rewritten; a compatibility resolver maps the old binding at read or
   execution time, while new publication freezes Profile ID and revision.
5. The workspace credential file becomes a `LegacyWorkspaceFile` reference. It is neither copied
   nor deleted automatically. An explicit migrate operation writes to Keyring first, updates the
   reference transactionally, verifies it, and only then offers removal of the old file.
6. Historical Runs, Artifacts, Usage and Published Version snapshots remain immutable. New calls
   append Profile/revision/price-snapshot identity.
7. Every migration is rollback-safe: schema/data work is transactional and external secret-store
   writes are staged and reconciled explicitly rather than hidden in database migration.

## Verification policy

Every milestone updates Status and Acceptance, runs focused tests plus relevant regression checks,
and produces an independent local commit. No push, remote mutation, API key, model weight, reset,
rebase, amend or destructive checkout is permitted.
