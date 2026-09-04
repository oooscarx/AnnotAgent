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

- [x] Run APIs include `project_id`.
- [x] Normal object association never uses `project_name`.
- [x] Projects with duplicate names stay isolated.
- [x] Historical Runs remain openable after a Project rename.
- [x] Reviews are bound to Project and Run.
- [x] Images use stable IDs.
- [x] A foreign Image ID cannot be written into a Run.
- [x] Import never guesses a Run by name.

## C. Routing

- [x] Project Runs is a real Project child route.
- [x] Project Review is a real Project child route.
- [x] Project Run Detail retains the Project Shell.
- [x] Project Batch Detail is deep-linkable.
- [x] Global Runs remains a cross-project index.
- [x] Global Review remains a cross-project index.
- [x] Legacy URLs redirect to the real owner.
- [x] Unknown URLs show Not Found.
- [x] Back/Forward preserves the correct hierarchy.

## D. State recovery

- [x] Pipeline refresh preserves the exact Draft.
- [x] Test refresh preserves the exact Draft and immutable Sample Test ID.
- [x] Run refresh preserves Image, View, Node, and Artifact.
- [x] Review refresh preserves the Item.
- [x] A stale request cannot overwrite a newly selected Project.
- [x] SSE reconnection resynchronizes server truth.
- [x] Local storage never determines object ownership.
- [x] In-page selection changes do not steal H1 focus.

## E. Run and results

- [x] Start Dataset Run navigates directly to Batch Detail.
- [x] Start Image Run navigates directly to Run Detail.
- [x] Batch progress is a real aggregate.
- [x] Every image has its own status.
- [x] Results shows only the final projection.
- [x] Debug shows all intermediate artifacts.
- [x] One Run's artifacts cannot be overlaid on another image.
- [x] No Target is a valid result.
- [x] Review links are Run-scoped.
- [x] Model bindings show the real frozen snapshot.

## F. Workflow

- [x] Drafts have revisions.
- [x] Autosave uses optimistic concurrency.
- [x] Two tabs cannot silently overwrite each other.
- [x] Sample Tests do not overwrite history.
- [x] Sample Tests bind to a Draft content hash.
- [x] Publish requires an exact current Sample Test.
- [x] Published versions are immutable.
- [x] Pipeline URLs preserve Draft/version.
- [x] Stale asynchronous results cannot overwrite a new Draft.

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

- [x] Image status filtering works or is removed.
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
- [x] Refresh recovery for Build/Test/Run/Review.
- [x] Slow-request race.
- [x] Two-tab Draft conflict.
- [x] Final result vs intermediate artifact.
- [x] Mixed batch status/progress.
- [ ] Cross-project object attack.
- [ ] Malicious browser Origin.
- [ ] Feature-truth audit.
