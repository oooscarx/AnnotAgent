# Evidence-Driven Pipeline Self-Improvement

AnnotAgent improves an existing Published Workflow by creating editable Drafts. It never edits the
published version and never publishes a candidate automatically.

## Evidence boundary

An improvement session has two explicit sets:

- Diagnosis Evidence Runs contain human corrections and structured Runtime or Validator failures.
- Evaluation Holdout Runs contain human-accepted bounding boxes used only for before/after
  evaluation.

The Run sets must be disjoint, and the comparison also rejects overlapping Project image indices.
Four evaluation images can never produce a recommendation. Five to nine images are provisional;
the default recommendation threshold is ten independent images. A Project may make the policy more
conservative, but insufficient evidence is never upgraded by an Agent response.

## Diagnosis and patching

Failures are classified as infrastructure, Provider, no candidate, semantic, geometry, missing
score, domain risk, invalid Artifact, budget or insufficient evidence. Prompted segmentation is
considered only when the primary evidence is a geometry error with an existing candidate. It is not
used to repair Provider failures, missing candidates or wrong-object semantics.

The service clones the exact immutable baseline twice: one comparison Draft and one candidate. A
geometry repair may add the typed Detection → Box Prompt → Prompted Segmentation → Mask to BBox →
Geometry Evaluation → Geometry Decision chain when an Available model exists. Otherwise the
candidate retains or adds mandatory Human Review and records an unresolved setup requirement. The
result is stored as a `PipelineDraftDiff`; unrelated graph replacement is not part of this flow.

## Comparison

Baseline and candidate execute on the same independent holdout in a non-committing Dry Run. The
stored comparison includes semantic precision/recall, mean/median/P10 IoU, median/P90 center shift,
manual resize, loose/tight/no-candidate/review rates, cost, latency, failure classes and separate
small/medium/large object buckets.

A candidate is recommended only when evidence is sufficient, recall stays within policy, median
IoU improves without P10 regression, manual adjustment does not increase, review/cost/latency stay
within constraints, and no new failure class appears. A recommendation is advisory; applying
selected changes still creates an editable Draft and publication remains a separate human action.

## API

- `POST /api/projects/{projectId}/pipeline-improvements` creates a diagnosis and candidate Draft.
- `GET /api/projects/{projectId}/pipeline-improvements` lists persisted sessions.
- `GET /api/pipeline-improvements/{improvementId}` reads diagnosis, diff, validation and comparison.
- `POST /api/pipeline-improvements/{improvementId}/compare` runs the independent before/after
  comparison.
- `POST /api/pipeline-improvements/{improvementId}/apply-to-draft` applies only explicitly selected
  diff changes after comparison.

The Pipeline Builder also exposes the bounded read-only `compare_pipeline_geometry` Tool for a
persisted session. It cannot publish, start a full Run, access credentials or turn provisional
evidence into a production recommendation.
