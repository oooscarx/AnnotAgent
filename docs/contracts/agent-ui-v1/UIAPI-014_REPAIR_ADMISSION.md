# UIAPI-014 second-round repair admission

The prior Builder implementation persisted the exact repair source, but its subsequent model calls only carried the Task grant. The global pending-human gate therefore rejected a valid Applied correction when another image still needed an answer. Sample vision calls had the same omission.

This increment carries the existing Builder/Sample operation ID to the same call ledger. No new HTTP route, grant, retry or model engine is introduced. Normal Schema, queued planning, feedback interpretation and arbitrary Task calls keep the pending-human gate.

## Narrow admission scope

A Builder call may proceed past unrelated pending answers only when its durable operation is `reserved`, belongs to the Task, preserves its admitted `call_scope_hash`, and names a `human_request` repair source. In the call reservation transaction the server rechecks the precise HumanRequest is Applied, its answer equals the repair copy's saved feedback, its resume identity matches the editable owned copy, and the copy uses the admitted owned Schema revision. The Application still verifies the originally confirmed repair revision/hash and approved model before entering Builder. Cancelled, interrupted or completed operations cannot supply this exception.

A Sample call now additionally requires an active sealed repair Journey (see the follow-up below). It requires its exact operation to be `running` and to be the current sample grant. The saved operation Task/scope and original confirmed revision bind the editable Applied repair copy. The existing exclusive sample lease must belong to this operation; it permits the trusted preparation step to advance revision while excluding concurrent editor changes. Original sample scope validation/seals continue to bind model/image selections. The same answer/feedback/Schema checks run before each call. Merely possessing an old repair Draft ID is insufficient.

Both paths retain schema-clarification, cancellation/stop, expiry, revocation, Project limit and remaining Task allowance checks. No other request is answered, cancelled, accepted or removed. Errors and unknown-outcome semantics stay unchanged; no automatic retry is added.

New reserved Builder evidence includes `call_scope_hash` when a Task grant exists. Old receipts remain readable. Old interrupted operations are not re-executed: fetch a fresh existing `builder-preview` or `journey-preview` for the current saved repair revision and explicitly authorize a new operation. Reposting an original completed operation only recovers its receipt.

## Saved typed repair reuse

A new repair preview may refer to the copy already advanced by the earlier local precomposition. The selected-label, bounded local recovery graph is recognized and statically revalidated as a saved candidate. It is protected from setup-template replacement and retained at the existing budget boundary. The repair compiler does not append another local-recovery pass. No graph is called successful inference merely because it validates; geometry and Review gates remain active during the separate sample.

## Isolated acceptance

- Existing Application Schema/Builder/Repair scenario now seeds two other images with pending requests using the real Store. Ordinary and fake-operation calls are rejected; exact Applied repair calls are metered; other requests remain byte-equivalent; stale repair, completed-operation reuse and source lifecycle guards remain covered.
- Existing typed-localization test verifies re-recognition preserves the exact graph and cannot select unrelated labels or add a second recovery pass.
- `crates/annotagent-e2e-fixture/support/http_repair_admission_check.py` creates three TEST images through real HTTP, executes the existing external deterministic Provider/sample pipeline, saves one answer, then explicitly invokes its Builder, verifies an ordinary Sample is blocked without calls, and executes a sealed repair Journey Sample while two other images remain pending. It verifies real call receipts, original-operation replay and unchanged pending requests.

Run an opt-in fixture with `python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture`, then run `python3 crates/annotagent-e2e-fixture/support/http_repair_admission_check.py /absolute/TEST-agent-ui-.../manifest.json`. The checker requires the TEST marker, loopback fixture header and non-production ports; its trace is saved as `UIAPI014_REPAIR_HTTP_TRACE.json`. External model output is synthetic; API, database, admission, feedback, Builder and Sample code are real. This proves transport/admission, not real-model localization quality. No real workspace or paid model was used.

Historical pre-tightening HTTP trace (standalone Sample is no longer exempt): TEST-agent-ui-k5_lx924/UIAPI014_REPAIR_HTTP_TRACE.json (isolated retained workspace). Builder reaches draft_ready_for_human_review, separate three-image Sample has zero failed images, two earlier pending requests remain identical, and Builder replay records zero extra calls. Strict all-target Storage/Application/Server clippy and formatting passed.

Final Rust verification: Application/Storage/Server tests **413 passed, 3 existing ignored, 0 failed**. No migration is needed for this increment. The owned TEST fixture was stopped after verification; its database and traces were retained.


## Follow-up: sealed Journey Sample only

The previous operation identity plumbing in `ConversationVisionCalls::begin` and `sample_limits` is retained. At call reservation, the existing running-Sample check additionally loads its saved `conversation.journey_consent_id`. The effective consent must be active, unexpired and a repair of this exact Applied HumanRequest/copy. Its sealed Sample must match operation ID, Draft ID, confirmed revision, authorization fingerprint, original Builder grant, human-review setting and ordered image indices. Existing `fits` checks the exact Schema, image hashes, allowed model binding digests and call ceiling. The execution seal must exist and match the copy's Schema and sealed image identities. Original live scope/model validation and exclusive lease remain in force; trusted preparation may advance the Draft revision under that lease.

No client flag grants this exception. Missing Journey/seal, ordinary samples, non-repair Journeys, expiry/revocation, changed revision/fingerprint/model/image scope and mismatched source remain blocked. Existing Schema clarification, stop, ownership, budget and grant checks remain unchanged. Other pending answers are not edited or deferred.

HTTP contract is unchanged: fetch `GET T/journey-preview` with `repair_request_id` and the existing explicit selection fields, confirm the returned consent through `POST T/journey-consents`, then dispatch `POST T/journey-consents/{id}/execution`. The server seals and supplies Sample identity itself. Observe existing execution projection (`sample.status`, `dispatch.error`) and Sample report. An ordinary Sample may finish orchestration with per-image admission failures; a terminal operation alone does not mean successful model inference.

The replayable TEST checker now exercises the real Sample adapter through `sample_limits` → `ConversationVisionCalls` → the durable ledger. Ordinary Sample: three failed images, zero new call receipts. Active sealed repair Journey: three images, zero failures. Two original pending requests remain byte-equivalent; completed Builder replay adds zero calls. Retained trace: `TEST-agent-ui-f41u455q/UIAPI014_REPAIR_HTTP_TRACE.json`; Journey `751e001c-e3e7-4e28-903b-26efe1f6317d`, Sample `b32e683a-6d8d-4805-aad4-cebe584692f6`.

The deterministic HTTP case executes the VLM adapter; it is not evidence of a paid SAM run or segmentation quality. Registry model-binding mismatch is covered by isolated storage regression, and the shared admission gate does not special-case model names. No migration or new runtime is introduced.

Follow-up verification: `cargo test --offline -p annotagent-storage -p annotagent-application -p annotagent-server`: **418 passed, 3 existing ignored, 0 failed**. Strict all-target clippy (`-D warnings`) and `cargo fmt --all -- --check` passed. The owned TEST fixture was stopped; its database and HTTP trace remain available.
