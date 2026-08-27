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
