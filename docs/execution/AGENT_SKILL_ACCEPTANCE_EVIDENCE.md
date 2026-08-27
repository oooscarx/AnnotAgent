# AnnotAgent Agent + Skill Acceptance Evidence

## M0 — baseline and constraints

### Repository audit

- Start branch: `main`; start state: clean; ahead of `origin/main`: 11 commits.
- Latest starting commit: `2fe4de7 feat(product): complete guided workspace acceptance`.
- Remote was inspected read-only and remains `git@github.com:oooscarx/AnnotAgent.git`.

### Course constraints read before implementation

- [Requirements](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/requirements/): R1–R6 require
  Rust core logic, a working UI, configurable providers, interruptible live progress, persisted
  context/history, and exact usage/cost/budget tracking.
- [Quick start](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/quick-start/): implement and test
  incrementally, keep design aligned with reality, preserve Git history, and prepare a stable demo.
- [Agent architecture](https://lab.cs.tsinghua.edu.cn/rust/projects/agent/agent-architecture/): a real
  loop is model proposal → Agent-executed tool → tool result in history → another model turn, with
  final-answer, step, cancellation, token, and cost stop conditions. Skills should load detailed
  context on demand.

### Baseline checks

```text
scripts/check-agent-skill-boundaries.sh
cargo test -p annotagent-runtime tool_history_requires_one_ordered_result_per_call
```

Result: both checks passed on 2026-08-27. The boundary scan covered Core, Runtime, Server and the
generic Web component directory. The focused protocol test verified ordered multi-call results and
rejected missing or wrong-id results.

## M1 — layered Skills and multi-Skill resolution

Implementation:

- `crates/annotagent-core/src/skill.rs`: kinds, versions, dependencies, conflicts and contribution
  declarations;
- `crates/annotagent-core/src/traits.rs`: unified object-safe `Skill` contract;
- `crates/annotagent-runtime/src/registry.rs`: layered catalog/resolver and safe resource loading;
- `crates/annotagent-runtime/tests/skill_extension.rs`: independent Capability/Domain/Pack fixtures.

Verification:

```text
cargo test -p annotagent-runtime --test skill_extension
4 passed; 0 failed
```

The tests also retain the legacy external Domain Skill execution proof and deterministic namespace
merge proof.

## M2 — tool-call protocol and Artifact lineage

Verification:

```text
cargo test -p annotagent-core artifact
3 passed; 0 failed
cargo test -p annotagent-runtime engine::tests
4 passed; 0 failed
cargo test -p annotagent-provider \
  openai_compatible::tests::serializes_assistant_tool_call_history_for_follow_up_turns
1 passed; 0 failed
cargo test -p annotagent-runtime --test published_dag
6 passed; 0 failed
```

The DAG suite covers deterministic cache/replay, retry/fallback, timeout and cancellation. Trace
assertions validate strong output envelopes and exact Project scope. The protocol tests reject
duplicate, missing, unexpected, wrong-order and nested tool results and verify geometry-reference
hand-off.

## M3 — Classification Capability Skill

```text
cargo test -p annotagent-skill-classification
3 passed; 0 failed
cargo test -p annotagent-provider pipeline_backends::tests
4 passed; 0 failed
cargo test -p annotagent-skill-yolo --test label_pipeline_runtime
4 passed; 0 failed
cargo test -p annotagent-application \
  target_label_advisor_draft_is_editable_dry_runnable_and_publish_blocking
1 passed; 0 failed
```

Evidence covers Capability manifest/template discovery, whole-image single/multi-label output,
confidence verification, generic HTTP JSON, OpenAI-compatible subject bounding, crop parent
lineage, Commit and classifier-only Replay without rerunning the detector.

## M4 — Detection Skills and Core processing

```text
cargo test -p annotagent-skill-vlm-detection
1 passed; 0 failed
cargo test -p annotagent-skill-yolo
5 passed; 0 failed across unit and integration targets
cargo test -p annotagent-core label_pipeline
4 passed; 0 failed
cargo test -p annotagent-runtime --test published_dag
6 passed; 0 failed
```

The YOLO integration executes two overlapping Mock detections, applies class mapping, threshold and
NMS, commits one Detection, then verifies Crop dimensions/cache/parent lineage and classifier
Replay. Skill template tests assert that neither detector owns `core.crop`.

## M5 — iterative Workflow Advisor Agent

```text
cargo test -p annotagent-core agent
1 passed; 0 failed
cargo test -p annotagent-storage migration_creates_required_tables
1 passed; 0 failed
cargo test -p annotagent-application \
  iterative_advisor_revises_invalid_draft_and_stops_for_publish_approval
1 passed; 0 failed
cargo test -p annotagent-server \
  label_pipeline_http_advisor_dry_run_inspector_and_replay_are_real
1 passed; 0 failed
```

The application test asserts the complete 12-action loop, valid sandbox Dry Run, persisted session,
cancelled session, `waiting_for_human` terminal state, and zero published versions.

## M6 — RoboCup Ball Domain Skill and Pack

```text
cargo test -p annotagent-skill-robocup
10 passed; 0 failed across unit and integration targets
cargo test -p annotagent-runtime --test hybrid_workflow
3 passed; 0 failed
scripts/check-agent-skill-boundaries.sh
passed
```

The tests prove Pack/Domain separation, two templates without concrete model bindings, safe
resource rejection, structured white-shoe/penalty/memory issues, field inside/outside/missing
evidence behavior, and backward-compatible hybrid execution. The generic Runtime/Core/Server/Web
component boundary remains free of the forbidden RoboCup label terms.

## M7 — Correction Memory and adaptive Annotation Recovery

```text
cargo test -p annotagent-skill-robocup recovery
1 passed; 0 failed
cargo test -p annotagent-storage correction_memory_isolated_by_project_skill_task_and_label
1 passed; 0 failed
cargo test -p annotagent-application \
  recovery_uses_scoped_memory_persists_trace_and_keeps_clean_fast_path
1 passed; 0 failed
cargo clippy -p annotagent-skill-robocup -p annotagent-application \
  -p annotagent-storage --all-targets -- -D warnings
passed
scripts/check-agent-skill-boundaries.sh
passed
```

Direct evidence:

1. `SqliteStore::query_corrections` requires the exact Project UUID, Skill ID, Task ID and optional
   Label. A five-record fixture differing in each dimension returns only the one exact match.
2. A clean candidate returns `Accept` through `fast_path` and creates no Agent Session. Risky
   candidates run the separate Recovery loop and persist only observable tool names, arguments and
   results—never hidden reasoning.
3. The loop explicitly loads the declared hard-negative resource, inspects candidate/parent and
   Validator issue data, queries structured Memory, optionally runs the actual bounded
   `BallEvidenceTool`, compares evidence, and chooses accept, reject or Human Review.
4. The application test first receives Human Review for an uncertain candidate. After a scoped
   operator correction with reason `white_shoe_as_ball`, the same candidate is rejected and
   `memory_changed_decision` is true. Both risky sessions are persisted under the product Project.
5. Memory traces expose only controlled reason codes and timestamps. The operator note is not
   promoted into tool instructions or the session trace.
6. A zero-step/tool budget stops with `AgentSessionStatus::BudgetExceeded` and Human Review. A
   cancelled request similarly produces a cancelled session and never starts a new tool.
7. The Project-local image loader canonicalizes the path and rejects paths outside the Project
   root before any evidence tool receives pixels.

## M8 — Web, TUI and guided Agent/Skill UX

```text
cargo test -p annotagent-core agent
1 passed; 0 failed
cargo test -p annotagent-server skill_api_groups_layered_registry_contributions
1 passed; 0 failed
cargo test -p annotagent-server \
  label_pipeline_http_advisor_dry_run_inspector_and_replay_are_real
1 passed; 0 failed
cargo test -p annotagent-server \
  project_sse_review_revision_and_budget_flow_works_over_http
1 passed; 0 failed
cargo test -p annotagent tui::tests
5 passed; 0 failed
cargo clippy -p annotagent-runtime -p annotagent-storage -p annotagent-application \
  -p annotagent-server -p annotagent --all-targets -- -D warnings
passed
scripts/check-agent-skill-boundaries.sh
passed
cd web; npm run typecheck
passed
cd web; npm test -- --run
10 files and 24 tests passed
cd web; npm run build
passed
```

Direct evidence:

1. The Skill API exposes all three layered kinds and their nodes, tools, Validators, Refiners,
   policies, capabilities, dependencies, resources, templates and consuming Projects. An API test
   discovers a Domain Skill and its Capability dependency from the catalog, persists both into a
   generic Project and makes no Core/Server branch on a concrete domain label.
2. Project Build presents enabled Capability and Domain Skills as actual persisted controls. A
   dependency is selected automatically from manifest data; a legacy Pack stays on its explicit
   compatibility configuration until the operator chooses migration.
3. The Advisor HTTP test verifies a persisted 12-step Agent Session, static validation, isolated
   one-image Dry Run, explicit human publication boundary and cancellation. No Workflow Version is
   created by the Advisor.
4. Review accepts an explicit enabled Skill ID, rejects disabled Skills, persists a structured
   reason code under that exact Skill and returns it through the Project Correction Memory API.
5. TUI tests exercise `/skills`, `/skills show`, `/advisor cancel`, `/memory`, generic empty state,
   responsive terminal layout and non-color-only state labels using the shared application store.
6. On 2026-08-28 the in-app browser opened the real local product at
   `http://127.0.0.1:8791/projects/qwen-live`. The Skills screen grouped Classification, VLM
   Detection and YOLO as Capability Skills, RoboCup Ball as a Domain Skill and RoboCup as a Pack.
7. A fresh GUI Advisor run showed `Waiting for human`, 12 observable tool steps, zero validation
   issues, a one-image successful Dry Run, tokens/cost, explicit stop reason and
   `publish_workflow`. Clicking `Cancel Agent` changed the persisted state to `Cancelled`, disabled
   cancellation, set the stop reason to `cancelled by operator` and cleared Human action to `None`.
8. Agent trace details are restricted to typed arguments and results. The UI explicitly states and
   enforces that hidden chain-of-thought is not part of the trace.

## M9 — batch reliability, offline demos and Release Matrix

Final command run on 2026-08-28:

```text
./scripts/acceptance.sh
exit 0
```

The script passed, in order:

- Agent + Skill domain boundary scan and repository secret-prefix scan;
- `cargo fmt --all -- --check`;
- strict all-feature/all-target Workspace Clippy;
- 150 Rust unit/integration tests, with zero failures;
- all-feature Workspace build;
- Web typecheck, 10 test files / 24 tests, and production build;
- `annotagent doctor` with SQLite migrations, example Project and Web build present;
- `demo generic-classification` with `classification@1`, one whole-image ClassificationSet and
  completed Commit result;
- `demo generic-detection-crop` with `yolo-detection@1`, DetectionSet → Core Crop, exact parent
  detection item, pixel dimensions and cache key;
- `demo robocup-ball` with four cases and no external request.

The 100-image test `persistent_batch_pauses_restarts_and_resumes_one_hundred_images` passed. The
same suite also covers cancel preventing new nodes, transactional duplicate-start rejection,
startup requeue/reconciliation, checkpoint reuse, budget reservation, history, export and Replay.

The final Advisor regression test proves two independent revisions: Static Validator errors restore
registry bindings; a failed Dry Run changes the model node's bounded retry policy, moves the Draft
to Editing and stops for `edit_failed_dry_run` without requesting Publish approval.

### Release Blocking Acceptance Matrix

| Area | Status | Direct evidence |
| --- | --- | --- |
| A. Agent behavior | PASS | 12-step Advisor loop; static and Dry Run revisions; explicit approval/cancel/budget stops; Recovery evidence loop; typed trace only. |
| B. Skill architecture | PASS | layered registry dependency/conflict/resource tests; external dummy Skill; domain boundary scan; immutable Project/Run snapshots. |
| C. Generic capabilities | PASS | Classification whole image/crop, VLM Detection, YOLO Mock/HTTP, Core Crop parent lineage, shared detector once, generic Project tests and demos. |
| D. RoboCup Ball | PASS | Ball registry/Pack tests; white-shoe/sock, penalty/line, field relation/missing evidence tests; fast path and Recovery policy demo. |
| E. Correction Memory | PASS | exact Project/Skill/task/Label storage isolation, GUI impact view and four-case demo where the second decision changes. |
| F. Artifact correctness | PASS | ordered Tool history, reference-only model geometry, strong envelopes/provenance, Inspector/Replay/cache, SucceededEmpty, partial tasks and idempotent Commit tests. |
| G. Product | PASS | AnnotAgent brand, generic empty-state tests, layered Skills, enabled-Skill Project controls, Review taxonomy, Web/TUI Agent trace and cancellation; real browser verification. |
| H. Course requirements | PASS | Rust core, TUI, GUI, provider/context/reasoning/pricing configuration, SSE/progress/control/history/usage, three domain customizations and offline five-minute demo. Qwen/YOLO live checks are conditional as permitted. |

### Milestone commits through M8

| Milestone | Commit | Message |
| --- | --- | --- |
| M0 | `494958e` | `docs(agent): establish agent skill execution baseline` |
| M1 | `d72fbe2` | `feat(skills): add layered skill registry` |
| M2 | `eba7780` | `feat(runtime): enforce artifact envelope protocol` |
| M3 | `559c7e1` | `feat(classification): formalize capability skill` |
| M4 | `7298c9e` | `feat(detection): formalize detection capabilities` |
| M5 | `e975246` | `feat(advisor): add iterative workflow agent` |
| M6 | `ce52557` | `feat(robocup): add ball domain skill pack` |
| M7 | `2d75fd2` | `feat(recovery): add memory-guided annotation agent` |
| M8 | `51c54a9` | `feat(product): expose layered skills and agent sessions` |

M9 is the final local commit containing these demos, documents and Release Gate changes; its hash
is reported after the commit is created.
