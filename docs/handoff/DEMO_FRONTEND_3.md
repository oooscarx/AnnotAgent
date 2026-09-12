# Demo Frontend 3 handoff

## Scope and baseline

- Branch: `codex/demo-frontend-3`
- Shared baseline: `8a7d8007904a00af62f3b179c216b8a3b2732a1d`
- Ownership: Model/Provider/Plugin/Settings domain components plus the new task-scoped usage component. No public `App`, HTTP Adapter, shared API/types, route, global CSS or Rust file was changed.
- `ANNOTAGENT_DEMO_FRONTEND_3_PROMPT.md` is not present in any local branch, remote-tracking tree, worktree or attachment directory. Work followed the user's explicit R3/R6 contract and the frozen backend contract at `0c6ceb6623848241ac2b82dd67cbddd36b43673a`.

## Delivered commits

- `9cf06ed` — task-scoped `TaskUsage` component and service seam.
- `6abb856` — bounded `ModelLimits` and `GenerationDefaults` editor.
- `decd93a` — passive effective-request projection and revision checks.
- `c6a5e11` — task usage aligned with physical-attempt evidence.

## Frontend 1 seams

### Task usage

`TaskUsage({ projectId, taskId, service, compact? })` is exported from
`web/src/agent-ui/TaskUsage.tsx`. The local service contract is:

```ts
getTaskUsage(projectId, taskId, cursor?, signal?) => Promise<TaskUsagePage>
subscribeTaskUsage?(projectId, taskId, onChange) => unsubscribe
```

The concrete HTTP Adapter must resolve the task's owned Conversation ID from the
server read model; the component does not infer ownership from a project name or URL.
It shows every physical attempt, immutable Model Profile and pricing revision,
Provider, input/cached/output Tokens, usage source, Decimal cost, failure, status and
effective request. Probes are rendered separately. Mixed currencies are never added.
Unknown Token/cost evidence is not presented as zero. Preset tasks show
`无本次模型请求` rather than fabricated usage.

### Effective model request

`ModelRequestEvidence` is exported from
`web/src/agent-ui/ModelRequestEvidence.tsx`. Its optional service method is:

```ts
getEffectiveModelRequest(modelProfileId, signal?) => Promise<EffectiveModelRequest>
```

`ModelProfiles` mounts it only for an existing Model Profile. The projection must
match the current model, Provider, Model Profile revision and pricing revision before
it can provide mode options to the editor. This GET is passive. It shows requested
and effective max output, input context budget, temperature, top-p, Provider wire
mapping and pricing snapshot. It does not claim that the remote Provider accepted
the values; actual task-attempt evidence remains necessary.

`reasoning_controls` stays a protocol capability declaration. It never creates a
mode selection. A current revision's configured `supported_reasoning_modes` may be
selected, but remains explicitly “configured” until an actual test receipt verifies
it. Stale options are ignored.

### Model setup return

The baseline already contains `SetupRequest`, `SetupSettingsReturn` and
`modelPreparation`. They distinguish Agent planning models, remote visual Model
Profiles, Plugins, Model Instances and Model Bundles; preserve Project/Conversation/
Task/Draft revisions and allowed models; treat unknown readiness as uncertain; and
recheck on both configure and cancel return. Reads do not install, probe or execute.
No parallel setup implementation was added.

## Backend dependency

Backend issue `DEMO-F3-001` requests passive effective configuration, actual TEST
request evidence and task-attempt usage. `DEMO-F3-002` records mismatches between the
initial contract example and the in-progress Rust DTO. Frontend must integrate only
the backend's committed, tested final DTO. A saved form value alone is not R3
effectiveness evidence.

## Verification

- Focused R3/R6 tests: 14 passed.
- Full Web unit suite: 387 passed.
- Web typecheck: passed.
- Production Web build: passed.
- Paid or user Provider call: not executed.
- Endpoint/credential handling: unchanged; no credential was read or printed.

`codex queue` failed locally with an ENOENT for its packaged executable. Numbered
handoffs were sent to the fixed Frontend 1 and Backend task UUIDs through the Codex
task messaging API instead; no private task database was accessed.

## P0 autonomy addendum

The P0 task supersedes Demo packaging priority. Frontend 3 now exports a compact
`CapabilitySetupCard` and a passive `PreparationContinuation` result. Completing or
cancelling model setup returns to the same Task without a second setup-page relay.
Only an unchanged server snapshot with an explicit
`can_resume_without_authorization=true` is described as `server_continuing`; React
does not dispatch Journey work. Scope changes require the task's one current
approval, while missing readiness remains a concrete blocker.

Backend issue `P0-F3-001` tracks removal of the duplicate Sample approval and the
durable Builder-to-Sample wakeup. Until that backend contract is committed and
integrated, Frontend 3 does not claim A1/A2 are complete.
