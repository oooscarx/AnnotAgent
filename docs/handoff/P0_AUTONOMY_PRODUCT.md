# P0 autonomy product handoff

Product branch: `codex/p0-autonomy-product`<br>
Baseline SHA: `a7bee2ca6e4908d6cb1eff3ed820a6c7ce51653e`<br>
Role boundary: product walkthrough and copy only; no runtime files changed.

## Delivered

- `docs/product/p0-autonomy/BASELINE_WALKTHROUGH.md` records a real isolated HTTP
  path from six new uploads and one complete user request to three saved Sample
  candidates. It classifies every manual action and records the persisted operation IDs.
- `docs/product/p0-autonomy/COPY_SPEC.md` defines the five default states, external
  blockers, forbidden internal instructions and the Paper & Graphite presentation.

## Baseline conclusion

The existing Rust Journey completed the fast Builder and three-image Sample after one
bounded authorization. It created three pending HumanRequests without a second Sample
execution action. The current UI still makes the user repeat explicit request data,
refresh to observe completion and click a locator before the review canvas appears.

This is G0 evidence. It is not a G1 pass. Async Builder completion over three seconds,
subscription race, browser-close continuation, server restart, and unknown remote result
still require Backend evidence on the rolling integration SHA.

## Integration requests

- Frontend 1: use one current-task read model; show one authorization and automatically
  select the review target when `needs_review` arrives. Keep technical panels reachable
  outside the default path.
- Frontend 2: supply the concrete review question and terminal Sample candidate without
  turning it into a formal annotation.
- Frontend 3: retain the bounded model/receiver/budget facts and replace raw JSON parsing
  failures with an accurate usage-unavailable state.
- Backend: prove async wake and lifecycle cases with the existing Journey executor and
  return fixed commit/test evidence to Frontend 1.

Frontend 1 owns `docs/execution/P0_AUTONOMY_UX_STATUS.md` and the final screenshots.
README and product claims must wait for an acceptance-passing integrated SHA.
