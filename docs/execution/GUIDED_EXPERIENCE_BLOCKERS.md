# Guided Experience Alpha Blockers

Updated: 2026-08-30

## Active blockers

None. No automated or in-repository Release Blocking item remains open.

## Manual verification

- Actual 200% browser zoom requires a browser environment where the browser chrome's native zoom level can be changed and observed. Automated 1024px and 720×450 coverage plus reduced-motion emulation are not represented as the same check.

## Live-conditional evidence

- A real external VLM smoke requires a user-configured credential through the supported workspace-local secret boundary.
- A real external detector/segmenter smoke requires a configured worker/model.

These conditions do not block the offline Mock Guided Experience Alpha. No credential from conversation history will be read, restored, logged, or used.
