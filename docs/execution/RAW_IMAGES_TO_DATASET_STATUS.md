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

### M1 admission guards and selected samples

UI commit: `f5aca7a`. Added server/Application intake checks before Schema/Builder planning admission: an opted-in delivery Task with missing slots, unsupported target or stale/removed selected images cannot proceed. Non-delivery Tasks retain their existing behavior. Extended the real HTTP regression to prove missing slots block Schema preview before a model call is recorded. Existing journey data-scope regression passed. Automatic journey samples now filter by the saved selected image IDs before applying the existing three-image maximum; the new sample selection regression proves that selecting later images does not silently sample the first Project images.

This is not yet a frozen execution contract: delivery revision/context must still be bound to the actual Schema/Builder request and authorization, including concurrent edits and old receipts. The full task→Schema→Workflow→Exporter→validator capability chain is also pending. Do not claim a complete delivery execution based on these admission checks alone. No paid provider calls were made.

## M2 — real ZIP generator (library slice, not end-to-end delivery)

Admission-guard commit: `e10deda`. Added a strict `annotagent-export::training_package` writer distinct from permissive flat annotation exports. It takes a frozen typed intent, revision, exact source-image set, accepted annotations and explicit image-confirmation/exclusion references. Callers must resolve those references from trusted owned server records; the library does not establish that a supplied reference represents a real human action.

Implemented: stable-ID image/label filenames; explicit contiguous class mapping from ordered stable label IDs; normalized center bbox serialization; reject unresolved/incomplete/failed images and unaccepted objects; confirmed negatives produce empty labels; explicit exclusions include reasons; original PNG/JPEG streaming copy with exact source SHA checking; EXIF-transform blocking; deterministic seeded grouping of exact duplicates/known groups; preserve existing splits or reject contradictions; nonempty train/val; block val-only classes absent from train; path-independent data.yaml without `path`; README; manifest, split manifest, exclusion report and scoped validation report. The ZIP is written to a new partial file, reopened for file-hash/path/pair checks, then atomically hard-linked to a new destination without overwrite. Final ZIP SHA stays in the receipt, outside the manifest hash graph. Original paths/credentials are not serialized.

Actual TEST evidence in `crates/annotagent-export/tests/training_package.rs`: 12 generated synthetic PNGs / 2 classes → 11 included original images, 20 boxes, 1 explicitly confirmed negative and 1 explicit exclusion. Tests compare every included image's exact bytes, empty negative labels, stable class index and the requested normalized 0.2 coordinates, ZIP checksum, and identical ZIP bytes after reversing source enumeration. Extracted into two temporary directories including `含 空格的数据包`; relative train/val resolve to nonempty directories using the absolute extraction root. This is an independent path-resolution check, **not** execution of Ultralytics `check_det_dataset`.

Adversarial cases: unresolved image, corrupted original file, conflicting known-group splits and object-only acceptance block publication. Export tests passed; all 3 new package tests passed; exporter all-target Clippy passed. These temporary TEST artifacts are not a user download and do not demonstrate live model accuracy.

Still required: independent validator must additionally re-parse label geometry/class IDs, decode extracted images, recheck source/group semantics and YAML portability rather than rely on writer preflight; official loader smoke; model/workflow/annotation revision lineage; rare-class warnings; persisted export job/cancellation/recovery and command/snapshot binding; owned HTTP download; trusted whole-image review receipts and partial-scope authorization; browser E2E. No package-ready UI is enabled by this library slice.

### M2 independent semantic validation

Generator commit: `c80f337`. Added `training_package_validation.rs`, which reopens ZIP payloads independently of writer preflight. It checks the intent hash and exact image scope, YAML whitelist and frozen class order, stable paired paths, source hashes, explicit whole-image states and exclusions, unique annotation references/row counts, decoded original dimensions/orientation/pixels, five-field normalized boxes with finite positive geometry, train/val nonemptiness, no class present only outside train, preservation of existing splits and exact-content/known-group isolation, and consistency of split/exclusion reports. Only expected dataset files are accepted; directory/symlink/unsafe entries are rejected. Image validation streams to temporary files before decoding; metadata is bounded to 32 MiB, YAML to 1 MiB and label files to 16 MiB. These limits fail explicitly.

Adversarial TEST rewrites deliberately recompute all matching hashes: NaN/out-of-image/empty boxes, invalid class IDs, unsafe absolute-path/download YAML, an extra credentials file, cross-split capture group leakage and corrupted PNG pixel bytes are rejected. A no-op rewrite remains valid, so these are not false positives from the rewrite helper. Existing positive package/two-directory extraction checks still pass. A standalone package with no positive training examples is blocked; absent/rare classes receive warnings without inventing coverage.

Checks: all 5 package tests passed after the semantic checks; exporter all-target Clippy passed and `cargo fmt --all --check` passed. Existing exporter format tests were also rerun during this stage. No model or network inference was used.

Still pending: official pinned-loader execution (path checks are not that smoke), trusted persisted confirmation/partial-scope authorization, revision/model/workflow lineage, export job recovery/cancellation/idempotency and owned HTTP download, plus the remaining M1 frozen Builder binding and M3 browser journey. Independent validation establishes structural consistency, not annotation completeness or model accuracy; trusted server review receipts must provide that boundary.

### M2 trusted whole-image confirmation storage

Validator commit: `7223f8b`. Added migration 0062 and server-owned immutable whole-image receipts, attached to existing Task intent revisions. Positive completion, explicit negative confirmation and reasoned exclusion are separate from object review. Confirmation requires an explicit user flag, exact intent revision/hash, per-image review CAS and a server-recomputed original/annotation snapshot hash. Caller-supplied annotation bodies and arbitrary confirmation IDs are not accepted. Positive confirmation selects an owned terminal source Run; accepted objects plus any unresolved candidate block whole-image completion. Empty output and all-rejected objects create no automatic negative receipt. Exact retries preserve the old immutable receipt without making it current after an intent edit.

Three new isolated SQLite regressions cover explicit action, partial candidate review, original byte version change, stale annotation snapshot, cross-owner/image/Run rejection, exclusion reasons, CAS, changed retries, restart persistence and intent revision changes. `cargo test --offline -p annotagent-storage --target-dir /tmp/annotagent-dataset-delivery-target` passed: 173 unit tests plus 18 integration tests. Storage all-target Clippy passed after correcting one test-only lint. No real workspace or service was touched.

Boundary: these are storage commands, not a completed UI workflow. The Application/HTTP layer must still expose the exact source-image review and recheck snapshot freshness for readiness; package creation must resolve these trusted receipts, never accept a client-written confirmation string. Exact source Run selection does not yet establish Schema/Workflow/delivery lineage compatibility. Those checks and HTTP/Canvas integration remain pending.

## Not complete / not executed

The inline three-slot card, typed persistence, HTTP admission, tested ZIP generator and whole-image receipt storage exist, but delivery intent is not yet frozen into Builder/authorization or connected to a server-owned package job/download. Whole-image confirmation UI/HTTP wiring, exact-scope authorization and automatic authorized packaging remain to implement. Browser screenshots, end-to-end HTTP delivery, official loader smoke, full workspace checks and live model quality evaluation have not run. Two-directory extraction is covered only by the Rust TEST package case. No completed user delivery or cost is claimed.
