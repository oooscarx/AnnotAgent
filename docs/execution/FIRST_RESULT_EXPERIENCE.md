# First-Result Experience — execution ledger

## Scope and status

Started 2026-09-07. M0 complete (`7bc6aa0`); M1's images/goal → existing Builder/sample → persisted feedback vertical slice delivered. M2–M5 remain open. This is the only ledger for this task. This is not a release-complete claim.
Base commit: `734d1e3`. Fourteen existing acceptance PNG modifications are unrelated and preserved.
No push, remote change, reset, rebase, amend, real-workspace cleanup, or paid inference is authorized
by this implementation. Tests use isolated workspaces and explicitly labelled protocol fixtures.

## M0 baseline (code inspection, before changes)

| Persona | Actual route/actions | Breakpoints |
| --- | --- | --- |
| No model | Home → New project (four-part wizard) → Project → Data → Labels → Automation → Settings | Cannot choose browser images; import accepts a server path only. No same-task model preparation panel. |
| Ready model | Create → Data import → Labels → Automation/Ask → Test → open sample dialog → activate → start full Run → project Batch → Review → Export | First result spans four Build pages. Sample dialog is read-only, feedback is not persisted. Test/start buttons lack a complete authorization receipt. |
| Returning | Home → Project guidance → active Batch/Run, Review, Build or Export | Real combined readiness already exists. Home gives introduction and global metrics equal weight; no focused continue-last-work entry. |

Existing foundations: `App.tsx` BuildWorkspace/BuildTestPublish/SampleAnnotationDialog,
`navigation.ts` canonical object routes, `projectWorkspace.ts` and backend Guidance,
`queryCache.ts`, Registry Settings forms, AnnotationCanvas, persisted sample tests, immutable
publication, idempotent formal runs, project-scoped review revisions, export and lifecycle management.
No new ProjectStage or second Workflow/Provider/Router implementation is planned.

Confirmed missing: browser image upload; sample feedback persistence/editing; first-result
authorization and recoverable publish/start receipt. Unknown until tests: true browser-zoom behavior,
user usability, live-provider cancellation charging. No accuracy change is in scope.

## Planned commits

- M0: failing E2E baseline + interactive, explicitly offline three-stage route prototype.
- M1: images and goal → same Draft/sample → persisted feedback using existing Canvas.
- M2: scoped inference authorization, inline existing model setup, return context.
- M3: recoverable publish/start, batch/review/export continuation and management reachability.
- M4: image-first hierarchy, hints, error guidance, responsive/accessibility refinement.
- M5: regression evidence, screenshots, identity chain, limits and human study protocol.

## Verification and limitations

M0: `npm --prefix web run test:e2e -- first-result.spec.ts` failed first (missing Start with images), then passed (1/1). `npm --prefix web run typecheck` passed. E2E watches all API requests and asserts zero mutations while opening and interacting with the example.
The three-screen prototype uses the existing AnnotationCanvas and an explicitly illustrated asset from `examples/robocup`. Its persistent offline badge and disclosure distinguish authored geometry from inference. Native dialog supplies focus containment/Escape; returning users get existing Projects before collapsed global statistics. Existing canonical creation remains the prototype exit, not a completed first-result journey.

### M1 implemented slice

- Replaced the four-part creation wizard with images + natural-language goal + manually chosen label/output in one form. Project Schema persists `annotation_goal`; no LLM is used for parsing. Internal identity is a stable generated Project ID, independent of the display name.
- Empty goals are omitted from serialization to preserve legacy Project snapshot material; explicit backward-compatibility and goal round-trip regression added.
- Explicit Save creates the Project and an ordinary editable Draft. Removed automatic `suggestWorkflow` from creation. An empty Draft now yields Choose automation, not Fix automation, through the existing server Guidance service.
- Browser PNG/JPEG upload (25 MiB/file), immediate local previews, server-side decode checks, existing content deduplication, scoped temporary staging and cleanup. Existing workspace-local import remains reachable; browser source paths are never treated as server paths.
- Final sample preview reuses AnnotationCanvas with an image-major layout, original-image toggle, target list, bbox coordinates, Undo, and unsaved feedback protection. Non-bbox geometry is honestly read-only; classification/mask feedback is available without pretending unsupported geometry edits work.
- Persisted `sample_feedback_revisions` links exact Sample Test, stable Image ID, final outcome ID and revision sequence. Original predictions remain immutable. Image hash, ownership, final-outcome membership and normalized geometry are validated. Conflicting concurrent edits are rejected; identical revision retries are idempotent. Missing-target feedback can attach to an image without inventing a detection.
- Restored sample previews resolve persisted Image IDs + hashes, not current dataset array positions. Data import/rename cannot silently attach an old result to the wrong image.
- No automated improvement is claimed: feedback does not change Runtime, publish, accept formal annotations, or start a paid rerun.

| Persona | Current verified route | Remaining work |
| --- | --- | --- |
| No model | Home → images/goal form → saved Project + Draft; no Provider call | Same-task model setup and scoped authorization: M2 |
| Ready model | images/goal → Project → existing Automation/explicit Ask → exact Draft Test → final preview → persisted feedback → reload | Three-stage presentation, bounded local repair authorization, combined publish/start receipt: M2–M4 |
| Returning | Home's Project list → Project guidance → existing Run/Review/Export/Data/management | More focused scope-specific continuation: M3 |

Regression evidence so far:

- Red-first upload E2E failed because Choose images did not exist; now passes with zero model POSTs and goal restored after reload.
- `cargo test --workspace --all-targets`: **500 passed, 5 ignored, 0 failed**, 60 test executables. Ignored tests require explicitly supplied external/real-weight conditions; they were not run.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo build --workspace`: passed.
- `cargo fmt --all --check`, Web typecheck/unit/build: passed; **70 Web unit tests**.
- Full Web E2E: **54 passed**. Additional bbox editing/Undo/unsaved-close protection/reload assertions: **35/35 guided journey tests passed**. Final entry/upload/reflow run: **5/5 passed**; empty-entry-only screenshot run: **4/4 passed**. Full-suite load hit the existing 120 writes/minute protection; the test-only HTTP helper now waits at most 45 seconds only for the explicit pre-action `mutation_rate_limited` rejection. Production limits and inference retries are unchanged.
- 1440×900, 1024×768, 390×844 offline entry/editor, reduced motion and Escape/focus return passed. Existing Review keyboard, reflow, ownership, optimistic concurrency, Run lifecycle and export regressions passed. **Real browser 200% zoom was not performed**; the pre-existing test named “200 percent reflow boundary” tests viewport reflow only.

### Screenshots and identity evidence

Actual browser captures from isolated protocol-fixture workspaces (synthetic image and fixture predictions; **not real-model quality evidence**):

- `docs/execution/first-result/entry.png`
- `docs/execution/first-result/sample-feedback.png`
- `docs/execution/first-result/project-review.png`
- `docs/execution/first-result/bbox-feedback.png` (deliberately imperfect fixture prediction; saved human correction, not an accuracy demonstration)

One completed fixture chain from `/tmp/annotagent-guided-e2e-77041`:

| Object | Identity |
| --- | --- |
| Project | `project-284d8483-c001-4b74-bb34-c5a3c9226913` |
| Draft / revision | `8e3bbdaf-35c2-455c-a65a-4e7b93c3623f` / 6 |
| Sample Test | `e9b0513d-789e-4654-a401-3dfc00b441d2` |
| Image | `a9bfdd78-b48a-5df7-b089-b358e1ef6d1e` |
| Sample feedback revision | `71ebdd06-0835-4fc5-bdba-37cd48701e48` |
| Run | `807f5617-38ce-417a-8a3c-845f4c96ac44` (subsequently purged by the existing **isolated** lifecycle test, durable management receipt verified) |
| Export | Project `exports/native/annotagent-native.json`; report confirms 2 exported annotations, 0 skipped; includes accepted human annotation `406f4432-d277-4dca-a526-e69f540fdcf8` |

Exact Review ID chain and a real-model execution/quality study remain M5 evidence gaps. The actual user's `workspace/robocup-ball` was not used for these mutations or model calls.

Five-person usability study has not been performed and cannot be substituted with E2E.
Real external inference is not authorized for this turn; protocol-fixture results must not be
presented as model accuracy. No before/after improvement comparison may be fabricated.

## Next

Next: M2's server-enforced inference scope, accurate cost/budget disclosure, sample selection/cancellation, and inline existing Registry forms. M3 must add a durable publish/start receipt and exact scope; M4 the unified three-stage presentation and complete accessibility; M5 remaining release evidence and five-person study plan. Existing explicit Ask/Test buttons are not yet the new bounded first-result authorization design. The browser vertical evidence currently uses a one-image fixture; a three-distinct-image first-result walkthrough remains required.
