# Provider Registry + Pipeline Builder Alpha — Decisions

## D001 — introduce persistent profiles beside, not inside, the runtime backend registry

The existing `ModelRegistry` owns executable `VisionModelBackend` instances and runtime descriptors.
`ProviderProfile` and revisioned `ModelProfile` are durable product configuration. They will be new
Core contracts and persistent records, then resolved into the existing runtime Registry. Renaming or
overloading the runtime registry would conflate lifecycle, secrets and execution again.

## D002 — Provider presets are pure data

DashScope, OpenAI, OpenRouter, Gemini-compatible, custom, local and Mock presets prefill adapter,
endpoint and suggested remote model IDs. Runtime dispatch uses `ProviderAdapterKind`, never preset or
vendor string branches.

## D003 — Keyring is the new GUI default; the existing file is legacy-only

The current server writes a singleton key to `.annotagent/credentials/provider-api-key` and moves a
legacy Keychain entry into that file. New writes will instead use `SystemKeyring` unless the user
selects Environment Variable or Session Only. The existing file is represented as
`LegacyWorkspaceFile` and is not silently migrated or deleted.

## D004 — secret resolution belongs behind a credential reference

SQLite, API DTOs, Drafts, Published Versions, Agent Tool results and Run history store only a
`CredentialReference` or `credential_configured` boolean. Provider adapters receive a resolved
secret at the last responsible moment. Secret types do not implement `Display`, `Debug` reveals no
value, and errors use sanitized messages.

## D005 — semantic Profile changes create revisions

Remote model identity, Provider endpoint/adapter snapshot, generation defaults, protocol mode,
image parameters and capabilities create a new semantic revision. Credential rotation and pricing
updates do not. Published Versions freeze the selected semantic revision and the actual price
snapshot used by each call.

## D006 — Provider fallback and Workflow fallback stay separate

Provider routes handle infrastructure failures only. Empty/uncertain/conflicting/domain-invalid
results continue through explicit Workflow Decision/fallback edges. The existing Workflow fallback
field is not reused for Provider routing.

## D007 — compatibility is typed and fail-closed

Model selection filters enabled Provider, configured credential, enabled Profile, health, modality,
task capability and required protocol features before ranking. Locked bindings cannot be changed by
the Agent. Absence produces `unresolved_model_binding`; no vendor/model guess is allowed.

## D008 — keep the proven Lean Agent loop and extend its catalogs

The current audited Tool loop, persistent Draft mutations, Rust validation, sandbox Dry Run,
budgets, cancellation and human-only publication boundary are retained. Provider/Profile inspection,
revision-aware binding, requested undo/comparison/runtime-policy tools and structured errors are
added without exposing arbitrary JSON replacement, code, Shell, Python or URL access.

## D009 — migration separates SQLite atomicity from external secret-store side effects

Database schema and compatibility records migrate in transactions. Keyring migration is an explicit
two-phase user action, because a database rollback cannot roll back an operating-system secret store.

## D010 — Vision Workers remain independent

HTTP Vision Workers keep their version, checkpoint, license, score semantics and health contracts.
They may later materialize compatible inference choices, but they do not share API Provider
credentials or Provider CRUD semantics.

## D011 — compatibility startup prefers Keyring, then legacy, without side effects

Until M3 moves selection to explicit Provider Profiles, the singleton Settings compatibility path
uses a deterministic workspace-scoped Provider ID and Keyring account. Startup checks Keyring first,
then the one registered legacy file path. Both checks are read-only. A new write always targets
Keyring; deleting or migrating a legacy file requires an explicit credential action.

## D012 — Keyring backend errors carry no native diagnostic payload

The injectable Keyring backend deliberately collapses native errors into a zero-data error marker.
The public Secret Store maps that marker to stable operation-specific messages. This prevents an OS
backend, account locator, or accidental native diagnostic from echoing credential material through
API errors while keeping the operation category actionable.

## D013 — Model Profile revision is a semantic sequence, not an edit counter

Revision starts at one and advances only when Provider identity, remote model identity, modalities,
protocol features, task capabilities, limits, or generation defaults change. Display label, status,
enable/lock state, pricing, and credential rotation update metadata without creating a semantic
revision. Skipped and redundant revisions are rejected at the SQLite boundary.

## D014 — Draft binding and runtime descriptor coexist during migration

New Workflow nodes can carry a typed `WorkflowModelBinding` containing a Profile ID and lock state.
The legacy `model_binding` string remains temporarily as the runtime-registry projection so current
Projects keep executing. Publication resolves the typed Profile to a runtime descriptor and freezes
the semantic Profile snapshot; M8 removes reliance on vendor/model guessing, not compatibility data.

## D015 — compatibility returns reasons and never ranks an invalid candidate

Compatibility first removes disabled/unhealthy Providers, missing credentials, unavailable Models,
missing modalities, missing protocol features and missing task capabilities. Ranking happens only
after this filter. An empty result becomes `unresolved model binding`; no Provider name, preset, or
remote model string is used as an implicit choice.

## D016 — passive check and active probe are separate protocols

Passive check uses Mock state or an OpenAI-compatible `GET /models` request and records health but
never generation usage. Active probe requires `confirmed_billable=true`, sends one minimal bounded
generation request, and records its Profile revision, Provider-reported tokens, latency and cost.
Neither operation follows redirects or exposes remote response bodies in errors.

## D017 — Registry deletion is fail-closed and never cascades history

Provider and Model deletion searches Model Profiles, Project bindings, Drafts, published versions,
Run snapshots and probe usage. Any durable reference returns 409 with locations and a rebind/disable
remedy. Delete does not cascade into Workflow, Run or Usage history.

## D018 — preserve the singleton runtime path as visibly labeled compatibility state until M8

The new Settings tabs own durable Provider and Model lifecycle. The existing singleton runtime
settings remain under Storage as compatibility configuration until published Workflow resolution is
cut over in M6–M8. It is not projected into fake Provider/Profile records during M3. Legacy
`/models` continues to reach the independent Vision Worker page.

## D019 — one registry keeps execution compatibility and public authoring as separate maps

`VisionNodeDescriptor` remains the executable operation contract used to validate old Drafts and
immutable Published Versions. `NodeDefinition` is the smaller contract exposed to people and the
Pipeline Builder Agent. A public definition cannot register without a corresponding executable
descriptor, but executable legacy operations do not automatically become authoring nodes. This
preserves old Workflows without leaking Cache, Filter, Map or gate internals back into the product.

## D020 — local image geometry carries identity and a root-image region

Resize, Tile and later Crop-local inference must not correlate fan-out results by array order.
Derived Image Artifacts carry a parent `ArtifactRef`, a stable item identity and an optional
normalized root-image region. Coordinate Projection accepts a single unambiguous image or requires
the DetectionSet to name the source image artifact/item, then performs the affine rectangle mapping.
Missing or ambiguous lineage fails closed instead of emitting plausible but misplaced annotations.
