# Provider Registry + Pipeline Builder Alpha — Known Limitations

This file records observed baseline limitations and will be narrowed as milestones land.

## Baseline limitations

- The compatibility singleton runtime configuration remains under Storage until M8. Registry
  Providers and Model Profiles are durable and manageable, but existing Runs do not resolve them yet.
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
- Project and Workflow nodes bind string IDs; lock and compatibility hierarchy are absent.
- TUI lacks Provider and Binding inspection/mutation commands.
- Active Probe usage persists Provider/Profile revision, tokens, latency and configured pricing.
  Normal Run usage still needs the same Profile/revision and pricing-snapshot integration in M6–M8.
- Published Workflow snapshots support frozen Model Profile semantics, but the current publication
  service does not populate them until the M3 lifecycle/API integration resolves typed bindings.
- Existing Node Registry is richer than the intended public Alpha catalog and still exposes some
  technical identities that need guided convergence.

## Explicitly outside this Alpha

Provider marketplace, unknown-provider auto-registration, vendor-specific runtime adapters, cloud
secret sync, shared team credentials, automatic billing/recharge, arbitrary code/Shell/Python/URL
tools, Agent publication/full-batch start, runtime self-modification, model-weight download,
training platform, plugin marketplace and multi-tenant authorization.
