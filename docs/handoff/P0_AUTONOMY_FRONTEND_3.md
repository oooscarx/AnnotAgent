# P0 Frontend 3

Baseline: `a7bee2ca6e4908d6cb1eff3ed820a6c7ce51653e`; worktree branch `codex/p0-autonomy-f3`.

## P0-PM-G0-004 — task usage read failure

The baseline Http Adapter requests `deliveryRoot(project, task)/model-usage?limit=50`. No `model-usage` Rust route was found at this baseline. PM observed HTML reaching JSON parsing; this does not establish zero usage or no billing.

F1 and Backend were asked to confirm the canonical task usage endpoint, delivery commit, and HTTP tests. F1 owns transport wiring/content-type handling; Backend owns the route. No shared Adapter or Rust files changed here.

TaskUsage now shows a short read-failure notice with unknown cost and explicit refresh, not raw parser/HTML content. Attempt detail is collapsed by default; amounts from frozen pricing are labeled estimates. Page aggregation rejects mixed ownership and deduplicates physical attempts; late pagination results cannot append after the task/request generation changes. Failed reads do not issue model requests or introduce Fixture data.

Validation at `75f1958422f091fe99396575af9442a5f1f617f9`: focused TaskUsage unit tests 7 passed; Web typecheck including token check passed. These are local component/aggregation checks, not a production HTTP endpoint verification. Browser and live Provider checks have not been executed for this branch.

## Rolling integration evidence

Frontend 1 has already integrated the R3/R6 domain work onto the P0 integration line. The production adapter reads the passive task-owned `model-usage` page and the effective-request evidence. The UI keeps physical attempts separate, shows input/output tokens separately, preserves immutable Model Profile and pricing revisions, does not add unlike currencies, and renders missing evidence as unknown rather than zero. Provider probes remain distinct from task attempts. `reasoning_controls` remains a protocol capability declaration; only the server-provided supported-mode list is offered as a runtime selection.

The server-owned autonomous Sample path has independently passed at Backend `6e8f336f2e9e4d6ddf7c39650d6f79d4a50f0d05`: six newly uploaded images, one bounded Sample consent, zero execution relay POSTs, three Sample results requiring human judgment, and no pre-created formal Run. The same clean HTTP TEST run then saved 18 formal object decisions plus six whole-image decisions, exposed one package authorization only after those decisions, created one durable package job, downloaded and independently validated an 18-entry ZIP, and recovered the same package across restart. Independent manifest: `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-ntz7p1q9/manifest.json`.

This is orchestration and packaging evidence from an explicitly marked external TEST Provider, not commercial-model accuracy evidence. Live Provider execution, real pricing reconciliation, and paid task accuracy remain unverified here. Frontend 1 still owns the final production-route browser integration and must preserve the order: Sample human decisions, one formal processing approval, formal object and whole-image review, then one package consent.

Previously requested P0-AUTO-F3-002: Backend must expose a passive, scope-bound continuation verdict after setup. Frontend must not infer renewed authorization from registry changes or dispatch paid work from GET.
