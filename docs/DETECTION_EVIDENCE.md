# Detection Evidence and Decisions

AnnotAgent combines detector output through two generic, deterministic Core operations. Neither
operation knows a model brand or domain Label.

## Match Detection Sets

`core.match_detection_sets` accepts exactly two `DetectionSet` Artifacts for the same image and
emits one `CandidateClusterSet`.

```yaml
method: iou
minimum_iou: 0.5
preserve_unmatched: true
```

Matching is one-to-one and stable. Candidates with the same Project Label and sufficient IoU form
`multi_source_agreement`; overlapping candidates below the threshold become `geometry_conflict`;
overlapping candidates with different Project Labels become `label_conflict`. Unmatched candidates
remain `single_source` when `preserve_unmatched` is true. Every member keeps its source model,
capability, model/query identity, Project Label, original box and its own optional score semantics.
The representative box is selected deterministically; AnnotAgent never averages detector scores.

Detection must be mapped to a Project Label before matching. An unmapped candidate fails closed
because model-native class names are not annotation semantics.

## Evidence Gate

`core.evidence_gate` accepts a `CandidateClusterSet`, upstream `validation_issues`, and optional
structured `correction_risk`. It selects exactly one route:

- `accept`
- `fallback`
- `review`
- `reject`

The node supports explicit `accept_when`, `fallback_when`, `review_when`, and `reject_when` rule
lists. Source fields are exact Model Registry IDs, not backend aliases. A score threshold considers
only calibrated probability or relative-confidence values. Missing, ranking-only, and unknown
scores remain incomparable and can be routed to Review.

Every execution emits a persisted `evidence_gate` report with the decision, stable reason codes,
plain-language messages, candidate/source references and numeric metrics. These are visible in Run
Debug beside the Candidate Cluster preview. They are observable decision facts, not hidden model
reasoning.

Example:

```yaml
accept_when:
  - minimum_sources: 2
    minimum_iou: 0.6
  - source: specialist-model-v1
    minimum_score: 0.85
    no_domain_issue: true
fallback_when:
  - source: specialist-model-v1
    empty_specialist_result: true
  - source: specialist-model-v1
    specialist_score_below: 0.55
  - domain_issue: true
review_when:
  - geometry_conflict: true
    label_conflict: true
  - score_missing: true
reject_when: []
```

Rules inside one object are conjunctive; separate list entries are alternatives. Explicit Reject
has highest precedence, followed by Fallback, safety review for evidence conflicts, configured
Review, and then Accept. If no rule is sufficient, the safe default is Review.

`CandidateClusterSet` is a persisted lineage parent. A single-source accepted cluster may project
its comparable score into an Annotation. A multi-source cluster does not fabricate an aggregate
confidence; the committed/reviewed Annotation keeps the source-model list and leaves confidence
unset while the complete evidence remains inspectable.

## Results and Review

Run Results lists each source model and its own score semantic; a score-less result says
`confidence not provided`. The Evidence Inspector exposes original boxes, capability, query/model
label and agreement IoU or conflict. Copied downstream Artifacts are deduplicated by source-model
and geometry identity without deleting genuinely independent evidence.

Review receives a stable structured queue explanation for policy, low score, missing score,
fallback, geometry/label conflict, validation issue or domain hard negative. `Use {model} box`
replaces the editable rectangle and stores `selected_detection_evidence` in the revision. It does
not bypass Save/Accept, and the selected source remains available to Correction Memory.

Domain wording originates in the persisted Skill-owned `ValidationIssue.message`. The generic
Server correlates issues by Annotation ID and renders that message without branching on a domain
Label or reason-code substring.
