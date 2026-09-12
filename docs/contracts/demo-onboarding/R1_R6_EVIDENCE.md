# Demo R1-R6 evidence matrix

Baseline `8a7d800`; statuses describe executable code, not intended UI behavior.

| Requirement | Baseline evidence | Gap / acceptance required | Status |
|---|---|---|---|
| R1 catalog | `GET /api/demo-catalog`, exact manifest and manifest-owned asset routes load only `object-detection-review@1.0.0` from the repository allowlist | Unit/HTTP tests validate canonical SemVer, IDs, unknown fields, symlink/traversal and asset hash/size/MIME; public manifest omits paths; GET creates no Project/Run/Draft | Implemented, isolated HTTP verified |
| R2 start/review | `POST /api/demos/start` and recovery GET persist an exact command, independent owner/Conversation/Task, six image identities, delivery intent and source provenance. Preset imports become owned `Imported/NeedsReview` candidates in the existing delivery snapshot path; `GET T/workspace` routes directly to `review_delivery_images`. | Concurrent exact replay returns one identity; changed scope is 409; restart recovers the same human revision and whole-image receipt. Real router test starts preset mode, reads the first image/candidate, saves a human candidate revision, confirms that image, and proves a package with only 1/6 image receipts is rejected. Zero model attempts/Runs/Drafts remain true. Live freezes a compatible profile and has no preset/fallback, attempt, Draft or Run. | Implemented and isolated HTTP verified |
| R3 configuration | Profile persistence plus Builder/Published assembly existed | Passive effective-request API, typed reasoning mapping, output/context cap, frozen text-attempt evidence implemented; explicit HTTP TEST request pending | Implemented core/text, fixture pending |
| R4 observe/stop | Call progress, stop receipts, Run SSE, Batch/Run stop and unknown outcome semantics exist | Demo receipt must link exact operations; >3s TEST delay evidence | Partially implemented |
| R5 archive | UIAPI-018 archive-only export/preview/confirm/recovery is implemented and browser-stable | Demo-specific extension is unnecessary; execute round-trip regression on Demo Task | Implemented base, evidence pending |
| R6 usage | Final call receipts and Run usage existed; active probe stays separate | Durable physical retry rows and Task read model implemented for OpenAI-compatible conversation text and authorized Published/Sample vision routes; full Demo HTTP fixture pending | Implemented code path, fixture pending |

No commercial Provider call is authorized. Acceptance uses tempfile SQLite, loopback
HTTP, InMemorySecretStore and the explicit TEST fixture only. The 1500 input / 500
output token case uses test prices 2 and 8 per million and must persist total `0.007`;
this is not a real model quote.
