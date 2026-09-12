# Frontend 3 status

## Baseline and ownership

- Worktree: `/Users/oscar/.codex/worktrees/demo-f3/AnnotAgent`
- Branch: `codex/demo-frontend-3`
- Starting HEAD: `c9320bcfeecd182df19cafc8e6c5ee9dac1a67b5`
- Integration baseline announced by Frontend 1: `fbbda66f488dae19063e95f2a8b3061862e06e57`
- Scope: model/Provider/Plugin preparation, Settings, R3 request settings and R6 task usage. Public App, HTTP Adapter, routes, shared public types and Rust remain owned by Frontend 1/Backend.

## P0 autonomy correction

Current source evidence showed two relay points:

1. Model setup rechecked the server, but when Registry or authorization guards changed it remained on the setup screen and required another “return” click.
2. Backend `pending_journey_sample_action` projected a second `requires_confirmation` Sample action even though the saved Journey consent already froze Builder and Sample scope. Backend issue `P0-F3-001` requests a persistent server continuation and authoritative read-model state; Frontend will not replace it with `useEffect` POSTs.

Frontend 3 changes:

- Added `CapabilitySetupCard`, a single task blocker with one setup action and no technical IDs in the default view.
- Setup completion now performs one passive recheck and immediately returns the result to the original Task. It does not execute Builder/Sample, probe a Provider, install a model or change `allowed_models`.
- Added `PreparationContinuation`. `server_continuing` is emitted only when the authoritative server snapshot explicitly says the existing active authorization can resume and every frozen guard is unchanged. Changed scope returns `approval_required`; missing readiness remains `setup_required`.
- Aligned that state to Backend `563f2f7`: only `queued`/`running` with `saved_execution_intent_is_active` can become `server_continuing`. Inconsistent boolean/state pairs are rejected instead of being upgraded locally.
- Reduced the default setup surface: the concrete missing capability and at most three compatible options remain visible; model inventories, cost/version evidence and frozen technical scope are under details.

## Integration seam

Frontend 1 should mount:

```tsx
<CapabilitySetupCard request={request} onOpen={openExistingSetupRequest} />
```

After the existing `SetupRequest` calls `onReturn(result)`:

- `result.continuation.state === "server_continuing"`: reload and display server progress only; do not POST from React.
- `approval_required`: show the task's one current server-issued scope approval.
- `setup_required`: keep the concrete capability blocker.

The Backend read model must own `authorization.can_resume_without_authorization`; a missing or false value can never be upgraded by the frontend.

## Verification

- Focused capability/setup tests: 16 passed.
- Web typecheck: passed.
- Backend durable Journey continuation: delivered at `563f2f7`, pending Frontend 1 integration and joint HTTP A1/A2 evidence.
- Full Web unit/build: passed at the first P0 delivery; focused contract tests and typecheck passed after alignment to `563f2f7`.
- Live HTTP P0 A1/A2: pending after Frontend 1 integration.
- Independent Backend verification of `563f2f7` is currently blocked before Journey tests run: `cargo test -p annotagent-storage conversation_journey --lib` reports seven `ConversationSendInput` test initializers missing `task_images`. Reported as `P0-F3-002`; do not count the backend regression as passed until its follow-up SHA is verified.
- No Provider call, probe, install, credential read, push or real workspace mutation was performed.
