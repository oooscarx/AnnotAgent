# Provider Registry + Pipeline Builder Alpha — Known Limitations

This file records observed baseline limitations and will be narrowed as milestones land.

## Baseline limitations

- One workspace has only one OpenAI-compatible Provider configuration and one model string.
- Multiple accounts for the same vendor cannot be represented.
- A GUI-entered key is written to a plaintext workspace-private file; Keyring is treated as legacy
  and automatically emptied into that file.
- Provider and Model are combined in Settings and in several run/history string fields.
- No persistent Provider, Model Profile, Project Binding or Agent Binding tables exist.
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
- Existing Node Registry is richer than the intended public Alpha catalog and still exposes some
  technical identities that need guided convergence.

## Explicitly outside this Alpha

Provider marketplace, unknown-provider auto-registration, vendor-specific runtime adapters, cloud
secret sync, shared team credentials, automatic billing/recharge, arbitrary code/Shell/Python/URL
tools, Agent publication/full-batch start, runtime self-modification, model-weight download,
training platform, plugin marketplace and multi-tenant authorization.
