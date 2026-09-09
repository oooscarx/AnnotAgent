# UIAPI-011 — preserve executable LabelPipeline materialization

Scope: bounded Builder/data-path repair, not a new execution engine. The existing bounded sample guard, Registry availability/ownership checks, exact Journey approval, model snapshots, Geometry Safety and call budgets are unchanged. No real workspace or reported Draft was read/edited by this agent; no paid request, model installation, service restart, push or main merge.

## Root cause and changes

1. Conversation already creates a controlled `LabelWorkflowComposition`, but `PipelinePlanCandidate` captured only nodes/edges. FromScratch starts an empty working draft; candidate materialization and revalidation consequently lost the authored `label_pipeline` even when the candidate originated from a controlled pipeline.
2. Candidate now has additive nullable `label_pipeline` (serde default for old sessions). Candidate creation, persistent Session round-trip, Registry revalidation and materialization preserve it. A legacy flat candidate remains null; no projection is fabricated from a random DAG. Materializing a flat candidate clears any stale projection on the working draft.
3. Schema-bound controlled Conversation seeds are no longer automatically replaced by a flat legacy localization template in feasibility/default candidate synthesis. Generic Expert workflows without that Schema binding keep their existing template behavior.
4. Conversation passes the exact Schema task and a label when the Schema has exactly one label to the existing Registry refinement candidate generator. No hardcoded SAM brand, checkpoint, object name or label. Existing availability/geometry-path checks determine whether a prompted-segmentation candidate can be produced.
5. Controlled seed selection uses the filtered approved Registry profiles. When a Provider profile has no legacy Runtime descriptor, an unbound controlled grammar is bound through those profiles instead of selecting an unrelated global/mock detector. Registry binding changes synchronize the authored LabelPipeline's model IDs with compiled nodes; exact ModelProfile locks and model-instance identities remain on the original nodes.

## Existing API/field contract

No new endpoint or request field. `T/workspace.builder_operations.items[]` remains `{operation,session,schema_id,schema_revision}`. `item.session.plan_candidates[].label_pipeline` is now an optional/null full controlled composition. `item.session.builder_proposal.draft.label_pipeline` is the actual selected composition; `draft.nodes[]` retains `model_profile_binding.model_profile_id` and/or `model_binding="model-instance:UUID"`.

Do not infer successful sample execution from Builder status. `operation.evidence.samples_tested=false` remains truthful. Sample preview/approval still binds exact draft revision/hash and resolved remote/native snapshots. Unsupported/flat/unauthorized plans still fail `sample_operations::validate_scope` before inference. Installed SAM alone is not evidence the selected candidate used it; inspect the saved segment node and execution results.

Old completed operation `c13a818e-da33-4c8d-9d6f-8354b68aae43` is immutable under its original operation ID. Integrating this commit does not rerun it, rewrite its Draft, or reset its grant. A new reviewed Builder/Journey operation and fresh exact scope are required to generate/test a new plan using the existing authorization flow. This agent has not issued that paid operation.

No SQL migration: existing Agent Session JSON accepts the additive nullable candidate field. Old candidate JSON without it deserializes with None; arbitrary old DAGs are not converted retroactively.

## Validation and limits

Extended isolated candidate regression verifies a Schema-bound refinement candidate survives SQLite save/read and materialization into an empty working draft, retains the Schema/working identity, ModelProfile and model-instance bindings, includes geometry evaluation, and becomes unavailable when its selected segmenter is disabled. The native fixture is capability/availability metadata only; it does not load a checkpoint or claim actual segmentation quality. Existing tests retain ownership, exact approval and replay behavior.

Commands: `CARGO_TARGET_DIR="$PWD/target" cargo test -p annotagent-core -p annotagent-application -p annotagent-storage -p annotagent-server --offline`; `cargo clippy --workspace --all-targets --offline -- -D warnings`; `cargo fmt --all --check`; existing `http_fixture.py --enable-fixture --smoke` for isolated real HTTP regression. Final counts and evidence are recorded in delivery.

This does not force every Builder choice to use refinement or guarantee model quality. Multi-label Schema keeps its full controlled composition, but this change does not invent automatic per-label refinement planning: the existing single-target refinement generator is entered only for one exact label. Explicit Expert/legacy graph edits can still produce plans rejected by guided sampling. Real installed VLM+SAM inference verification is owned by the integration agent under its separate user authorization.
