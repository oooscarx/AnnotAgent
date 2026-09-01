# Geometry Safety Acceptance

Status values: `open`, `implemented`, `verified`, `live-conditional`.

| Case | Status | Current evidence |
|---|---|---|
| 1. High semantic score cannot pass geometry safety | open | M0 fixture currently proves the unsafe legacy acceptance. |
| 2. VLM plus mandatory Review is legal | open | Existing graph primitives are present; geometry policy test pending. |
| 3. Healthy SAM refinement path | open | Core conversion nodes exist; availability/quality policy pending. |
| 4. Unavailable SAM becomes setup alternative | open | Builder has availability concepts; exact safe fallback test pending. |
| 5. Provider failure does not suggest SAM | open | Failure classifier exists; Agent behavior test pending. |
| 6. No candidate does not suggest SAM | open | Failure classifier exists; Agent behavior test pending. |
| 7. Wrong object does not use SAM as primary fix | open | Domain guidance exists; Agent behavior test pending. |
| 8. Bbox edit creates geometry evidence | open | Manual geometry helpers exist; durable lineage record pending. |
| 9. Calibration can pass with sufficient evidence | open | Not implemented. |
| 10. Configuration changes stale calibration | open | Not implemented. |
| 11. Qwen-only first Draft requires Review | open | Current published template violates the desired rule. |
| 12. Registered SAM enables improvement Draft | open | Not implemented end to end. |
| 13. No measured improvement means no recommendation | open | Comparison tools exist in partial form; holdout rule pending. |
| 14. Legacy Workflow remains immutable and new Run is guarded | open | Version immutability exists; safety compatibility pending. |
| 15. Small objects are evaluated separately | open | Existing summary lacks size buckets. |
| 16. Generic Project remains domain-neutral | open | Must be covered by static and UI tests. |

## M0 evidence

- Fixture: `workflow::tests::baseline_reproduces_unsafe_vlm_semantic_score_auto_commit`.
- Predicted box: `[0.40, 0.40, 0.30, 0.30]`.
- Human reference: `[0.48, 0.48, 0.10, 0.10]`.
- Declared semantic score: `0.99`; geometry: `coarse_hypothesis`.
- Legacy result: validation report is valid and contains no geometry-safety blocker.
- Verification: `cargo test -p annotagent-core` passed all 74 Core tests; formatting and diff
  whitespace checks passed.
