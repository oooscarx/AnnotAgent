# Workflow Advisor

`WorkflowAdvisor` receives only a bounded Project Schema, enabled Skill versions, registered node/model catalogs, registered Validator/Refiner/resource IDs, a small dataset profile, and operator constraints for cost, latency, accuracy, and review.

The offline Mock Advisor deterministically builds a valid registry-bound Draft and is the release acceptance path. The optional workspace LLM Advisor receives a safe base Draft and one strict `submit_workflow_advice` action. It may choose only registered bindings and review gates; it cannot emit code, Shell commands, URLs, unknown tools, or an executable/published Workflow.

Every suggestion returns a Draft, rationale, unresolved model bindings, warnings, and alternatives. Suggestions are persisted as editable Drafts and must pass the same Rust static validator and explicit human Publish action as hand-authored Workflows. LLM output is untrusted and cannot bypass this boundary.

The Web editor supports blank, Skill-template, Mock Advisor, and optional LLM Advisor entry points. Dry Run executes selected workspace images in an isolated sandbox and reports per-node Artifact classes, latency, cost, and issues without writing annotations.

