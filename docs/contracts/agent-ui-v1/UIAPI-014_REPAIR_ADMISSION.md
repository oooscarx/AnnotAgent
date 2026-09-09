# UIAPI-014 second-round repair admission

The prior Builder implementation persisted the exact repair source, but its subsequent model calls only carried the Task grant. The global pending-human gate therefore rejected a valid Applied correction when another image still needed an answer. Sample vision calls had the same omission.

This increment carries the existing Builder/Sample operation ID to the same call ledger. No new HTTP route, grant, retry or model engine is introduced. Normal Schema, queued planning, feedback interpretation and arbitrary Task calls keep the pending-human gate.

## Narrow admission scope

A Builder call may proceed past unrelated pending answers only when its durable operation is `reserved`, belongs to the Task, preserves its admitted `call_scope_hash`, and names a `human_request` repair source. In the call reservation transaction the server rechecks the precise HumanRequest is Applied, its answer equals the repair copy's saved feedback, its resume identity matches the editable owned copy, and the copy uses the admitted owned Schema revision. The Application still verifies the originally confirmed repair revision/hash and approved model before entering Builder. Cancelled, interrupted or completed operations cannot supply this exception.

A Sample call requires its exact operation to be `running` and to be the current sample grant. The saved operation Task/scope and original confirmed revision bind the editable Applied repair copy. The existing exclusive sample lease must belong to this operation; it permits the trusted preparation step to advance revision while excluding concurrent editor changes. Original sample scope validation/seals continue to bind model/image selections. The same answer/feedback/Schema checks run before each call. Merely possessing an old repair Draft ID is insufficient.

Both paths retain schema-clarification, cancellation/stop, expiry, revocation, Project limit and remaining Task allowance checks. No other request is answered, cancelled, accepted or removed. Errors and unknown-outcome semantics stay unchanged; no automatic retry is added.

New reserved Builder evidence includes `call_scope_hash` when a Task grant exists. Old receipts remain readable. Old interrupted operations are not re-executed: fetch a fresh existing `builder-preview` or `journey-preview` for the current saved repair revision and explicitly authorize a new operation. Reposting an original completed operation only recovers its receipt.

## Saved typed repair reuse

A new repair preview may refer to the copy already advanced by the earlier local precomposition. The selected-label, bounded local recovery graph is recognized and statically revalidated as a saved candidate. It is protected from setup-template replacement and retained at the existing budget boundary. The repair compiler does not append another local-recovery pass. No graph is called successful inference merely because it validates; geometry and Review gates remain active during the separate sample.

## Isolated acceptance

- Existing Application Schema/Builder/Repair scenario now seeds two other images with pending requests using the real Store. Ordinary and fake-operation calls are rejected; exact Applied repair calls are metered; other requests remain byte-equivalent; stale repair, completed-operation reuse and source lifecycle guards remain covered.
- Existing typed-localization test verifies re-recognition preserves the exact graph and cannot select unrelated labels or add a second recovery pass.
- `crates/annotagent-e2e-fixture/support/http_repair_admission_check.py` creates three TEST images through real HTTP, executes the existing external deterministic Provider/sample pipeline, saves one answer, then explicitly invokes its Builder and a separately authorized Sample while two other images remain pending. It verifies real call receipts, original-operation replay and unchanged pending requests.

Run an opt-in fixture with `python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture`, then run `python3 crates/annotagent-e2e-fixture/support/http_repair_admission_check.py /absolute/TEST-agent-ui-.../manifest.json`. The checker requires the TEST marker, loopback fixture header and non-production ports; its trace is saved as `UIAPI014_REPAIR_HTTP_TRACE.json`. External model output is synthetic; API, database, admission, feedback, Builder and Sample code are real. This proves transport/admission, not real-model localization quality. No real workspace or paid model was used.

Verified real HTTP trace: TEST-agent-ui-k5_lx924/UIAPI014_REPAIR_HTTP_TRACE.json (isolated retained workspace). Builder reaches draft_ready_for_human_review, separate three-image Sample has zero failed images, two earlier pending requests remain identical, and Builder replay records zero extra calls. Strict all-target Storage/Application/Server clippy and formatting passed.

Final Rust verification: Application/Storage/Server tests **413 passed, 3 existing ignored, 0 failed**. No migration is needed for this increment. The owned TEST fixture was stopped after verification; its database and traces were retained.
