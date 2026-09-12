# Mainline Frontend 1

## Baseline

- Common baseline: `d3220cb54bd50589efed86b6c0753bf4ea8db0db`.
- Integration worktree: `/Users/oscar/Documents/my_workspace/AnnotAgent-mainline-integration`.
- Branch: `codex/mainline-integration`.
- The source worktree contained only user-owned untracked design/product directories; none were copied, removed or committed here.

## F1-0 in progress

Frontend 1 owns App/http/adapter/public route and type composition. Frontend 2 owns review/image/package modules. Frontend 3 owns model/plugin/settings modules. Backend owns Rust, migrations and `docs/contracts/mainline-v1`.

The first seam defines a server-backed `MainlineTaskView`, typed real message projections, complete frozen `VisualSelection`, capability setup return object and domain composition slots. It introduces no guessed HTTP route and no browser state machine. A task is complete only when the server-backed training package is Ready and downloadable.

Communication:

- `ML-001` queued to Frontend 2 UUID `01a094ad-5715-75d0-88cf-33b5eb7fa3e8`.
- `ML-002` queued to Backend UUID `01a0855e-9c39-7c33-9f18-93e084d14816`.
- `ML-003` resolved exact Frontend 3 UUID `01a094ad-337c-7bc0-99ea-b3d18acb7d2f` and was queued successfully.

No Provider call, true workspace write, push or main merge.

### F1-0 delivery

- `7fd1439`: stable composition seam and ownership/status records; 336 unit tests passed.
- Backend B0 contract `99d16f9` integrated as `5a732c5`; all planned routes remain disabled/unreferenced.

### F1-1 current slice

The existing server-backed delivery-intent editor now names the missing slots and initially renders only those fields. Full editing stays explicitly available. Saved dataset scope, label rules and YOLO training target remain one Task-owned revision; no model call occurs when saving them.

Persisted structured model decisions are projected as Agent replies or clarification messages with the actual call identity/status. Failed, empty or invalid receipts remain operation records and are not converted into assistant prose. The generic completed phase now says “current operation completed”; overall completion remains reserved for Package Ready.

Backend reports the exact processing subset, automatic package consent HTTP and the unified task read model as planned, not implemented at B0. Frontend does not call those routes yet.

The HTTP send boundary now accepts only a frozen `VisualSelection`, converts it to the existing `sample_candidate` message reference, verifies the Project Schema is still current before POST, and keeps the same command for uncertain retries. Preview-only/bare candidate identities are rejected before any request. Frontend 2 still needs to supply this selection from the real canvas before the Composer can expose the path.

### Isolated HTTP evidence

- Service: `http://127.0.0.1:8841`, fixture header `external-model-only`, generated TEST workspace only.
- Source build: `2c4b032bf614a63e26fc56904368d3b050ee315f` before the candidate-send boundary commit.
- The real Project route restored saved user messages, structured model decision receipts, execution history, a pending HumanRequest, completed Batch metadata and a real download link.
- Expanding delivery intake displayed only the three server-reported missing items. No Provider request or real workspace write was made.
- Candidate-reference unit coverage: exact image hash, Schema revision, Draft revision, Sample Test, candidate and source Artifact; stale Schema and Preview identities make zero POSTs.
- Browser regression `persisted Agent replies stay distinct and intake asks only server-reported missing items` passed against that isolated HTTP service and observed zero non-GET requests across open and refresh.
