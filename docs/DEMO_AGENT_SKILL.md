# AnnotAgent Agent + Skill Alpha Demo

This is the stable, offline five-minute course demo. It uses only compiled Rust, Mock model
backends, typed Artifacts and an in-memory SQLite correction store. It does not require or read an
API key.

## 1. Generic whole-image Classification

```bash
cargo run -p annotagent -- demo generic-classification
```

The command loads `classification@1`, passes a typed Image subject to the Mock Classification
backend and prints the resulting label, confidence and subject reference. Expected terminal state:
`status=completed` and one model call.

## 2. Generic detection and Core Crop

```bash
cargo run -p annotagent -- demo generic-detection-crop
```

The command loads `yolo-detection@1`, produces a DetectionSet and then calls the domain-neutral
`core.crop` node. The printed CropSet includes its parent DetectionSet Artifact, parent detection
item, pixel dimensions and deterministic cache key. This proves that Crop is not owned by YOLO.

## 3. RoboCup Ball Domain Skill

```bash
cargo run -p annotagent -- demo robocup-ball
```

The output presents four deterministic cases:

1. normal football: Validator pass → Gate → Commit on the fast path, with no Agent Session;
2. white-shoe hard negative: Validator risk → scoped Memory query → crop evidence → Reject;
3. penalty-mark hard negative: Validator risk → explicit Human Review;
4. correction adaptation: the first uncertain candidate requires Review, the operator rejection is
   written to Project/Skill/task/Label-scoped SQLite Memory, and the second candidate is rejected
   because matching Memory changes the Recovery decision.

For each case the terminal output shows the loaded Skill, Validator result, Memory count and impact,
final decision, observable Agent tool steps, token usage, cost and stop reason. Only typed tool
arguments/results are shown; hidden model reasoning is not recorded.

## 4. Product UI

```bash
cargo run -p annotagent -- serve --workspace ./workspace
```

Open the loopback URL printed by the server. The five-minute walkthrough is:

1. Skills: compare Capability Skills, the `robocup.ball` Domain Skill and the RoboCup Pack;
2. Project → Build: enable Skills, inspect dependencies, open Pipeline, run Workflow Advisor;
3. inspect the Draft-only proposal, validation, Dry Run, cost and observable Agent trace;
4. cancel the waiting Advisor and verify its pending action clears;
5. Runs: inspect deterministic node Artifacts and Replay;
6. Review: select an enabled Domain Skill correction reason;
7. Project Overview: inspect persisted Agent sessions and Correction Memory impact.

The GUI and TUI use the same `LocalApplication` and SQLite state. TUI commands relevant to this
demo are `/skills`, `/skills show <id>`, `/advisor`, `/advisor cancel <session-id>`, `/run`,
`/pause`, `/resume`, `/cancel`, `/memory`, `/history` and `/gui`.

## 5. Full offline release gate

```bash
./scripts/acceptance.sh
```

This performs the domain/secret scans, formatting, strict Clippy, all-feature workspace tests and
build, Web typecheck/tests/build, doctor and all three demos.

## Live-conditional checks

Real Qwen and a real out-of-process YOLO worker are conditional on operator-owned configuration.
They are not needed for the offline release gate, no key is stored in the repository, and no live
success is claimed by this demo. Browser interaction is a manual/product verification layer; the
Server/Web/TUI behavior also has automated tests.
