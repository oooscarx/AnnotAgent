# Provider Registry + Pipeline Builder Alpha — Blockers

## Active blockers

None for offline implementation and automated validation.

## Live-conditional dependencies

- Real provider probes require operator-owned credentials and may incur cost. They are never run
  automatically and no credential from conversation history is used.
- Native Keychain verification depends on an available, unlocked desktop credential service. CI
  uses the in-memory implementation; environment and session stores are deterministic everywhere.
- Provider model discovery depends on external `/models` compatibility and network availability.
- Manual browser review is live-conditional; automated Chromium coverage remains release-blocking.

## Blocker handling rule

A live-conditional dependency may prevent only its named live check. It does not pause Registry,
Secret abstraction, API, GUI, TUI, migration, Scripted Mock, security or regression implementation.
