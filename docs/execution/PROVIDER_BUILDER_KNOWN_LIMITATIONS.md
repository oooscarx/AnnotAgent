# Provider Registry + Pipeline Builder Alpha — Known Limitations

This file records observed baseline limitations and will be narrowed as milestones land.

## Baseline limitations

- The compatibility Settings screen still exposes one OpenAI-compatible Provider/model selection;
  reusable Provider Profiles are persisted in Core/SQLite but do not receive CRUD UI until M3.
- Multiple same-vendor Provider identities are representable and persistable, but cannot yet be
  created from HTTP/Web/TUI.
- Native Keyring calls are live-conditional on an unlocked desktop credential service; CI covers
  the same contract through an injected backend and in-memory implementation.
- The legacy workspace credential file is still readable to avoid breaking existing users. The M8
  migration UI must make copy-to-Keyring and optional source deletion separate explicit actions.
- Provider and Model remain combined in the current Settings UI and in several legacy run/history
  string fields; M3 will expose the new independent persistent entities.
- Model Profile, Project Binding, Agent/default Binding and revision tables exist, but existing
  singleton Projects are not migrated until M8.
- Model capabilities are runtime descriptors rather than user-visible revisioned profile claims with
  provenance.
- Provider health, credential state, model health and Vision Worker health are not cleanly separated.
- Passive Provider check and billable active probe are not distinct product operations.
- `/api/models` returns a mixed workspace model/Worker/Labs list; Provider/Profile CRUD is absent.
- Settings is a single page with Provider, Detection Workers and budgets rather than the requested
  Providers / Models / Vision Workers / Storage / Usage information architecture.
- Project and Workflow nodes bind string IDs; lock and compatibility hierarchy are absent.
- TUI lacks Provider and Binding inspection/mutation commands.
- Usage does not yet persist Provider Profile ID, Model Profile revision, Agent Session identity,
  image/cached-token counts, usage provenance and per-call pricing snapshot in one record.
- Published Workflow snapshots support frozen Model Profile semantics, but the current publication
  service does not populate them until the M3 lifecycle/API integration resolves typed bindings.
- Existing Node Registry is richer than the intended public Alpha catalog and still exposes some
  technical identities that need guided convergence.

## Explicitly outside this Alpha

Provider marketplace, unknown-provider auto-registration, vendor-specific runtime adapters, cloud
secret sync, shared team credentials, automatic billing/recharge, arbitrary code/Shell/Python/URL
tools, Agent publication/full-batch start, runtime self-modification, model-weight download,
training platform, plugin marketplace and multi-tenant authorization.
