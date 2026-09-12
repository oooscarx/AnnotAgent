# Demo R1-R6 evidence matrix

Baseline `8a7d800`; statuses describe executable code, not intended UI behavior.

| Requirement | Baseline evidence | Gap / acceptance required | Status |
|---|---|---|---|
| R1 catalog | Repository examples exist; no server Demo catalog | Static allowlist, v1 validation, passive catalog/assets, traversal/hash tests | In progress |
| R2 start | Existing Project/Conversation/Send/delivery APIs are idempotent at their own boundaries | One atomic/recoverable Demo command and preset/live source separation | In progress |
| R3 configuration | Model Profile persists limits/defaults/pricing; Builder and Published execution consume part of the snapshot | Passive effective mapping, reasoning validation, all call paths freeze effective request/context evidence | Priority gap |
| R4 observe/stop | Call progress, stop receipts, Run SSE, Batch/Run stop and unknown outcome semantics exist | Demo receipt must link exact operations; >3s TEST delay evidence | Partially implemented |
| R5 archive | UIAPI-018 archive-only export/preview/confirm/recovery is implemented and browser-stable | Demo-specific extension is unnecessary; execute round-trip regression on Demo Task | Implemented base, evidence pending |
| R6 usage | Final call receipts and Run usage exist; active probe has a separate usage list | Physical-attempt rows, Decimal snapshots, cached/retry/unknown/mixed-currency Task read model | Priority gap |

No commercial Provider call is authorized. Acceptance uses tempfile SQLite, loopback
HTTP, InMemorySecretStore and the explicit TEST fixture only. The 1500 input / 500
output token case uses test prices 2 and 8 per million and must persist total `0.007`;
this is not a real model quote.
