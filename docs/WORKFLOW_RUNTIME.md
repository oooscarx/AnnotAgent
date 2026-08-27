# Workflow Runtime

`PublishedDagExecutor` accepts only an immutable `PublishedWorkflowVersion` whose content hash matches its frozen snapshot. It schedules ready nodes in parallel waves and passes typed `VisionArtifact` values across checked ports.

Execution supports bounded per-node retry, timeout, declared fallback activation, conditional routes, cancellation, deterministic Artifact caching, HumanReview suspension, safe Commit, and JSON-serializable checkpoints. Built-in ImageInput, CandidateMerge, HumanReview, Commit, and Export semantics cannot be overridden by registering an operation with the same name. Commit accepts only validated or explicitly human-approved Artifacts.

The checkpoint contains node statuses and outputs, trace records, activated fallbacks, review approvals, and aggregate usage. Each trace records attempts, cache evidence, route, exact Artifact inputs/outputs, structured errors, timestamps, tokens, and cost. Resume verifies the Workflow hash and schedules only unfinished nodes.

The product image path uses `PublishedWorkflowRuntime` whenever an exact published version is selected. It persists the selected version before execution, runs the real generic DAG, then adds the executor checkpoint to the Run snapshot. The Dataset Coordinator carries the same selected version in the Batch snapshot and gives it to every child Run. A legacy single-Skill AgentRuntime remains only for Projects that start without a selected published version.

The lower-level AgentRuntime retains the standard model tool-call protocol: complete assistant tool-call messages are replayed, every call ID has exactly one ordered tool result, structured geometry reaches the model, and incomplete histories are rejected before another Provider call. Its task budgets separate model turns, tool calls, recovery turns, task timeout, Provider timeout, and retry count.

Product Review currently materializes a DAG review suspension as durable `NeedsReview` annotations and a terminal `CompletedWithReview` image Run. The executor-level approval/resume contract and checkpoint are implemented and tested; automatically resuming that exact product Run after a later human decision is not yet wired.

