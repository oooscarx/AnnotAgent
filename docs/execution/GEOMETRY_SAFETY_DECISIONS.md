# Geometry Safety Decisions

## D-001 — Safety is enforced below the prompt

Advisor instructions will explain preferred pipelines, but Core publication validation will reject
unsafe score-only geometry acceptance even if an Agent proposes it.

## D-002 — Quality contracts are operation-scoped

Quality meaning is keyed by Model Profile revision plus capability/operation. A text-generation or
classification use of the same remote model must not inherit bbox geometry claims.

## D-003 — Scores, measured geometry and review state remain separate

Semantic/detector scores retain their declared semantics. IoU, center shift, area ratios and mask
support are reports, not fabricated confidences. Calibration and human verification are separate
states.

## D-004 — Conservative legacy interpretation

Legacy VLM detections migrate to coarse, uncalibrated geometry. Legacy specialist detections migrate
to predicted, uncalibrated geometry. Historical versions are not rewritten.

## D-005 — Refiner availability is evidence-backed

SAM may enter a runnable Draft only when a compatible prompted-segmentation Model Profile and Worker
have publishable availability evidence. Adapter or example-worker source code alone is insufficient.

## D-006 — Improvements patch existing automation

Improve Automation preserves the baseline graph and produces an auditable Draft diff. It never
publishes, starts a full Run or uses diagnosis samples as sufficient proof of improvement.

## D-007 — Geometry safety requires a boundary on every Commit path

A Human Review, refiner or evaluation node is protective only when it dominates the relevant
candidate-source-to-Commit path. A side branch to Review does not make a direct auto-commit branch
safe, and a semantic/domain Validator is not geometry evidence.

## D-008 — Dry Run is inspectable; publication and production execution fail closed

Unsafe geometry issues remain visible as warnings during a sandbox Dry Run so evidence can be
collected. The same issues are blocking during publication. Immutable legacy versions are assessed
again before a new production Run and fail with `unsafe_legacy_workflow`; historical Run viewing and
sandbox Replay remain available.

## D-009 — Default bbox policy prefers a safe, usable repair

Until exact calibration exists, bbox Projects require training-quality geometry and accept a
mandatory Review or an available geometry-refinement path. `allow_unvalidated_commit` is not a
geometry-risk override. Typed risk acceptance is hash-bound but cannot bypass the conservative
default policy.
