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
