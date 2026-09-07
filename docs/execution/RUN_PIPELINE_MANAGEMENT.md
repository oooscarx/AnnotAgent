# Run and Pipeline Management

## M0 baseline

Date: 2026-09-07

The repository has durable Runs, Dataset Batches, Workflow Drafts, immutable Published Workflow
Versions, Sample Tests, Builder sessions, Review records, annotations, geometry evidence, usage
records, and execution snapshots. Before this work it has no user-facing lifecycle model for
trash, restore, or permanent cleanup. Workflow Drafts have a one-way `archived` status, while the
Project default is inferred from the latest Published Version rather than stored as an explicit
Project-scoped pointer. Run and Batch lists return every stored row.

This feature extends the existing `SqliteStore`, `LocalApplication`, Axum server, and Web/TUI
surfaces. It does not create a second workflow engine or management database. Acceptance uses only
in-memory stores, temporary workspaces, deterministic fixtures, and browser fixtures. The real
`workspace/.annotagent/history.sqlite` and user records must not be deleted by implementation
tests.

## Reference map

| Object | Current durable references | Lifecycle rule |
| --- | --- | --- |
| Run | `run_images`, `task_runs`, `run_steps`, `run_events`, `model_calls`, `model_messages`, `tool_calls`, `vision_artifacts`, `validation_issues`, `usage_records`, `active_project_runs`, `run_start_requests`, `batch_images.child_run_id` | Soft deletion hides the Run and unresolved Review work. Accepted annotations, revisions, correction evidence, immutable workflow snapshot, and usage ledger remain. Active state and leases block deletion. |
| Dataset Batch | `batch_images`, `batch_events`; child Runs are referenced by `batch_images.child_run_id` | One operation moves the Batch and only its currently visible child Runs to trash. Restore uses the same operation membership and does not revive items deleted earlier. |
| Workflow Draft | Sample Tests by `draft_id`; Builder sessions carry a Draft id in session JSON; Published Versions carry `source_draft_id` | Draft deletion does not delete Published Versions. Active Builder/Sample Test writes block lifecycle changes. Completed Sample Tests follow Draft visibility and remain recoverable. |
| Published Version | Run and Batch frozen snapshots; Pipeline Improvement baseline; plugin/model-bundle reference ledgers | Published content remains immutable. Lifecycle metadata is stored separately and never changes the content hash. Historical Runs keep their frozen snapshot. Active execution references block deletion. |
| Pipeline parent | There is no separate parent table: existing `workflow_id` is the stable parent identity and Draft id is the current publication lineage identity | Add only lifecycle/alias metadata around the existing identity. Whole-Pipeline operations target the parent plus visible Draft/Version children and record exact operation membership. |
| Review | `review_queue` points to annotations and Runs | Unresolved items for a trashed Run leave normal Review queries and cannot accept decisions. Accepted annotations/revisions remain visible independently. |
| Annotation / Revision | Annotation stores source `run_id`; revisions point to annotation | Never cascade from Run management. Permanent Run cleanup retains a minimal source summary for these user-owned records. |
| Artifact / trace / checkpoint | Run-scoped SQL rows plus serialized Batch/Run checkpoints; files may be shared by other evidence | Permanent cleanup removes only unreferenced debug rows/files. File GC is post-commit, path-free at the API boundary, symlink-safe, idempotent, and reports retained/failed items. |
| Calibration / Improvement | Geometry corrections store `run_id`; improvements store Workflow identity; calibration reports store frozen model/node context | Referenced evidence is protected or retained as provenance. No dangling Passed state is created. |
| Usage / cost | `usage_records` and `model_calls` by Run | Historical spend survives soft deletion and permanent cleanup through a compact immutable ledger; visible-list totals and historical spend are distinct. |
| Export | Exported files live under the Project workspace and annotations are the export source | Run/Pipeline management does not remove exports or accepted annotations. |

The only existing unconditional cascade is Batch to `batch_images` and `batch_events`. Management
must not rely on it for soft deletion or provenance cleanup. No API accepts a filesystem path.

M0 commit: `6fb27dd docs(management): record run and pipeline lifecycle baseline`

## Rules

- Normal Delete means move to Project-scoped trash. It does not claim to free disk space.
- Restore preserves stable IDs and never restarts a Run or restores a default Workflow binding.
- Permanent cleanup is an explicit, confirmed trash action backed by a durable idempotent operation.
- `Pending`, `Running`, `Paused`, `CancelRequested`, runtime-resumable Review, live worker leases,
  Builder sessions, Sample Tests, and publication writes are blockers checked again at execution.
- Batch parent/child selections are normalized before preview, counting, and mutation.
- A default Published Version requires an explicit replacement or explicit clearing in the same
  transaction as archive/delete.
- Soft deletion, archive state, aliases, default pointers, operation receipts, Review visibility,
  and revision checks are lifecycle metadata; Published Workflow JSON and hashes are not rewritten.
- At most 100 explicitly identified top-level objects are accepted per request. Cross-Project IDs
  fail atomically. A changed revision/lease invalidates the complete confirmed operation.
- Destructive lifecycle actions are not exposed to the LLM Tool Catalog.

## Milestones

1. M1 — lifecycle schema, typed requests/previews/receipts, Run/Batch trash and restore, active and
   Project ownership protection, storage/application tests.
2. M2 — Draft/Version/Pipeline aliases, archive/default/clone/trash/restore, immutable hash and
   historical snapshot tests.
3. M3 — Project-scoped HTTP API, structured errors, idempotency, concurrency checks, and server
   integration tests.
4. M4 — Runs/Pipelines selection, menus, impact dialog, Trash workspace, undo, navigation, SSE
   invalidation, responsive UI, and Web tests.
5. M5 — explicit permanent cleanup, compact provenance/usage retention, reference-aware SQL/file
   GC, restart recovery, TUI entry points, isolated E2E, and final verification.

## Verification log

No paid Provider is used.

### M1 — Run and Dataset Run lifecycle foundation

- Added migration 0018 with independent lifecycle revisions, trash metadata, operation receipts,
  exact operation membership, explicit workflow defaults, Pipeline parent metadata, entity leases,
  compact usage ledger, and provenance tombstone tables.
- Added typed, bounded management requests, impact previews, structured blockers, confirmation
  tokens, idempotent receipts, Trash entries, and purge-report contracts in Core.
- Added transactional Run and Dataset Run trash/restore. Project ownership, lifecycle revision,
  active status, durable active-Run markers, Batch leases, and active children are rechecked at
  execution. Parent/child selections are deduplicated. Batch restore revives only child Runs moved
  by the same Batch operation.
- Normal Run, Dataset Run, active-execution, and Review queries exclude trashed records. Direct Run
  lookup remains available for a Trash-aware deep link. Accepted annotations and correction
  evidence are counted as retained and are not mutated by soft deletion.
- Added one Project-scoped HTTP action family for preview, execute, Trash listing, and durable
  receipt lookup. Existing local-session/CSRF middleware protects these write routes; no Agent tool
  exposes them.

Verification:

```text
cargo test -p annotagent-storage --lib
24 passed (before the Batch membership regression was added)

cargo test -p annotagent-storage management --lib
3 passed

cargo check -p annotagent-application
passed

cargo check -p annotagent-server
passed

cargo test -p annotagent-server management_http_preview --lib
1 passed
```

M1 intentionally does not yet expose permanent cleanup or Pipeline lifecycle actions. Those typed
actions return explicit unsupported blockers until their later milestones are committed.

Commit: `4dc9b7c feat(management): add run and batch trash lifecycle`

### M2 — Pipeline identity, defaults, archive, alias, and lifecycle

- Added a Draft lifecycle revision that is separate from authoring revision and content hash.
  `workflow_id` remains the stable Pipeline identity; `workflow_pipelines` stores only alias and
  lifecycle metadata.
- New Drafts and publications transactionally ensure their Pipeline parent. Publication now writes
  an explicit Project default pointer, preserving the existing activate-on-publish behavior without
  inferring “latest” later. Project reads use only that explicit pointer and never silently promote
  another Version.
- Added Pipeline lifecycle inventory with Draft/Version membership, aliases, lifecycle revisions,
  default state, immutable hashes, and historical Run reference counts. Normal selectors exclude
  archived and trashed parents/children.
- Added Draft, immutable Version, and whole-Pipeline trash/restore; Version/Pipeline archive and
  unarchive; Pipeline alias rename; and Version set/clear-default actions to the shared management
  transaction. Deleting/archiving a current default requires an explicit valid replacement or an
  explicit clear. Whole-Pipeline restore uses deletion-operation membership and never revives a
  child deleted earlier.
- Active Builder sessions, Sample Tests, publication leases, active Runs, active Dataset Runs, and
  lifecycle revisions are blockers. Sample Test and publication entry points now own durable,
  expiring leases. Deleted/archived Published Versions are rejected for new Runs while historical
  snapshots and clone/inspection paths remain readable.
- The existing clone-Version operation remains the single “copy as new Draft” path. Legacy Draft
  archive now writes lifecycle metadata and leaves Draft execution content/hash untouched.

Verification:

```text
cargo check -p annotagent-server
passed

cargo test -p annotagent-storage management --lib
5 passed

cargo test -p annotagent-server pipeline_management_http --lib
1 passed

cargo test -p annotagent-application workflow_alpha_editor_journey_is_persistent_and_version_explicit --lib
1 passed
```

Commit: `ff22cac feat(management): manage pipeline lifecycle and defaults`

### M3 — Permanent cleanup, cancellation handoff, provenance, and usage

- Explicit cleanup now deletes eligible Run traces, events, model messages/calls, tool calls,
  validation rows, unresolved Review candidates, unreferenced SQL Artifacts, and the Run record in
  one lifecycle transaction. It is not another soft-delete flag.
- Accepted annotations, human revisions, correction evidence, original Project images, exports,
  model/plugin assets, credentials, and referenced geometry Artifacts remain. A compact source
  tombstone preserves Run identity, frozen Workflow identity/hash, provider/model identity, dates,
  and the retained annotation count without keeping prompts, source image bytes, or full responses.
- Historical token/cost totals move once into an idempotent compact ledger before Run usage detail
  is removed. The API reports visible-Run usage, cleaned-source usage, and actual historical total
  separately.
- Project export and annotation lookup include retained accepted annotations whose source Run has
  been cleaned. A missing Run deep link can resolve to the read-only retained provenance endpoint.
- Dataset cleanup removes only children carried by that Batch's deletion operation. Individually
  removed children are protected. Pipeline cleanup likewise blocks if independently removed child
  Drafts/Versions would otherwise become unreachable.
- `cancel_and_delete` persists a `waiting_for_cancellation` receipt, uses the existing Run/Batch
  cancel path, then rechecks terminal state, current lifecycle revision, child state, in-process
  control state, and unexpired lease before moving anything to Trash. Timeout/failure stores a
  failed receipt and leaves all selected entities undeleted.
- Batch lifecycle checks distinguish a live worker lease from an expired stored lease.

Verification:

```text
cargo test -p annotagent-storage management --lib
7 passed

cargo test -p annotagent-server management_http_preview_trash_restore_and_active_protection_are_project_scoped --lib
1 passed

cargo check -p annotagent-application -p annotagent-server
passed
```

Commit: `6953a6e feat(management): preserve provenance during permanent cleanup`

### M4 — Project-scoped GUI management

- Project Runs adds explicit selection, row menus, Batch-aware grouping, bulk Move to Trash,
  impact preview, durable receipt display, and Undo backed by the Restore API. Selection clears
  when the Project or filter changes.
- Run and Dataset Run detail pages expose Delete or Cancel and Delete according to real lifecycle
  state. A trashed deep link renders its state and Restore action instead of failing or silently
  selecting another entity.
- Project Automation now includes a compact Pipelines and Versions manager. Users can select
  parents, Drafts, or Published Versions; rename a parent display alias; archive/unarchive; copy an
  immutable Version as a new Draft; set/clear the Project default; or move a specific Draft,
  Version, or whole Pipeline to Trash. Dirty current Drafts are guarded before deletion.
- The Project Trash route supports type filters, deletion timestamps, operation membership,
  single/bulk Restore, and explicit permanent cleanup. Cleanup requires a second typed `DELETE`
  confirmation and reports rows/files reclaimed versus protected data.
- Default replacement or explicit clearing is selected inside the lifecycle confirmation dialog;
  AnnotAgent never guesses the newest Version. Restore never reinstates a default.
- Run history distinguishes visible usage from historical actual usage. Permanently cleaned Run
  deep links render the retained source summary rather than a generic server error.
- Completed lifecycle writes invalidate Runs, Review, Project, Draft, and Inspector caches. A
  same-origin browser notification synchronizes open AnnotAgent tabs; every tab then reloads
  server truth instead of restoring a local React-only record.
- The Trash route remains under the Project shell and Back/Forward/deep links preserve Project
  scope.

Verification:

```text
npm --prefix web run typecheck
passed

npm --prefix web test
13 files / 64 tests passed

npm --prefix web run build
passed (Vite reports the existing >500 kB chunk-size advisory)
```

Commit: `4d39522 feat(web): add project lifecycle management workspace`

### M5 — TUI confirmation and isolated acceptance coverage

- TUI `/trash [kind]` reads Project Trash through `LocalApplication`.
- `/manage <action> <kind> <id> [version] [--clear-default]` resolves the current lifecycle
  revision and prints the same impact/blockers as the GUI. It makes no mutation until `/confirm`;
  `/discard` clears the pending request. Cleanup output distinguishes SQL rows, files/bytes, and
  retained annotation/usage reasons.
- The TUI test creates a temporary Project/database, verifies that preview changes nothing, then
  confirms the soft delete and observes the durable Trash entry.
- The Playwright acceptance flow uses only its `/tmp/annotagent-guided-e2e-*` workspace. It deletes
  a terminal Run, reloads Trash, restores it, uses the bulk selection path, cleans it permanently,
  verifies accepted-annotation export readiness did not change, and opens the retained provenance
  deep link. No model call is required for lifecycle mutation.

Final acceptance verification:

```text
cargo fmt --all --check
passed

cargo clippy --workspace --all-targets --all-features -- -D warnings
passed

cargo test --workspace --all-features
passed (billable Provider smoke tests and legal-weight/sample-image integrations remain explicit opt-in ignores)

cargo build --workspace --all-features
passed

npm --prefix web run typecheck
passed

npm --prefix web test
13 files / 64 tests passed

npm --prefix web run build
passed (Vite reports the existing >500 kB chunk-size advisory)

npm --prefix web run test:e2e
46 passed
```

The final browser pass also verified that cleanup cannot be invoked without the privileged
confirmation path and that a retained `needs_review` candidate is not promoted to an accepted
export merely because human revision evidence remains. Both findings were fixed before the passing
46-test run.

Commit: `006a2d8 feat(tui): add confirmed lifecycle management commands`

Hardening commits:

- `3d82803 fix(management): enforce confirmed cleanup semantics`
- `8f41ef9 refactor(management): name batch lifecycle metadata`

## User workflow

In a Project, open **Runs**. Use a row's `•••` menu for one Run/Dataset Run, or select up to 100
explicit rows and choose **Delete selected**. Review the impact dialog; a terminal execution can be
moved to Trash, while an active execution must be opened and handled with **Cancel and delete**.
After a successful soft delete, **Undo** performs a real server Restore.

Open **Build → Automation** and scroll to **Pipelines and Versions**. Expand a Pipeline to manage
its editable Draft and immutable Published Versions independently. Whole-Pipeline deletion includes
only its currently visible children; a child removed by another operation keeps its own lifecycle.
Archiving merely removes work from normal selectors. If the target is the current default, choose a
replacement Version or **Clear default Automation** in the preview dialog.

Use **Trash** from Runs or Pipelines and Versions. Restore keeps stable IDs and never reruns work.
**Permanently clean up** is available only in Trash, requires typing `DELETE`, and removes eligible
history/debug detail. It does not delete accepted annotations, human edits, original images,
exports, models, plugins, weights, credentials, or shared/referenced Artifacts. The report is the
source of truth for what was actually reclaimed and retained.

## Known limitations

- The current Runtime stores Run debug Artifacts in SQLite and does not own a separate per-Run blob
  directory. Therefore cleanup can reclaim SQL detail but normally reports zero files/bytes; it
  deliberately does not infer paths from serialized provider/tool output. SQLite file size is not
  claimed to shrink until ordinary database maintenance compacts free pages.
- There is no automatic Trash expiry. Cleanup is always an explicit user action.
- Cross-Project bulk management is not exposed in the first GUI; each operation is scoped to one
  stable Project and at most 100 explicit top-level IDs.
- A cancellation receipt is restart-safe and can be queried/retried with the same idempotency key,
  but no general background-job platform was added. The synchronous local API waits up to ten
  seconds; an unfinished cancellation remains undeleted and reports failure.
- Same-origin open GUI tabs are invalidated immediately after GUI lifecycle actions. Lifecycle
  changes made exclusively through TUI/API are observed on the next server refresh/reconnect; the
  Run SSE protocol was not overloaded with fake Pipeline events.
- Existing Workflow storage uses Draft ID as its `workflow_id`; Copy as Draft intentionally creates
  a new editable Pipeline identity. Published content and historical execution snapshots remain
  immutable.
