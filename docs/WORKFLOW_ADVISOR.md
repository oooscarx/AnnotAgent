# Workflow Advisor

`WorkflowAdvisor` receives only a bounded Project Schema, enabled Skill versions, registered
node/model catalogs, registered Validator/Refiner/resource IDs, a small dataset profile, and
operator constraints for cost, latency, accuracy, and review.

The default HTTP Advisor runs the persisted Pipeline Builder `AgentSession`, not a one-shot template
wrapper. Its observable tool sequence is:
observable tool sequence is:

```text
inspect_project → inspect_label → list_enabled_skills → load_skill_resource (when declared)
→ list_available_nodes → list_available_models → create_draft_from_template
→ validate_pipeline → revise through registered tools → validate_pipeline
→ dry_run_pipeline → inspect_dry_run_summary → submit_draft_for_human_approval
```

The deterministic offline policy intentionally makes the first proposal invalid, consumes the
static validation result, and revises it. This proves the loop is iterative. Each call and result is
stored with a unique call id in SQLite. Rust enforces step/tool/token/cost budgets and cancellation.
The successful terminal state is `waiting_for_human`, never Published.

The offline ScriptedMock policy is the release acceptance path. The optional workspace LLM runs the
same multi-turn Tool Loop and receives a safe base Draft plus a closed Tool Registry. It may choose
only registered bindings, typed mutations and review gates; it cannot emit code, Shell commands,
URLs, unknown tools, or an executable/published Workflow. Declared Domain Advisor resources are
loaded through a traversal-safe Rust Registry call and are required before a Domain Draft is made.

Every suggestion returns a Draft, rationale, unresolved model bindings, warnings, alternatives,
the Agent trace, validation and sandbox Dry Run. Suggestions are persisted as editable Drafts and
must pass the same Rust static validator and explicit human Publish action as hand-authored
Workflows. LLM output is untrusted and cannot bypass this boundary.

The Web editor supports blank, Skill-template, Mock Advisor, and optional LLM Advisor entry points. Dry Run executes selected workspace images in an isolated sandbox and reports per-node Artifact classes, latency, cost, and issues without writing annotations.
