# Guided Journey UI — execution record

## Baseline (M0, 2026-09-07)

Base: `main` at `3ff165a`, ten local commits ahead of origin. No AGENTS.md in
the repository or ancestors. Preserve the fourteen already modified screenshots;
backup: `/tmp/annotagent-journey-baseline.lsPzW6`. No push, remote changes, real
workspace mutations, old credentials or paid inference are authorized by this task.

Current source verified: custom typed routes, FocusHeader, CreateProject,
BuildTestPublish, SampleFeedbackEditor, Project Guidance, schema/upload/sample APIs,
Registry and model preparation, immutable publication, Batch, Review and export.
The previous Focus implementation is not the requested journey: its creation page
still combines all decisions; collapsed advanced editors remain in the render tree.

| Current path | Visible decisions / controls | Recovery and problems |
| --- | --- | --- |
| Ready model | Inventory → creation (image, name, goal, label, type) → project → automation → test → sample | Project/Test persist, but local creation files do not; advanced workbench remains the default intermediary. |
| Missing model | Creation → project → automation → embedded full Registry | Same Draft is retained, but users must navigate Provider/Model/Plugin concepts. |
| Existing work | Project → Review → source Run → Review → export | Existing owner checks, management and recycle bin remain; default detail controls expose technical evidence. |

Baseline browser suite runs only on isolated ports 8791/8796 using explicitly
synthetic fixture images and inference. This does not evaluate live model accuracy.
Result: **60/60 passed**, log `/tmp/annotagent-journey-baseline-e2e.log`.
Current-source captures: `guided-journey/before-{create,model-setup,sample,review,export}.png`.
Original screenshot directories were restored byte-for-byte from the baseline backup.

## Screen contracts and implementation order

| Scene | Question / main action | Persisted entities / exit |
| --- | --- | --- |
| Images | Which images? / upload and continue | Existing Project + actual uploads; local selections clearly unsaved; back to Projects. |
| Goal | What output? / generate up to three samples | Project Schema goal, output and all labels; same project image context. |
| Model needed | What connection is missing? / connect compatible service | Existing Registry; return or cancel to saved goal, never automatically charge. |
| Waiting | What is executing? / stop | Existing Agent session / Sample Test, real phases only. |
| Samples | Is this useful? / use this plan | Exact Draft/Test/Image, sandbox-only feedback; direct canvas editing. |
| Confirm | Which images and cost? / confirm processing | Exact tested revision, existing publication + Batch, durable retry boundary required. |
| Results / Review | What remains to inspect? / scoped save | Existing Run/Review, save before advance, formal vs sample scope distinct. |
| Delivery | What can be downloaded? / explicit format export | Existing export readiness/report and real file. |

Advanced automation, bindings, raw evidence, history CRUD and recycle bin remain
on their unique management routes, not hidden inside normal task DOM. New journey
routes are presentation only and must not create a second workflow or runtime.

M1: real Images → Goal → Sample vertical slice first. M2: conditional setup,
bounded authorization, cancellation and repairs. M3: processing, Review, delivery.
M4: advanced isolation, restoration and full regression. Each completed milestone
receives its own local commit; incomplete scope must remain explicitly recorded.

## Known backend boundaries (not solved by layout)

Existing sample execution is a synchronous POST; there is no aggregate durable
sample consent/cancel receipt. Publish and start are separate operations, without a
combined durable idempotency receipt. Sample feedback currently supports existing
bbox correction but not adding or relabeling objects. These require real adapters
and tests, not fake controls or implicit authorization. Real-person usability,
native 200% browser zoom and live-model accuracy have not been tested in this task.

## M1 — first three scenes (implemented vertical slice)

M0 commit: `e5c624d`. M1 uses `/projects?new=1` then stable
`/projects/:id/task/images|goal|samples` routes. The initial page renders only
image selection, real previews, removal of local selections and Continue. It
creates a unique Project and uploads through the existing services. Goal/output
and every explicitly entered category persist in the existing Project Schema,
with an expected-content revision check; complex existing schemas are preserved.

Planning and image testing have separate consent. Planning allows zero image
tests; visual testing checks the exact Draft revision, recipients, image hashes
and saved goal. The guided sample path currently supports Registry-bound label
pipelines only, with at most three images and twelve shared logical adapter
calls. HTTP internal retries are not additional logical calls. Unknown cost is
explicit, not zero. Non-registry/plugin execution remains available in existing
management but is not advertised as bounded guided testing.

The sample route reuses existing terminal projection, image canvas and sandbox
feedback. Technical details, node controls, Inspector, model tables and project
menus are absent from its default render tree. Refresh restores Test/Image IDs
without another POST; leaving a pending request no longer navigates back when
its response arrives. This is not yet durable active-job recovery or cancellation.

Evidence: `guided-journey/{images,goal,sample}-{1440,1280,1024,390}.png`.
All are actual isolated browser pages, using explicitly named TEST model fixtures
and synthetic images, not live-model or geometry-quality validation.

Tests: Rust fmt, all-feature Clippy/build passed; Rust all-feature tests **503
passed, 5 ignored**. Web typecheck/build passed (existing large-chunk warning),
unit **80/80**, targeted creation/focus/i18n **8/8**, ready-model journey **1/1**.
Full browser regression: **58 passed, 1 failed, 2 not run**. Trace identifies the
SAM setup failure as HTTP 429 on privileged confirmation before inference; longer
assertion time does not retry a rejected action. Do not report this suite green.
Original screenshot directories restored byte-for-byte, including user edits.

Remaining release blockers: conditional task model connection (currently missing
model can save goal only); durable active planning/sample recovery and stop;
schema/model changes invalidating prior sample authorization at publication;
atomic authorization scope revalidation; combined durable publish/start receipt;
sample improvement and adding/relabeling geometry; guided processing/review/export.
No claim that M2–M4 or end-to-end user acceptance is complete.

## M2 — connection and planning recovery increment

M1 commit: `ee58cee`. `/projects/:id/task/model` is now a conditional task page,
not an embedded Providers dashboard. Missing-model Continue first saves the goal.
The page offers existing compatible text/tool/structured-response profiles or a
small OpenAI-compatible connection form. It uses the existing Provider, private
workspace-file credential, Model Profile, explicit active probe and Project
binding APIs. No keychain, URL secret, global default overwrite, model download,
installation or implicit inference is introduced. The active text probe checks
connectivity; tool capabilities are still user-declared, not a measured quality
guarantee. Returning or cancelling restores the saved goal/images. A locked
Project planning binding is not overwritten. Partial connection saves remain in
the Registry; typed credentials not yet saved do not survive refresh.

Planning sessions now use the existing stable `session` URL parameter. Refresh
loads that Project's saved session rather than calling suggest again. Unknown or
cross-Project session IDs do not fall back to another task. Real server phases
have concise labels, the existing cancel API is directly exposed for running
planning, and stopped/failed sessions are not automatically treated as successful
sample results. Project-keyed components and mounted-response guards prevent old
responses from navigating into a new Project.

Recovery testing exposed a protocol mismatch: zero authorized Dry Runs were still
required by the Builder's finalization gate. Fixed tool availability, the static
validation next-action hint, finalization guard and prompt for planning-only
sessions. They now finish with an explicitly **untested editable Draft**, without
failed forbidden Dry Run calls. Publication still requires a persisted passing
Sample Test; the E2E explicitly attempts premature publication and checks rejection.
No increase to step budgets and no permanent submit-only state was introduced.

Connection, cancellation-of-setup, persistence and recovered-session browser
tests passed on isolated fixture services. Screens: `model-{1440,390}.png`.
All three focused journey tests passed; the ready-model test checks every Builder
tool succeeds, no image call occurs during planning, and refresh does not replan.
Rust catalog/tool-availability regression also passed. Full regression outcomes
are recorded below after final verification.

This is **not full M2 acceptance**: visual-model repair setup, persistent in-flight
Sample Test cancellation/recovery, crash-safe partial setup receipts and concurrent
binding edits remain. M3 confirmation/processing/review/delivery and M4 complete
default-route isolation remain unimplemented. Existing management routes and APIs
are retained, including their delete/restore/Debug/Replay/export behavior.

## Verification and handoff checkpoint

Final full browser run: **62/62 passed**, log
`/tmp/annotagent-journey-final-e2e.log`. Earlier full-suite failures are retained
above, not erased: the expanded suite exceeded the real 120-mutations/minute
sliding window. Security/HTTP-model tests now wait only for the exact middleware
`mutation_rate_limited` rejection before execution; settings retries reacquire
the 30-second one-use nonce. They do not retry an executed model action or a
Provider error. Production rate limits, auth, CSRF and nonce rules are unchanged.
Final focused rerun on the latest upload/connection changes: **11/11 passed**,
`/tmp/annotagent-journey-final-focused.log`. Original screenshot directories were
again restored byte-for-byte from the baseline backup after E2E completed.

Rust: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace --all-features`, and
`cargo build --workspace --all-features` passed. Tests: **503 passed, 5 ignored**
(including externally supplied real-model weights). Web typecheck, unit **81/81**,
production build and full E2E passed. Production build still reports the existing
large-chunk warning; no loading-performance improvement is claimed.

Default route changes:

| Before | Current implemented entry |
| --- | --- |
| Create combines image/name/goal/category/type | Images only → saved Goal → planning consent → image scope consent → Sample canvas |
| Missing planner requires embedded Registry management | Saved Goal → conditional connection → saved Goal, no automatic inference |
| Inspect sample alongside node/details workbench | Same persisted Test/Image on the default canvas; technical render subtree omitted |
| Refresh loses a pending planner's context | Stable Project/session URL restores persisted phase/outcome without another suggest POST |

The main remaining sample action is still **Confirm this sample**, not the
requested **Use this plan → confirm processing**; this is an explicit M3 gap,
not a claim that the complete Guided Journey has shipped. Structured sandbox
feedback reuses existing bbox correction; omitted-object/region editing and
in-context improvement remain incomplete.

Unique advanced management entrances retained (exit the task via Back to Project):
`/projects/:id/build/data`, `/build/labels`, `/build/pipeline`,
`/projects/:id/runs`, `/review`, `/export`, `/manage/trash`, and the existing
global Settings routes. Run/Pipeline archive/delete/restore, immutable versions,
default selection, full evidence and Replay were not removed. These management
surfaces have not all been redesigned as guided scenes.

Screens exercise 1440×900, 1280×720, 1024×768 and 390×844 (some goal screenshots
use 900px height). Existing keyboard/reduced-motion/reflow tests passed. **Native
200% browser zoom, OS-level Chinese IME/screen-reader verification, live-model
accuracy and real-person usability testing were not executed.** Reflow screenshots
and automated tests do not substitute for these checks.

No push, remote edits, history rewrites or real workspace data writes. The
original fourteen user-modified screenshots remain outside task commits. The
task does not restart the real workspace server; isolated E2E servers are used.

## M2 continuation — durable sample lifecycle (2026-09-07)

Implemented the guided Sample Test lifecycle adapter around the existing sandbox
executor. This is not a new inference engine or a second Workflow representation.

- POST `/api/projects/:project/sample-operations` reserves a durable SQLite receipt
  and returns promptly. Its UUID becomes the persisted Sample Test ID. Identical
  requests read the same receipt; changed scope under the same key is rejected.
  Admission permits one active sample operation per Project and two globally.
- `/task/samples?draft=…&operation=…` restores queued/running/terminal status using
  GET only. Completion opens the existing final-result canvas with the exact Test
  ID. It does not publish or accept formal annotations.
- Stop remains directly visible. It records cancellation, drops local execution,
  and releases the management lease (also on worker unwinding). Already submitted
  remote work can still be billed; the UI states this explicitly.
- Startup marks unfinished receipts interrupted, or completed when their Test
  was already persisted. It never automatically retries a model request.
- A non-secret pending request envelope in sessionStorage supports explicit
  same-key retry when a POST response is lost; mount, refresh and polling do not
  replay it. Server scope validation remains authoritative.
- The original 1–3-image/12-logical-call authorization remains in force. Scope is
  checked before preparation and image execution. Internal preparation may update
  Draft status/revision; later checks retain the captured authorization baseline.
  Fully atomic freezing across concurrent Registry/Schema edits is **not** claimed.

Regression coverage: storage deduplication, concurrency/admission and reopen
recovery; application cancellation before execution with lease release/no Sample
Test write; deterministic HTTP browser fixture for running refresh, duplicate
POST, wrong-owner rejection, stop/refresh and uncertain-request recovery. Fixture
latency is explicitly named `e2e-slow-sample`, not a production model or Live proof.

Two findings from browser testing were fixed:

1. Re-authorizing after cancellation briefly opened the consent scene, then the
   latest-result lookup redirected back to the old canvas. `view=authorize` now
   records this explicit intent; it does not trigger a POST and survives refresh.
2. Full-suite SAM setup hit a real `mutation_rate_limited` 429 on its discovery
   request after a successful settings PUT. Trace evidence identified the exact
   rejected stage. The fixture test now awaits discovery/sample completion and
   retries only that explicit pre-execution rejection. Product rate limits and
   model retry behavior were not relaxed. Its three isolated protocol tests passed;
   the initial full runs recorded 59 passed, one failed and two skipped by serial
   test ordering, rather than being described as successful full runs.

Current Rust checks: fmt, strict all-target/all-feature clippy and all-feature
build pass. Full Rust suite: **507 passed, 5 ignored**, no failures; ignored
cases require explicitly provided real-model assets. Web typecheck, production
build and **82 unit tests** pass. The pre-existing large JS chunk warning remains.
Final full browser suite: **62/62 passed**, including Run/Pipeline management,
restoration, Review, export, security and the corrected SAM fixture test.
The running/stopped scene also reuses Project image previews, explicitly labeled
as inputs rather than model results; it does not manufacture percentages or boxes.
Its final targeted visual/lifecycle check is logged in
`/tmp/annotagent-journey-m2-visual-final.log`: **1/1 passed**, including real
fixture HTTP execution, running refresh, cancellation and lost-POST same-key retry.
The final Web unit rerun remains **82/82 passed**.

Logs: `/tmp/annotagent-journey-m2-final-rust.log`,
`/tmp/annotagent-sample-clippy.log`, `/tmp/annotagent-sample-build.log`,
`/tmp/annotagent-journey-m2-final-unit.log`,
`/tmp/annotagent-journey-m2-final-build-web.log` and
`/tmp/annotagent-journey-m2-verified-e2e.log`.
Screenshots: `guided-journey/sample-running.png`, `sample-stopped.png`, and
`sample-{1440,1280,1024,390}.png`, all deterministic isolated fixtures.

Remaining: M2 partial connection-save recovery and atomic scope freezing; M3
exact-revision adoption/publish/start receipt and guided processing/Review/export;
M4 full default-path and assistive-technology validation. Real-model accuracy,
native 200% zoom, OS IME/screen reader and real-person usability are not tested.

Local handoff: branch `main`; no push, remote edits or real-workspace operations.
All three older screenshot directories were restored byte-for-byte to the saved
pre-task baseline, preserving the user's fourteen existing screenshot changes.
Only the Guided Journey evidence and this phase's source/test files enter the
local milestone commit (`feat(journey): persist and recover bounded sample tasks`).

## M3 in progress — confirmation and bounded Dataset execution

Screen contract: Sample canvas → **Continue with this plan** →
`/projects/:id/task/confirm?draft=…&test=…&image=…`.
Question: what exact images, tested plan and external services will be used?
Necessary inputs: image count and explicit call-budget/data-scope consent.
Success: same-Project canonical Batch detail. Failure: same confirmation receipt;
Back restores the selected sample. No mutation on preview, GET or refresh.

Implemented (verification ongoing):

- Durable processing receipts compose existing publication and DatasetCoordinator;
  receipt UUID is also the Batch UUID. Lost responses and retry cannot create a
  second Batch. Published-but-not-started records retain the published identity.
- Exact Sample Test/revision and current Project/model/image scope checks; new
  bounded samples store their scope seal atomically with the execution receipt.
  Older unsealed samples must be re-tested for this guided confirmation path.
- Approval checks inside the existing publication service prevent a changed Draft
  or model snapshot from being published under an earlier confirmation.
- A persistent pre-request allowance wraps existing external model adapters for
  confirmed Batches, including concurrent workers/retries/restart. Reservations
  are never refunded after failed/uncertain sends. No synthetic price estimate.
- Batch creation freezes authorized images/settings and checks image hashes and
  Project schema before image inference. It does not introduce another Runtime.
- Sample feedback confirmation remains separate from plan adoption and formal
  annotation review. Unsaved sample edits disable adoption.

First isolated browser traversal of sample → confirmation → actual publication
and Batch creation passed. Partial-start failure, full execution and regression
tests are in progress. Guided running/Review/export presentation and the remaining
M2/M4 checks are not yet complete. Goal remains active until the remaining work is
handled; no push or real workspace changes are authorized.

M3 vertical slice verification (2026-09-07):

- Real fixture-backed classification Batch reached `completed`, not merely a
  created record. The external service is the explicitly marked offline E2E
  fixture, not a live model or a claim about ball-detection accuracy.
- Simulated unavailable credential file in `/tmp/annotagent-guided-e2e-*` after
  sample testing: publication succeeded, launch failed, refresh restored the
  receipt, and explicit retry started the same deterministic Batch. Replaying the
  request returned that Batch without re-publication or re-execution.
- Guided Batch results reuse shared result/annotation/image query keys. Only
  terminal projection IDs are drawn; filters and image IDs are URL state.
  Original/results switching, empty failure filter and refresh are covered.
  Pause/Resume/Cancel remain visible during active processing; Resume now resolves
  credentials from the frozen published model registry instead of the old global
  session key. Management remains in the existing Project history/Trash pages.
- Screenshots: `guided-journey/processing-confirm.png`,
  `processing-results-1440.png`, `processing-results-390.png`. Screenshot
  animations are disabled to avoid capturing a transient fade. These are
  synthetic fixture images, not real inference evidence.
- `cargo test --workspace --all-features`, Clippy with warnings denied, workspace
  build, Web typecheck/unit tests (82), production build and the isolated journey
  E2E passed. Full browser regression and Review/export refinements remain next.

M3a local commit: `79b83fd` — confirmed processing and bounded results.

### M3b — continuous Review and delivery

Review contract: inspect this object → save/accept/reject using the existing
Project-owned endpoints → next actual queue item only after success. The last
decision stays on its stable image URL, with an explicit export action. Failure
retains edits. The default item renders direct shape editing, label/reason fields
only while editing, Add/Undo and original/result comparison; no Debug Inspector.
Existing technical evidence, revision history and provenance are still available
from the Review management list via the typed `?view=audit` canonical route.

Export contract: show real readiness, accepted count, unresolved work, compatible
formats and loss warnings → explicit export → persisted real report/output path.
The output path is labeled as server-side, not a browser-local folder. Duplicate
navigation tabs were removed. No install/publish/inference is triggered by entry.
Slow readiness requests and export completions cannot overwrite another Project.

Verification: full 62-test browser suite passed after updating audit-route and
connection-recovery assertions to the new presentation. Further regression adds
an injected HTTP 503 on Review save (fields retained, no acceptance or advance),
last-item canvas after refresh, and export DOM isolation. New screenshots are
`guided-journey/review-default.png`, `review-complete.png`, `export-complete.png`.
These remain explicitly synthetic fixture interactions, not real inference or
human usability testing.

Confirmation now includes the saved Sample Feedback history in its authorization
fingerprint. Feedback changed in another tab requires re-confirmation. The UI
warns that manually corrected samples do not change future model predictions.
Remaining work: task-specific vision setup, sample-feedback-driven controlled
improvement, complete default Run/management isolation and final M4 coverage.

M3b local commit: `99dcfb4` — continuous Review and delivery. Final full browser
run passed **62/62**, including HTTP 503 Review-save recovery, unchanged canvas
after the last decision and refresh, separate advanced audit route, export/SSE
recovery, management deletion/restoration and existing SDK/plugin checks.

### M2 follow-up — distinguish planning from image execution

- Goal entry checks task-compatible available image profiles independently from
  the text planner. Missing vision support goes to the typed `purpose=vision`
  connection scene, then returns to the same saved goal without inference.
- The task connection scene filters by actual declared modality/capability;
  text-only planning and prompted-refiner-only profiles are not presented as
  independent object detectors. Vision setup does not claim tool-call support.
- Successful Provider/model substeps persist in the existing Registry. Only
  their IDs are saved as a local recovery preference; no key enters browser
  storage/URL. A failed model-create step can resume after refresh, reusing the
  saved Provider and workspace-file credential without creating a duplicate.
- Selecting a model now compares and changes one binding inside a storage
  transaction. It does not delete/reinsert the Project's complete binding list;
  locked bindings and concurrent changes are rejected, unrelated bindings stay.
- Isolated planning + vision setup / cancellation / partial-save recovery and
  the ready-model end-to-end processing case passed (2/2). Storage tests passed;
  Clippy warnings-as-errors passed. Current unit additions distinguish image
  classification/detection from text planning and prompted segmentation.
- Limitation still open: this scene provisions OpenAI-compatible services, not
  native model bundles. Existing plugin installation remains in management until
  a task-scoped native setup return path is implemented and verified. This is
  not reported as a completed local-model first-use path.

### Planning confirmation freshness

Guided planning submits the displayed model revision, Provider identity/endpoint
and saved goal revision. The server rejects a stale scope before resolving
credentials or creating the Agent session; this path also requires zero image
dry runs. This is an entry-time validation, not a lock on all subsequent Project
edits. Isolated browser regression tested five rejected scopes, unchanged session
inventory, then the authorized planning → samples → processing path (1/1 passed).
No live Provider was used. Check: `cargo check --workspace` passed.

Planning authorization local commit: `d984162`.

### M3c — feedback, preserved revision and same-image comparison

Sample feedback now leads to a task-scoped revision scene, not the Pipeline
editor. The explicit adjustment action makes an exact-tested-revision copy via
the existing Draft storage; its original Draft and immutable Sample Test stay
unchanged. The copy and frozen feedback lineage commit in one SQLite transaction.
The copy UUID is an idempotency key and cannot overwrite another Draft. Subsequent
feedback does not silently change the frozen evidence for an in-progress revision.

The revision scene explains the selected planning destination, the feedback it
receives and bounded planning budget. Explicit authorization calls the existing
Builder in RepairDraft mode with zero image dry runs. Feedback is supplied as
untrusted user evidence, not tool permissions or proof of accuracy. The existing
management lease prevents concurrent repair/testing/publication of this copy;
normal completion/cancellation releases it, startup recovery does not restart it.
Reload only reads the same Draft lineage and Agent session. Stop remains visible.

A ready proposal leads to the existing separately authorized Sample Operation.
Two saved tests can be compared only when stable image ID AND content hash match
and the old test has a terminal projection. Original/result/before views share
the canvas; no generated perfect "after" is substituted. Keep original returns
to its exact Draft/Test/Image context; adopting the candidate uses the existing
confirmation boundary. Neither action accepts future formal annotations.

Verification so far: extended isolated journey E2E passed (1/1), covering feedback
save, copy identity, refresh before/after planning, no implicit image calls,
separate test consent, same-image comparison and unchanged parent Draft. Storage
regression passed for no-feedback, wrong-owner, conflicting-copy, stale-parent,
retry-with-progress and frozen-feedback cases. Web unit tests: 87/87; Clippy with
warnings denied passed. Full browser suite is being rerun. Screenshot:
`guided-journey/sample-comparison.png` (synthetic protocol Fixture, not live model
accuracy or human usability evidence).

Still open: ordinary Run/default legacy Batch presentation, task-native model
provisioning return flow, wider sample editing support and final M4 coverage.
The feedback-driven model can propose an unchanged or unsuitable plan; this
implementation does not claim any quality improvement without evaluation.

Release checks for this increment: full browser suite **62/62**; the final
two journey cases **2/2** after adding a fixture assertion that RepairDraft
actually receives saved feedback AND bounded original outcome summaries. The
summaries contain IDs, labels, status, semantic scores, failure classes and box
coordinates—not image bytes or mask payloads—and are disclosed in the consent.
Full Rust tests **510 passed / 5 ignored** (real-model file requirements), fmt,
all-feature build and Clippy passed. Web typecheck and **87** unit tests passed;
production build was performed by Playwright. No real model or person evaluated
accuracy/usability. The missing-model test now isolates by created model IDs and
matches query-bearing catalog URLs; it no longer depends on suite ordering.

M3c commit: `b67a483`.

### M4a — default Run and Batch results, management retained

Run's default question is now “Check this image’s results”; Batch's is “Your
processing results”. Both render the actual image with terminal projection only,
real status, unverified-boundary warnings and direct supported stop controls.
They do not mount the node timeline, Inspector, metrics dashboard or Project menu.
Back returns to the owning Project. Review and export use the existing APIs and
canonical owner routes. A failed/missing projection does not fabricate a box.

Processing history keeps an explicit Execution details action: Run `view=debug`
and Batch `view=history` reuse the existing professional views. Delete, recovery,
trash, Artifact and Replay implementations remain there, not copied or removed.
Legacy non-trash Batches use the same default results scene as confirmed Batches.
Selected annotation and original-image display are typed URL state for Run and
Batch; filters/image identity survive reload, and canvas selection does not
refocus the page heading. Unknown annotation IDs show an error on the same image.

Verification: Web typecheck and 89 unit tests passed; full browser regression
62/62 passed before the final Batch URL addition. Final suite rerun is recorded
below when complete. Isolated fixtures only. Screenshot `guided-journey/run-results.png`.
The high-confidence warning is presentation, not a change to Core geometry policy.
No real-model or human accuracy/usability validation is claimed. Native 200% zoom,
OS IME and screen-reader validation remain outstanding, alongside task setup
return and wider sample editing. Existing user screenshot modifications are
restored from the pre-test byte backup before committing.

Final M4a regression: **62/62 E2E**, **89/89 unit**, typecheck and production
build passed, including Batch original-view refresh after the final URL changes.
Screenshot visually inspected: actual fixture pixels, no technical success
reason masquerading as a warning, and no offscreen skip-link artifact. The skip
link remains focusable and becomes visible on keyboard focus.

M4a commit: `9f87e67`.

### M4b — changed plans keep honest old-sample context

Found and fixed a real stale deep-link bug: a saved test with `current=false`
was discarded, so its valid image link misleadingly reported “image unavailable”.
The guided view now retains that exact immutable test as a read-only reference,
shows “Sample Test is out of date”, and does not mount sample acceptance/adoption
or correction actions. The original image is resolved by stable ID AND hash.
Previous/next stay within the same test. “Review new sample scope” opens existing
authorization for the current Draft; it does not run a model or reuse old consent.
The management test page retains its existing out-of-date behavior.

Isolated live-server/browser regression edits the tested working copy through
the existing revision-checked PATCH API, reloads its exact test/image URL,
verifies the retained image and absent adoption action, then checks fresh consent
is unchecked and model-request count unchanged. Original parent Draft remains
unchanged. The extended journey passed; typecheck and 89 unit tests passed.
Screenshot: `guided-journey/sample-outdated.png`, synthetic Fixture only. Final
full-suite and broader multi-tab freshness checks remain for the final regression.
