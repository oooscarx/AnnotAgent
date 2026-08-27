# Guided Workspace Known Limitations

Updated: 2026-08-27

## Baseline limitations

- Run artifact and review selection are not fully connected to their durable URLs yet.
- Image import reports duplicates but the current backend does not expose corrupt-image diagnostics; the UI marks that report unavailable.
- Published-version archival is not implemented by the current backend; Version History supports view, compare, and clone-to-Draft only.
- Review items lack complete source workflow/node navigation context in the current UI.
- Bbox and crop selection are not yet bidirectionally linked.
- The current Web project has unit tests but no configured browser/e2e runner.

Items are removed only when verified; remaining gaps at release stay documented here.
