# Provider Registry + Pipeline Builder Alpha — Known Limitations

This file records observed baseline limitations and will be narrowed as milestones land.

## Baseline limitations

- The compatibility singleton runtime configuration remains under Storage until M8. Pipeline
  Builder `advisor=llm` now resolves its own Registry Provider/Profile, but existing annotation Runs
  do not all resolve typed Profile bindings yet.
- Native Keyring calls are live-conditional on an unlocked desktop credential service; CI covers
  the same contract through an injected backend and in-memory implementation.
- The legacy workspace credential file is still readable to avoid breaking existing users. The M8
  migration UI must make copy-to-Keyring and optional source deletion separate explicit actions.
- Model Profile, Project Binding, Agent/default Binding and revision tables exist, but existing
  singleton Projects are not migrated until M8.
- Provider model discovery only proves `/models` compatibility and returns IDs. It cannot verify
  modality, task capability, structured output, tools or pricing without an explicit declaration or
  active model-specific verification.
- `/api/models` remains the legacy Vision Worker/runtime-binding list; revisioned API/VLM profiles
  use `/api/model-profiles` and the separate Models tab.
- New Builder mutations can bind a typed, lock-aware `ModelProfileId`; the legacy runtime string is
  retained only when the Profile remote ID resolves to an existing runtime descriptor. Complete
  Profile-to-runtime resolution for every Provider is M6–M8 work.
- TUI lacks Provider and Binding inspection/mutation commands.
- Active Probe and Pipeline Builder Agent usage persist Provider/Profile revision, tokens, latency
  and configured pricing. Normal annotation Run usage still needs the same Profile/revision and
  pricing-snapshot integration in M7–M8.
- Published Workflow snapshots support frozen Model Profile semantics, but the current publication
  service does not populate them until the M3 lifecycle/API integration resolves typed bindings.
- Resize and Tile currently create typed virtual Image Artifacts and complete coordinate lineage;
  external inference adapters must materialize/consume their virtual blob references before tiled
  remote inference is considered end-to-end release evidence in M6–M8.
- Existing Annotations and generic Segment are registered public contracts, but their complete
  Project-store/template execution paths remain release-open until M6.
- The Builder's passive Provider availability tool reads the persisted sanitized health snapshot;
  it does not perform DNS or HTTP itself because the Application Tool Loop does not own the Server
  Secret Store. The explicit Provider Settings passive-check endpoint remains the network check.
- Builder undo is intentionally scoped to the current Agent session and retains at most 32 prior
  successful mutation snapshots. Durable cross-session recovery remains available through saved
  Draft comparison/clone operations rather than an unbounded hidden undo log.
- `submit_draft_for_human_approval` is the normal terminal action and immediately enters
  `WaitingForHuman`; `finish_agent_session` remains registered for protocol compatibility but is not
  required after a successful submission.
- The real OpenAI-compatible Builder smoke is ignored by default because it is billable and depends
  on external network/provider behavior. It runs only with an explicit billable opt-in and dedicated
  environment variables; offline release evidence uses Scripted Mock.

## Explicitly outside this Alpha

Provider marketplace, unknown-provider auto-registration, vendor-specific runtime adapters, cloud
secret sync, shared team credentials, automatic billing/recharge, arbitrary code/Shell/Python/URL
tools, Agent publication/full-batch start, runtime self-modification, model-weight download,
training platform, plugin marketplace and multi-tenant authorization.
