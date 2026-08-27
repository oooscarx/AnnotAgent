# Guided Workspace Known Limitations

Updated: 2026-08-27

## Baseline limitations

- Image import reports duplicates but the current backend does not expose corrupt-image diagnostics; the UI marks that report unavailable.
- Published-version archival is not implemented by the current backend; Version History supports view, compare, and clone-to-Draft only.
- Legacy Runs without a published Pipeline checkpoint may expose Workflow Version but cannot resolve a source Artifact ID.
- The browser harness validates the responsive equivalent of 200% zoom at 720 CSS pixels; platform-specific native zoom rendering is not pixel-diffed.
- HTTP image import accepts only paths inside the configured workspace. The E2E harness stages its source image inside its isolated workspace before import.
- Historical live-provider Replay intentionally does not recover credentials from Run history. A Replay whose selected downstream subgraph still contains a live model requires a current explicit binding; Core-only downstream Replay uses the persisted checkpoint without one.
- The final acceptance did not issue a new external VLM request because the task forbids using conversation API keys. Live bbox/crop evidence comes from the persisted successful B-Human Run; all new workflow execution tests use the offline Mock provider.

Items are removed only when verified; remaining gaps at release stay documented here.
