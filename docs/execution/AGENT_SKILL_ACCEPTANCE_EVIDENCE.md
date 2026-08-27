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
