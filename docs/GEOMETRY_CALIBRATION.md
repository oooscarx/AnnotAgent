# Geometry Calibration

Geometry calibration is an immutable evidence report for one exact execution method. It is not a
global statement that a model is accurate.

The calibration key binds Project, task and optional Label, Model Profile ID and revision, node
definition and configuration, prompt version, preprocessing, dataset profile, Label Schema and
refinement chain. Changing any of those inputs makes an older report effectively `stale`. Credential
rotation does not affect calibration.

Reports aggregate human-referenced geometry using sample count, small-object sample count, median
and P10 IoU, median and P90 center shift, area-ratio error, manual adjustment, too-loose and
too-tight rates. States are `uncalibrated`, `collecting_evidence`, `provisional`, `passed`, `failed`
and `stale`. Missing metrics stay missing; AnnotAgent does not invent localization confidence.

In the GUI, open Project → Build → Automation → Improve Automation and expand Geometry calibration.
Choose a published Version, a geometry-producing bound node, task/Label and reviewed Evidence Runs.
The resulting report is shown with its exact scope and staleness reasons. A passed report still
requires an explicit Geometry Evaluation/Decision path; it cannot turn a semantic Confidence Gate
into geometry evidence.

API:

- `GET/PUT /api/projects/{projectId}/geometry-policy`
- `GET/POST /api/projects/{projectId}/geometry-calibrations`
- `GET /api/geometry-calibrations/{calibrationId}`
- `GET /api/projects/{projectId}/geometry-corrections`
- `GET /api/runs/{runId}/geometry-quality`
