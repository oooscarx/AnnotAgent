# Agent UI integration — decisions and interface issues

## Frozen inputs (2026-09-09)

- User approved the existing Paper & Graphite UI and explicitly authorized integration.
- Approved frontend delivery: `f0bbbc692904aef9f89392bd4cbb1ba3cd79172a`; screenshot/source baseline within that delivery: `68f2568dc2e9ef3ed3119a0401ec37bb1a2685c5` (same UI pixels/code).
- Backend delivery: `ce46c6f7e52da4a1e2fa216256351d8689d05fa5`.
- Common base: `c41b281b49252d520117029d39611865133798af`.
- Integration branch/worktree: `codex/agent-ui-integration`, `/Users/oscar/Documents/my_workspace/AnnotAgent-integration`.
- Pinned merge: `3ddaa64eef3771dd1512f6a58a273663bacc5678`; no conflicts. Neither source branch moved.
- Original main worktree contains unrelated screenshot modifications/untracked design files; preserved. Backend worktree has in-progress `lib.rs`, HTTP bindings and fixture support; not copied or edited.
- No production workspace, 8787 service, credentials or published versions touched. No push or main merge.

## Communication

UIAPI-000: default `/opt/homebrew/bin/codex queue --help` failed with missing packaged binary (ENOENT). Existing second PATH executable `/Applications/ChatGPT.app/Contents/Resources/codex queue --help` succeeded. No installation/configuration or private database access used.

Communication test and pinned-delivery request queued successfully, message `01a085be-daea-7073-b7b7-22e1ca4a2d03`. Confirmed backend thread UUID **`01a0855e-9c39-7c33-9f18-93e084d14816`**; use this UUID for future messages. No ACK loop. Backend owns all Rust changes.

## Integration sequence

1. Project/task navigation, exact owned thread and images; read-only recovery and failure tests.
2. Safe settings and next-request model CAS.
3. Send receipt → exact preview → approval → saved sample output.
4. Durable stop observation, capability-based resume and explicit queue.
5. Revision-bound human answer and continuation.
6. Processing and export; production entry only after HTTP verification.

Each step is incomplete until independently tested. Fixture behavior is not HTTP evidence. No automatic fallback, unapproved external call, global fee aggregation or speculative resume.
