# RoboCup Ball Hybrid — Offline Mock

Copy `project.yaml` into a workspace Project directory. Because the legacy `skill: robocup`
marker is retained solely for the bundled synthetic-image generator, the first run can create a
local fixture when `images/` is empty. The active Skill set is the explicit four-entry
`enabled_skills` list.

Create the `robocup.ball.specialist_with_open_vocab_fallback` Draft. The Project capability
configuration binds all three model-backed nodes to in-process mocks, so validation, Dry Run,
publish, Artifact inspection, Replay, and review require no API key or downloaded weight.
`scenarios.yaml` documents the seven deterministic cases covered by Rust tests.
