# Raw Images → Training Dataset

## Baseline and boundaries (2026-09-10)

- Active request: three-slot intake and a complete `ultralytics_yolo_detection` delivery preset; bounding boxes only for this new preset. Preserve existing task kinds, services and approved Agent UI.
- Audited main advanced during inspection from `c8923a8` to `0fcfc5d1d5b085583d9506e4ef89c6caa01204d7` due to concurrent work. Main's pre-existing edits were not overwritten.
- Independent branch: `codex/raw-images-to-dataset`, worktree `/Users/oscar/Documents/my_workspace/AnnotAgent-dataset-delivery`, based on the latter commit. No push, remote changes, real workspace changes, paid calls, service restarts or training.
- No repository AGENTS.md or Sites hosting configuration was found during baseline inspection.
- This document is the single execution record for this task. Prior UI migration is not counted as delivery progress.

## M0 — audit and first regression

### Verified reuse and gaps

| Existing code | Verified behavior / required change |
| --- | --- |
| `annotagent-storage/src/conversation_tasks.rs` | Owned persistent Task tied to saved goal message and Schema revision; reuse, no parallel task executor. |
| `annotagent-application/src/conversation_schema.rs` | Authorized bounded Schema proposal; extend input with structured delivery context rather than infer bbox from label names. |
| `annotagent-storage/src/conversation_exports.rs` and server `export_jobs.rs` | Owned persistent export job/event records; retain boundaries and extend snapshot/idempotency scope. |
| `annotagent-application/src/lib.rs::export_project_dataset_with_id` | Reads project-wide readiness/snapshot, exports, packages generated files. Not yet exact Task delivery scope or whole-image completeness. |
| `annotagent-export/src/lib.rs::export_yolo` | Flat labels plus classes.txt only; no images, splits, portable data.yaml or image-level negative evidence. Current categories combine and sort Schema and observed labels; do not reuse this as stable preset class mapping. |
| `annotagent-application/src/export_delivery.rs` | Streaming ZIP of exporter-produced files, confined paths and download digest. Explicitly excludes original images unless exporter produced them. Not a ready training package. |
| `web/src/agent-ui/http.ts` | Real task export cards/download receipts exist. Preserve HttpAdapter boundary; no Fixture fallback. |

### Official format baseline

- Documentation checked: https://docs.ultralytics.com/datasets/detect/ and https://docs.ultralytics.com/reference/data/utils/.
- Pin: Ultralytics **v8.3.0**, peeled source commit **6e43d1e1e5db72afbf686dee6745669bcb124b0a**. The tag object is not the source commit.
- Read pinned `ultralytics/data/utils.py::check_det_dataset`: without `path`, root derives from the YAML filename's parent. The planned package omits `path` and uses relative train/val; verification must pass an absolute YAML filename from an unrelated working directory after extraction.
- The pinned loader calls `check_font`, potentially downloading a font. A real loader smoke must be isolated/network-disabled with installed dependencies; none has been executed yet. Source inspection is not a loader smoke.
- YOLO rows are zero-based class + normalized center x/y/width/height. Core stores normalized top-left boxes already, so no second division by pixel dimensions.

### First implemented fix

Reproduced and fixed silent flat-file overwrite when images have matching stems. Both existing YOLO exporters now reject collisions before creating output, including case-insensitive collisions and reserved `classes.txt`. The new full preset will use stable image IDs, not these legacy stems.

Test evidence: added failing collision regression and observed failure before fix. Final checks: `cargo fmt --all --check` passed after formatting; `cargo test --offline -p annotagent-export --target-dir /tmp/annotagent-dataset-delivery-target` passed (1 unit + 7 integration tests); `cargo clippy --offline -p annotagent-export --all-targets -- -D warnings` passed. Full workspace tests have not run.

## Next implementation stages

- M1: versioned Task delivery request with persistent missing slots, ownership/CAS, stable labels and explicit training target. Connect to real Schema/Builder and same-thread summary/authorization. Do not auto-run on refresh.
- M2: image-level evidence/readiness, deterministic source-image package, frozen manifests/splits, streaming atomic ZIP, independent validator, owned receipt/download and recovery.
- M3: same-chat full delivery with isolated TEST HTTP E2E, real ZIP extraction and two-path portability evidence. Synthetic data does not establish model accuracy.

## M1 — intent/storage foundation (partial; no UI completion claim)

Implemented `annotagent-core/src/dataset_delivery.rs`: the three optional typed slots, owner identities, source-image content evidence and grouping, ordered stable labels, an explicit task/framework/profile revision, and default 80/20 seeded grouping-preserving split policy with whole-image human review. A label name never infers an annotation kind. A segmentation intent remains segmentation and fails the detection-preset eligibility check. Structural completeness is explicitly not a capability check or spending authorization.

Implemented `annotagent-storage/src/task_delivery.rs` and migration 0061: intent revisions attach to existing conversation Tasks, preserve exact structured content and its SHA-256, enforce Project/conversation/Task ownership and revision CAS, and return the original receipt for an identical command retry. Changed retries fail. Replaying an old successful command does not roll the latest saved intent back. Missing slots survive reopening SQLite without reconstructing conversational history. No existing task/Schema/Run is replaced.

Verification: `cargo test --offline -p annotagent-core -p annotagent-storage -p annotagent-export --target-dir /tmp/annotagent-dataset-delivery-target` passed, including existing storage integration suites. `cargo clippy --offline -p annotagent-core -p annotagent-storage -p annotagent-export --all-targets --target-dir /tmp/annotagent-dataset-delivery-target -- -D warnings` passed. `cargo fmt --all --check` passed. Tests use temporary TEST SQLite databases only. New persistence regression separately rerun after adding old-command/current-revision assertions.

M0 local commit: `3d495dc`. M1 foundation follows as a separate local commit. This is not completion of M1: server-resolved scope admission, HTTP endpoints, same-thread intake card, Schema/Builder wiring and capability/authorization gates are next. Storage APIs alone are not exposed as a usable product feature.

### M1 HTTP admission

Foundation commit: `0c8c3f5`. Added Application and server `/api/projects/:project/conversations/:conversation/tasks/:task/delivery-intent` GET/POST. The server derives the stable Project owner and resolves selected image IDs/content digests from the Project image inventory; caller-supplied filesystem paths, hashes and owner fields are not accepted. Empty or foreign image selections fail. GET restores missing slots and reports removed/changed scope images without executing a model. Unsupported training targets remain saved and explicitly blocked from the detection preset. The maximum sample scope is three; this endpoint grants no execution permission.

Real Axum HTTP regression `delivery_http_restores_missing_slots_and_rejects_scope_and_revision_changes` passed using an isolated temporary TEST application: missing-slot restore, save/read identity, exact retry, modified-retry rejection, foreign-image rejection, cross-Project rejection, and unchanged latest state after rejection. This is HTTP handler evidence, not yet browser E2E. Existing split/capture-group metadata discovery, the React intake card, capability chain and Builder/authorization binding are still pending; this endpoint is not declared a finished workflow.

Admission checks: `cargo fmt --all --check` and `cargo clippy --offline -p annotagent-application -p annotagent-server --all-targets --target-dir /tmp/annotagent-dataset-delivery-target -- -D warnings` passed. Targeted server regression passed with the same isolated target directory. User services were not restarted.

### M1 React intake wiring (still partial)

HTTP commit: `d92f218`. Added `DeliveryIntake` through the existing `WorkspaceAdapter` boundary, available inside persisted real conversation Tasks, not a separate page or Fixture success path. The inline card edits image selection, ordered label names and explicit YOLO Detection target; preserves known label IDs/aliases/boundaries, saves via the owned HTTP route, shows server-save/error states, restores on mount using GET only, retains the exact command on unchanged retries, and provides a navigation/unload dirty guard. Inputs remain in the mounted card if the Task becomes active; saving is disabled during execution. Existing active-task stopping controls are unchanged. Unsupported saved task kinds are displayed as unsupported rather than as detection.

Checks: Web typecheck passed; production build passed in this isolated worktree (existing large-chunk warning remains). Web unit suite: 77 files passed, 317 tests passed, 1 todo. The two new label-intent tests prove stable-ID/semantic preservation and no category-to-shape inference. No browser evidence yet; a source-mounted card and unit tests do not prove the full conversation journey. Temporary dependency symlink reused installed dependencies without changing the original node_modules. No user service/build directory was replaced.

Remaining M1 work: natural-language known-slot filling, Schema/Builder context and exact-revision authorization gating, image split/group metadata discovery and the single primary continuation action. Currently saving this card does not yet affect Builder; do not treat the existing generic action buttons as delivery-bound execution. The UI card requires a persisted Task, so the pre-task first-message path also remains to integrate. Full package readiness/download is still unimplemented.

## Not complete / not executed

The inline three-slot card, typed persistence and HTTP admission exist, but delivery intent is not yet wired to Builder or authorization. No complete training ZIP has been generated. Whole-image confirmation, explicit negatives/exclusions, exact-scope authorization and automatic authorized packaging remain to implement. Browser screenshots, end-to-end HTTP delivery, official loader smoke, two-path extraction, full workspace checks and live model quality evaluation have not run. No completed delivery or cost is claimed.
