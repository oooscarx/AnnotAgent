# Lean Agent Alpha Known Limitations

Baseline limitations before implementation:

- A real external OpenAI-compatible loop is live-conditional because CI does not possess an
  operator credential. The identical multi-turn path is covered with a scripted model Provider.
- The Application persists the backward-compatible generic `AgentSession` audit envelope. Dedicated
  Builder status/constraints/stop-reason types exist in Core; API/GUI/TUI projection is completed in
  M6 without invalidating stored sessions.
- Live Alpha exposes a conservative subset of the 31-tool Registry: inspection, safe-template
  creation, typed disconnect/connect repair, one evidence-gated Crop verification template,
  model/parameter/decision/mapping edits, validation, Dry Run, bounded result inspection and human
  submission. Arbitrary empty-graph construction and unrestricted node removal remain unavailable.
- Node inspection returns execution metadata and structured issues, not complete Artifact bodies.
  Humans use the existing Run/Review inspectors for visual Artifact inspection; the Agent can request
  only bounded failed/Review summaries in Alpha.
- Pre-Lean Capability implementations still exist as Registry compatibility adapters and internal
  node IDs. New authoring exposes the generic Skills, but removal awaits persisted-version migration.
- Expert mode intentionally exposes internal Workflow node IDs, ports and raw parameters. Guided
  mode groups adjacent operations, but a non-adjacent pair remains separate so graph order is never
  hidden or changed.
- Published versions using the legacy `localization_grid` parameter continue to run. New authoring
  writes `grounding_assist`; the compatibility reader is not yet removed.
- Partial Diff selection operates on the exact flat typed DAG. When that selection no longer maps
  losslessly to the optional Label Pipeline authoring projection, the projection is dropped and the
  same graph remains editable through Guided recipe and Expert views; publish validation still
  applies.
- GUI Undo is deliberately one level and held by the current Automation page. Applied Draft content
  is durable, but the Undo affordance itself is not reconstructed after a browser restart; Agent
  Session and Suggested Draft audit records remain durable.
- Live Agent progress uses 250 ms polling of persisted Sessions rather than SSE. Very fast offline
  ScriptedMock work may complete before an intermediate frame is painted; the complete persisted
  Tool trace is still shown with the proposal.
- SAM, LocateAnything and RF-DETR workers are not running in the audited environment. YOLO has no
  repository weight. Real inference is not claimed.
- Runtime Recovery remains named as an Agent in code and some UI copy even though its behavior is
  deterministic and bounded.
