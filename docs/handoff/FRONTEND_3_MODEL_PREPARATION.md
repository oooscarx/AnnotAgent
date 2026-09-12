# Frontend 3: task-scoped model preparation handoff

## Scope and baseline

- Branch: `codex/mainline-frontend-3`
- Baseline: `eaeae4cf8ee92bded9afb5577d2c9b70bc112de0`
- Public `App` and `WorkspaceAdapter` were intentionally not changed.
- Rust, migrations, Provider credentials, model files, real workspace data, remotes, and `main` were not changed.
- The requested `design/annotagent-mainline-parallel/{00_AUDIT,01_SHARED_CONTRACT,04_FRONTEND_3}.md` files were not present in this checkout, any registered AnnotAgent worktree, or the fetched remotes at implementation time. The implementation therefore follows the explicit task contract and the existing `agent-ui-v1` HTTP contracts. Those design files must still be reviewed before integration if they become available.

## Frontend 1 integration boundary

`web/src/agent-ui/SetupRequest.tsx` and `web/src/agent-ui/modelPreparation.ts` are the handoff boundary.

1. Pass Frontend 1's G0 `CapabilitySetupRequest` to `setupContextFromCapabilityRequest` with a server-owned `SetupContinuationScope`. The types are structurally compatible without importing App/Adapter. Freeze the exact Conversation, optional Draft revision/hash, authorization fingerprint and existing `allowed_models` model ID plus binding digest. Missing continuation identity fails closed; do not infer it from a display name or route.
2. Persist it with `preserveSetupContext(sessionStorage, context)`.
3. Render `SetupRequest` in the existing task workspace. Pass `createModelPreparationService()` and the existing `BundleInstallerService`; do not create another installer.
4. Route `onOpenSettings` to the returned internal path. `SettingsView` already renders `SetupSettingsReturn`, which keeps the exact setup token visible on Agent, Provider/Model, Plugin, and Model Instance pages.
5. On `setup_outcome=configured|cancelled`, restore the exact context and call `service.recheck`. The result always has `can_resume_without_authorization: false`: reload the Task/Draft and obtain a new operation preview/authorization before any paid or external call. Never automatically dispatch a task merely because a candidate became Ready.
6. Clear the context only after consuming the return. A failed recheck must still allow navigation back to the original Task; the task page is responsible for a fresh server read.

## Behavior delivered

- Agent planning profiles and vision profiles are matched and presented as separate roles, even if one multimodal Registry profile can serve both.
- Provider Model Profile, Plugin code, installed Model Bundle, and Ready Model Instance are separate candidates and states.
- Candidate discovery uses exact input modalities, task capability, and protocol requirements. The server compatibility endpoint is consulted, while structurally matching `unknown` profiles remain visible as uncertain instead of being reported as absent.
- All preparation inspection and recheck calls are GET-only. There is no active probe or Provider inference call.
- Unknown price is displayed as unknown, never as zero or free. It is explicitly scoped to candidate Registry pricing, not global usage.
- The real Bundle installer is embedded only after a user opens it. Its existing license, download confirmation, lost-response recovery, verification, and Model Instance creation flow is reused.
- Return checks Task schema revision, Draft revision/content hash, Agent model preference revision, Project model bindings, and frozen allowed-model scope. It preserves Task and Draft identity and always requires authorization review.
- The G0 Registry revision and compatible model IDs are preserved. The live compatible set is re-read and any change marks the return stale; the Registry revision itself remains opaque until Backend exposes the planned task `capability_readiness` read model.
- Settings now separates Agent/default text roles from visual model roles. Model Profile editing retains the existing `expected_revision` CAS path.

## Existing CAS verification

`ModelProfiles` re-reads the current profile, retains the editor on a revision mismatch, and sends `expected_revision`. The existing server regression `model_profile_patch_cas_conflict_and_legacy_request` proves the competing CAS writer receives `409 model_profile_revision_conflict`. No Rust change was made.

## Backend observations FE3API-001 / ML-005

The task preparation snapshot currently requires multiple independent GETs (Task workspace, Draft, model profiles, compatibility, Providers, Plugins, Instances, Bundles, Project bindings, and Agent preference). The frontend guards ownership and rechecks before return, but the initial snapshot can span concurrent server revisions. A future non-blocking contract should expose an owned, task-scoped read model or snapshot revision covering these inputs. Until then, the integration must treat every return as authorization recheck required and must not auto-execute.

G0 `CapabilitySetupRequest` does not carry Conversation, Draft revision/hash or the frozen authorization/model-scope digest, and there is no server GET-by-request-ID contract resolving those fields. F3 therefore requires a separate frozen `SetupContinuationScope`. ML-005 asks Backend/F1 either to extend the task-owned readiness object or define the server resolver; direct composition without this scope must remain disabled.

## Test boundary

- Unit tests cover role separation, unknown availability, Plugin/Bundle/Instance readiness, exact return validation, unchanged `allowed_models`, Settings token recovery, and Settings scope filtering.
- Typecheck and production build cover component integration.
- The component is not mounted in public `App` by this branch, as required. Frontend 1 owns that integration and task-side `SetupContext` construction.
