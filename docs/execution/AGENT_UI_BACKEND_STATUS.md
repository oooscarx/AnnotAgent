# Agent B backend status

BASE c41b281b49252d520117029d39611865133798af
Branch codex/agent-ui-backend; isolated worktree AnnotAgent-backend.

B0: API source inventory complete; bindings/examples/change requests submitted before implementation.
B1: lightweight navigation, exact task thread/snapshot, six-section safe Settings view and budget CAS implemented. Run SSE replay extension implemented. HTTP server lib: 55 passed, 1 pre-existing ignored.
B2: completed necessary default-model freeze fix; reused exact Plan grants, FIFO queue, stop and checkpoint/budget recovery. Added concurrent reservation and real stop Trace; server lib 56 passed, 1 ignored. Added typed ownership/feedback conflict errors and scoped resume actions.
B3: final workspace regression/build/Clippy running after B2. First full pass: 738 passed, 6 ignored. No paid calls, real keys, workspace DB or 8787 used.
