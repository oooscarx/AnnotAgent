# Provider Registry + Pipeline Builder Alpha — Known Limitations

This file records observed baseline limitations and will be narrowed as milestones land.

## Baseline limitations

- Native Keyring calls are live-conditional on an unlocked desktop credential service; CI covers
  the same contract through an injected backend and in-memory implementation.
- The legacy workspace credential file is still readable to avoid breaking existing users. Registry
  import retains only its opaque reference; any copy to another Secret Store and optional source
  deletion are separate explicit actions.
- Provider model discovery only proves `/models` compatibility and returns IDs. It cannot verify
  modality, task capability, structured output, tools or pricing without an explicit declaration or
  active model-specific verification.
- `/api/models` remains the legacy Vision Worker/runtime-binding list; revisioned API/VLM profiles
  use `/api/model-profiles` and the separate Models tab.
- A Published Workflow can execute different frozen Model Profiles per node when they share one
  Provider connection. A version spanning multiple Provider credentials currently fails closed at
  Run admission instead of applying one credential to another Provider. Per-node multi-Provider
  credential routing remains the next Runtime boundary.
- TUI supports Provider/Profile inspection, passive configuration checks, compatibility queries and
  locked Project role bindings. It intentionally does not accept credentials or perform a
  credential-aware Provider network request; use GUI Settings or an environment-variable reference.
- Active Probe and Pipeline Builder Agent usage persist Provider/Profile revision, tokens, latency
  and configured pricing. Annotation Run history freezes Profile semantics but its aggregate usage
  row still uses the compatibility Provider/model display fields rather than a dedicated revisioned
  price-snapshot column.
- Resize and Tile create typed virtual Image Artifacts with complete coordinate lineage. Core/Mock
  execution is covered; live remote tiled inference still requires an adapter that materializes the
  virtual blob reference for the external Provider.
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
- Inline first-use setup spans Provider, credential reference, Model Profile and connection check as
  separate durable operations. If a later operation fails, the already-created safe Registry
  records remain visible for repair or deletion in Settings rather than being silently rolled back.
- Session-only credentials intentionally disappear when the server process stops. Environment
  references are the non-Keychain persistent choice and require that variable to exist in the
  server process environment.

## Explicitly outside this Alpha

Provider marketplace, unknown-provider auto-registration, vendor-specific runtime adapters, cloud
secret sync, shared team credentials, automatic billing/recharge, arbitrary code/Shell/Python/URL
tools, Agent publication/full-batch start, runtime self-modification, model-weight download,
training platform, plugin marketplace and multi-tenant authorization.
