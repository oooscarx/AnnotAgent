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
