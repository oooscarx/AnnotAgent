# Guided Workspace Known Limitations

Updated: 2026-08-27

## Baseline limitations

- Navigation has eight primary destinations and is held in component-local state.
- Project, build, run artifact, and review selection are not represented by durable URLs.
- Workflow authoring is globally located instead of project-scoped.
- Models and Skills are global primary destinations rather than Settings sections.
- Artifact inspection is embedded at the bottom of the Workflows page.
- Dry Run emphasizes execution details before user-facing result totals.
- Shared-stage reuse and label-lane relationships require internal workflow knowledge.
- Project readiness is inferred by the client and lacks an explicit blocking-issues contract.
- Review items lack complete source workflow/node navigation context in the current UI.
- Bbox and crop selection are not yet bidirectionally linked.
- The current Web project has unit tests but no configured browser/e2e runner.

Items are removed only when verified; remaining gaps at release stay documented here.
