# UIAPI-011 round 2 — preserve admitted Builder scope at salvage

## Root cause and repair

Conversation supplies the exact persisted Schema task/label and Journey-filtered Registry through `WorkflowAdvisorInput`; its outer Builder `target` argument is intentionally `None`. At the discovery deadline, Runtime reloaded a project-wide input using that outer argument. This lost the Schema-only target and the admitted Registry subset. Deterministic Registry fragment synthesis therefore found no bounding-box target, skipped the prompted-refinement candidate, and salvaged only the VLM baseline.

The refresh now retains the admitted Schema, goal, task/label, constraints and model/provider identities. Availability metadata still comes from the fresh Registry; missing/disabled models are not resurrected. Newly registered/unapproved identities do not enter this call. Existing deterministic conversion discovery, synthesis, candidate ranking, static validation and LabelPipeline materialization perform the rest. No new engine, model-brand recognition, paid retry or bypass of sampling guards.

## Integration contract

No new HTTP fields or migration. Existing Builder operation, task workspace, Draft `label_pipeline`, selected candidate and validation receipts retain their shapes. For an admitted single bounding-box label with an available compatible prompted segmenter, the existing typed path can be synthesized at salvage even when the model never invokes `find_geometry_refinement_path`:

Detection → Box Prompts → Prompted Segmentation → Mask to BBox → Geometry Evaluation → Geometry Decision/Review.

Registry model IDs are preserved, including `model-instance:` IDs. This is planning/materialization evidence, not proof that a real VLM/SAM inference succeeded. Geometry review remains required where existing safety rules require it. Unavailable refinement still cannot be declared runnable. The earlier completed VLM-only operation/Draft is not rewritten; the integration owner must use the existing explicit, freshly approved operation/working-copy flow and verify real inference separately.

## Isolated regression

A real temporary Conversation, user message and saved human Schema define a label absent from project.yaml. The Builder receives it only through input, outer target=None, planning-only, maximum four turns, and a scripted Provider that never calls path discovery. The result must retain exact Schema/task/label, an actual segmenter binding in both nodes and LabelPipeline, and valid static validation. A subsequently registered but unapproved segmenter must remain excluded after refresh.

Commands: `cargo test -p annotagent-application --offline`; `cargo clippy --workspace --all-targets --offline -- -D warnings`; `cargo fmt --all -- --check`. The existing native model-instance materialization and unavailable-segmenter tests remain relevant; no weights, worker process or real provider is used.

## Separate pending work

UIAPI-009 implementation is paused at the integration owner's explicit priority change. Its uncommitted storage/API/test work and migration 0059 remain in the backend worktree and are excluded from this Builder delivery. It has no delivery SHA or acceptance claim yet. UIAPI-010 Model PATCH atomic CAS was delivered separately as `210091a17b1d4ff7b5fe0a306cb9100f1b548aea`.

Isolated delivery-only worktree verification: Application 158 passed / 0 failed / 1 existing ignored; workspace all-target Clippy with `-D warnings`, fmt and diff checks passed. Pre-fix regression failed specifically because the segmenter node was absent.
