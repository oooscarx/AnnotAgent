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

## Planned milestones

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

## Known baseline limitations

- No management endpoint or Trash view exists.
- Run and Batch lifecycle metadata does not exist.
- Workflow lifecycle and display aliases are coupled to authoring JSON/status.
- The newest Published Version is silently treated as Project default.
- No durable cleanup operation, provenance tombstone, compact usage ledger, or reference-aware file
  GC exists.
