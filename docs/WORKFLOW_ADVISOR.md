# Workflow Advisor

`WorkflowAdvisor` receives only a bounded Project Schema, enabled Skill versions, registered
node/model catalogs, registered Validator/Refiner/resource IDs, a small dataset profile, and
operator constraints for cost, latency, accuracy, and review.

The default HTTP Advisor now runs a persisted `AgentSession`, not a one-shot template wrapper. Its
observable tool sequence is:

```text
inspect_project_schema → list_skills → list_skill_capabilities → list_models → list_resources
→ propose_draft → static_validate → revise_draft → static_validate → dry_run
→ inspect_metrics → request_publish_approval
```

The deterministic offline policy intentionally makes the first proposal invalid, consumes the
static validation result, and revises it. This proves the loop is iterative. Each call and result is
stored with a unique call id in SQLite. Rust enforces step/tool/token/cost budgets and cancellation.
The successful terminal state is `waiting_for_human`, never Published.

The offline Mock Advisor deterministically builds a valid registry-bound Draft and is the release acceptance path. The optional workspace LLM Advisor receives a safe base Draft and one strict `submit_workflow_advice` action. It may choose only registered bindings and review gates; it cannot emit code, Shell commands, URLs, unknown tools, or an executable/published Workflow.

Every suggestion returns a Draft, rationale, unresolved model bindings, warnings, alternatives,
the Agent trace, validation and sandbox Dry Run. Suggestions are persisted as editable Drafts and
must pass the same Rust static validator and explicit human Publish action as hand-authored
Workflows. LLM output is untrusted and cannot bypass this boundary.

The Web editor supports blank, Skill-template, Mock Advisor, and optional LLM Advisor entry points. Dry Run executes selected workspace images in an isolated sandbox and reports per-node Artifact classes, latency, cost, and issues without writing annotations.
