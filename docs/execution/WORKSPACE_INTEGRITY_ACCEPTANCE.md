# Workspace Integrity Acceptance

Release is blocked until every item below has executable evidence. `[ ]` means not yet accepted; baseline behavior is not inferred as success.

## A. Security

- [x] Permissive CORS is removed.
- [x] Cross-origin pages cannot call state-changing APIs.
- [x] Plugin installation requires a trusted same-origin session and privileged confirmation.
- [x] Unverified native plugins cannot be installed through the ordinary Web UI.
- [x] Credential APIs are protected against CSRF.
- [x] Billable probes require explicit confirmation.
- [x] Global body, concurrency, and SSE limits exist.
- [x] `/api/health` does not disclose absolute paths.

## B. Ownership

- [ ] Run APIs include `project_id`.
- [ ] Normal object association never uses `project_name`.
- [ ] Projects with duplicate names stay isolated.
- [ ] Historical Runs remain openable after a Project rename.
- [ ] Reviews are bound to Project and Run.
- [ ] Images use stable IDs.
- [ ] A foreign Image ID cannot be written into a Run.
- [ ] Import never guesses a Run by name.

## C. Routing

- [ ] Project Runs is a real Project child route.
- [ ] Project Review is a real Project child route.
- [ ] Project Run Detail retains the Project Shell.
- [ ] Project Batch Detail is deep-linkable.
- [ ] Global Runs remains a cross-project index.
- [ ] Global Review remains a cross-project index.
- [ ] Legacy URLs redirect to the real owner.
- [ ] Unknown URLs show Not Found.
- [ ] Back/Forward preserves the correct hierarchy.

## D. State recovery

- [ ] Pipeline refresh preserves the exact Draft.
- [ ] Test refresh preserves the exact Draft and Sample Test.
- [ ] Run refresh preserves Image, View, Node, and Artifact.
- [ ] Review refresh preserves the Item.
- [ ] A stale request cannot overwrite a newly selected Project.
- [ ] SSE reconnection resynchronizes server truth.
- [ ] Local storage never determines object ownership.
- [ ] In-page selection changes do not steal H1 focus.

## E. Run and results

- [ ] Start Dataset Run navigates directly to Batch Detail.
- [ ] Start Image Run navigates directly to Run Detail.
- [ ] Batch progress is a real aggregate.
- [ ] Every image has its own status.
- [ ] Results shows only the final projection.
- [ ] Debug shows all intermediate artifacts.
- [ ] One Run's artifacts cannot be overlaid on another image.
- [ ] No Target is a valid result.
- [ ] Review links are Run-scoped.
- [ ] Model bindings show the real frozen snapshot.

## F. Workflow

- [ ] Drafts have revisions.
- [ ] Autosave uses optimistic concurrency.
- [ ] Two tabs cannot silently overwrite each other.
- [ ] Sample Tests do not overwrite history.
- [ ] Sample Tests bind to a Draft content hash.
- [ ] Publish requires an exact current Sample Test.
- [ ] Published versions are immutable.
- [ ] Pipeline URLs preserve Draft/version.
- [ ] Stale asynchronous results cannot overwrite a new Draft.

## G. Review

- [ ] A Project A route cannot display a Project B Review.
- [ ] Item changes do not leak note or reason state.
- [ ] Unsaved bbox changes have navigation protection.
- [ ] Source-box selection preserves score semantics.
- [ ] Revision History uses a proper UI.
- [ ] Review ↔ Run navigation preserves context.
- [ ] Run Detail does not download the global Review queue.
- [ ] Accept & Next uses the correct Project queue.

## H. Feature truth

- [ ] Image status filtering works or is removed.
- [ ] “Review uncertain result” performs a real action rather than scrolling.
- [ ] Advanced Graph is not exposed in a corruptible state.
- [ ] Event-count fake progress is removed.
- [ ] Sample count matches dataset bounds.
- [ ] Overview does not duplicate all Build editing controls.
- [ ] Disabled operations expose an accessible reason.
- [ ] Labs capabilities are explicitly identified.
- [ ] Generic UI does not hard-code specialist model brands.
- [ ] Model selection uses the Node capability contract.
- [ ] Recommendations obey Geometry Safety.
- [ ] Browser file import is not presented as a server-local path field.

## I. Performance and architecture

- [ ] Run list does not load complete History for every Run.
- [ ] Project list does not load complete History for every Run.
- [ ] Review progress uses a summary query.
- [ ] Lists are paginated.
- [ ] Query counts remain bounded with a 1000-Run fixture.
- [ ] `App.tsx` is incrementally split by route feature.
- [ ] Critical server routes and application services are split into modules.
- [ ] No big-bang rewrite causes behavior regression.

## Required regression journeys

- [ ] Complete Project journey.
- [ ] Global discovery into owned detail.
- [ ] Duplicate Project names.
- [ ] Project rename with historical data.
- [ ] Refresh recovery for Build/Test/Run/Review.
- [ ] Slow-request race.
- [ ] Two-tab Draft conflict.
- [ ] Final result vs intermediate artifact.
- [ ] Mixed batch status/progress.
- [ ] Cross-project object attack.
- [ ] Malicious browser Origin.
- [ ] Feature-truth audit.
