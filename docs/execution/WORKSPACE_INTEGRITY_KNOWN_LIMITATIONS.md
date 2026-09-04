# Workspace Integrity Known Limitations

This file describes the current product, not the target state.

- Security tokens are process-local by design; after a server restart, the Web client transparently establishes a new local session before its next mutation.
- The current plugin package format can prove archive integrity but has no trusted-publisher signature variant. Consequently, native packages are inspect-only in the Web UI and require the explicit CLI trust flow to install.
- Legacy Runs whose Project was deleted or whose old display name is ambiguous remain explicit orphans until a future user-directed owner-resolution flow is added; they are never silently attached by name.
- Project-scoped Runs, Batch detail, and Review now have canonical nested routes. Older global detail aliases depend on the dashboard's current ownership summary to resolve their canonical owner.
- Route resources now have cancellation, request generations, deduplication, and precise Run-event invalidation. Agent sessions still require bounded exponential polling because the SSE protocol does not yet publish Agent-session events.
- Workflow Draft writes use optimistic concurrency and Sample Tests are immutable, exact-revision evidence. Automatic field-level merging is intentionally not attempted; a conflict can be compared, reloaded, or preserved as a new Draft.
- Results is now an explicit final-Annotation projection and Debug retains intermediate Artifacts. Legacy Runs without one authoritative stable Image relation fail closed rather than guessing an image.
- Review scope, local draft isolation, score provenance, revision history, and decision navigation are covered. The App-level navigation boundary and browser-close handling guard unsaved Review edits.
- Free-form technical graph mutation remains intentionally unavailable; the technical graph is a read-only Draft projection until typed ports, cycle prevention, deletion constraints, and undo are implemented together.
- Browser upload/chooser transport is not implemented. The only current manual source field is explicitly an advanced server-local path read by the local AnnotAgent process.
- The largest frontend/server/application implementation files still need further incremental extraction. M9 established tested seams for the Not Found route, owned workspace route registration, and Project summary service; it did not attempt a destabilizing full rewrite.
- List APIs are bounded to at most 100 rows per request. The current browser exposes incremental loading for Run and Review indexes; very large Project inventories currently show the first bounded page in the global dashboard rather than virtualizing every Project.
- The Web production bundle currently warns about a JavaScript chunk above 500 kB (579.03 kB in the M9 build). Route-level code splitting remains future performance work.
- Real provider/model checks are environment-dependent and remain outside offline acceptance until explicitly provisioned.
