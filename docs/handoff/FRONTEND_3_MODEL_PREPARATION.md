# Frontend 3: task-scoped model preparation handoff

## Scope and baseline

- Branch: `codex/mainline-frontend-3`
- Mainline common baseline: `d3220cb`; this branch's pre-ML-012 parent is `11a49dfe1378a6fea57211624e7e78caefc315ce`.
- Public `App` and `WorkspaceAdapter` were intentionally not changed.
- Rust, migrations, Provider credentials, model files, real workspace data, remotes, and `main` were not changed.
- The requested `design/annotagent-mainline-parallel/{00_AUDIT,01_SHARED_CONTRACT,04_FRONTEND_3}.md` files were not present in this checkout, any registered AnnotAgent worktree, or the fetched remotes at implementation time. The implementation therefore follows the explicit task contract and the existing `agent-ui-v1` HTTP contracts. Those design files must still be reviewed before integration if they become available.

## Frontend 1 integration boundary

`web/src/agent-ui/SetupRequest.tsx` and `web/src/agent-ui/modelPreparation.ts` are the handoff boundary.

1. Read Backend B4 `capability_readiness` from the task workspace or passive `GET .../capability-readiness`, select the exact server-owned item from `setup_requests[]`, then pass that item and the read model to `setupContextFromReadiness(request, readiness, createdAt)`. Do not supply a UI-inferred target, input type, purpose, or compatible-ID set. The helper verifies that the complete request belongs to the snapshot, then freezes the server-owned Conversation, optional Draft revision/hash, authorization digest and existing `allowed_models`. Missing or stale identity fails closed; do not infer it from a display name or route. `setupContextFromCapabilityRequest` remains the lower-level bridge for a separately server-owned `SetupContinuationScope`.
2. Persist it with `preserveSetupContext(sessionStorage, context)`.
3. Render `SetupRequest` in the existing task workspace. Pass `createModelPreparationService()` and the existing `BundleInstallerService`; do not create another installer.
4. Route `onOpenSettings` to the returned internal path. `SettingsView` already renders `SetupSettingsReturn`, which keeps the exact setup token visible on Agent, Provider/Model, Plugin, and Model Instance pages.
5. On `setup_outcome=configured|cancelled`, restore the exact context and call `service.recheck`. The result always has `can_resume_without_authorization: false`: reload the Task/Draft and obtain a new operation preview/authorization before any paid or external call. Never automatically dispatch a task merely because a candidate became Ready.
6. Clear the context only after consuming the return. A failed recheck must still allow navigation back to the original Task; the task page is responsible for a fresh server read.

## Behavior delivered

- Agent planning profiles and vision profiles are matched and presented as separate roles, even if one multimodal Registry profile can serve both.
- Provider Model Profile, Plugin code, installed Model Bundle, and Ready Model Instance are separate candidates and states. Within a server capability group they are alternatives: one Ready candidate satisfies that capability; the UI never presents all candidate kinds as mandatory.
- Candidate discovery uses exact input modalities, task capability, and protocol requirements. The server compatibility endpoint is consulted, while structurally matching `unknown` profiles remain visible as uncertain instead of being reported as absent.
- All preparation inspection and recheck calls are GET-only. There is no active probe or Provider inference call.
- Unknown price is displayed as unknown, never as zero or free. It is explicitly scoped to candidate Registry pricing, not global usage.
- The real Bundle installer is embedded only after a user opens it. Its existing license, download confirmation, lost-response recovery, verification, and Model Instance creation flow is reused.
- Return checks Task schema revision, Draft revision/content hash, Agent model preference revision, Project model bindings, and frozen allowed-model scope. It preserves Task and Draft identity and always requires authorization review.
- The G0 Registry revision and compatible model IDs are preserved. Backend B4 `mainline-capability-v1` is authoritative for ownership, readiness, production eligibility, TEST status, Registry digest, current Agent preference, authorization and task cost. Supplemental Registry/Plugin/Bundle reads provide display and installer details only. Any read-model change marks the return stale.
- Settings now separates Agent/default text roles from visual model roles. Model Profile editing retains the existing `expected_revision` CAS path.

## Existing CAS verification

`ModelProfiles` re-reads the current profile, retains the editor on a revision mismatch, and sends `expected_revision`. The existing server regression `model_profile_patch_cas_conflict_and_legacy_request` proves the competing CAS writer receives `409 model_profile_revision_conflict`. No Rust change was made.

## Backend observations FE3API-001 / ML-005 / ML-010 / ML-012 / ML-015

FE3API-001/ML-005 identified that the earlier UI had to assemble ownership, Registry and authorization truth from independent reads. Backend ML-010 commit `3d5b4429db4963a5d2ee81fabec67f2e979dde4b` resolves this with the passive, server-composed `mainline-capability-v1` snapshot. F3 consumes that object without importing Backend code. It still treats every setup return as authorization recheck only, because the server explicitly reports `can_resume_without_authorization=false` and `auto_expands_allowed_models=false`.

ML-012 identified that Backend commit `58e18ad6f6e27059d4b2e78345b057a2b90bd126` intentionally emits capability-only `setup_requests[]`: the server does not and should not choose a single Provider/Plugin/Instance target for the user. The F3 seam now derives one display requirement per declared capability and maps only the server's `compatible_model_ids` to alternative setup choices. The chosen candidate ID is carried into Settings, while the candidate's server `setup.api_url` remains visible provenance; no setup choice expands authorization or resumes work.

ML-015 Backend commit `07925834bf401a411e94245fa545f1b7640a4924` narrows the pre-Draft request to `role: task_planning` and `required_capabilities: [text_generation]`. F3 consumes that request literally and displays `visual_readiness_boundary` separately. A bbox output request is not converted into an `object_detection` setup requirement: exact visual capabilities and bindings are deferred to the frozen Draft's Builder/Sample previews.

## Test boundary

- Unit tests cover role separation, unknown availability, mixed Provider/Plugin/Instance alternatives, one-of readiness, exact server request ownership, unchanged `allowed_models`, candidate-aware Settings return, and Settings scope filtering.
- Typecheck and production build cover component integration.
- The component is not mounted in public `App` by this branch, as required. Frontend 1 owns that integration and task-side `SetupContext` construction.
