# Guided Workspace Known Limitations

Updated: 2026-08-27

## Baseline limitations

- Image import reports duplicates but the current backend does not expose corrupt-image diagnostics; the UI marks that report unavailable.
- Published-version archival is not implemented by the current backend; Version History supports view, compare, and clone-to-Draft only.
- Legacy Runs without a published Pipeline checkpoint may expose Workflow Version but cannot resolve a source Artifact ID.
- The current Web project has unit tests but no configured browser/e2e runner.

Items are removed only when verified; remaining gaps at release stay documented here.
