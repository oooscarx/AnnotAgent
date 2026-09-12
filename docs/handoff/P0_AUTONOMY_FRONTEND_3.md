# P0 Frontend 3

> Historical P0 evidence was imported into `codex/mainline-frontend-3`. The current integration boundary and latest validation live in `docs/handoff/FRONTEND_3_MODEL_PREPARATION.md`.

Baseline: `a7bee2ca6e4908d6cb1eff3ed820a6c7ce51653e`; worktree branch `codex/p0-autonomy-f3`.

## P0-PM-G0-004 — task usage read failure

The baseline Http Adapter requests `deliveryRoot(project, task)/model-usage?limit=50`. No `model-usage` Rust route was found at this baseline. PM observed HTML reaching JSON parsing; this does not establish zero usage or no billing.

F1 and Backend were asked to confirm the canonical task usage endpoint, delivery commit, and HTTP tests. F1 owns transport wiring/content-type handling; Backend owns the route. No shared Adapter or Rust files changed here.

TaskUsage now shows a short read-failure notice with unknown cost and explicit refresh, not raw parser/HTML content. Attempt detail is collapsed by default; amounts from frozen pricing are labeled estimates. Page aggregation rejects mixed ownership and deduplicates physical attempts; late pagination results cannot append after the task/request generation changes. Failed reads do not issue model requests or introduce Fixture data.

Validation: focused TaskUsage unit tests 7 passed; Web typecheck including token check passed. These are local component/aggregation checks, not a production HTTP endpoint verification. Browser and live Provider checks have not been executed for this change. Endpoint repair and G1 server-owned continuation remain pending external delivery.

Previously requested P0-AUTO-F3-002: Backend must expose a passive, scope-bound continuation verdict after setup. Frontend must not infer renewed authorization from registry changes or dispatch paid work from GET.
