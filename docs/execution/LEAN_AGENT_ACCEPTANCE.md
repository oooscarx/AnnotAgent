# Lean Agent Alpha Acceptance Evidence

Status values: `PASS`, `OPEN`, `LIVE-CONDITIONAL`, `NOT-IN-SCOPE`.

## M0 baseline

| Requirement | Status | Evidence |
|---|---|---|
| Git and remote inspected without mutation | PASS | `main...origin/main`; origin remains `git@github.com:oooscarx/AnnotAgent.git`. |
| Master task stored in repository | PASS | `docs/execution/LEAN_AGENT_MASTER_PROMPT.md`. |
| Full Rust baseline | PASS | `cargo test --workspace --all-features`: 221 passed, 0 failed on 2026-08-31. |
| Existing runtime capability inventory | PASS | `docs/LEAN_ARCHITECTURE_MIGRATION.md` records preserved contracts and compatibility policy. |
| Duplicate product concepts inventoried | PASS | Migration document records Select detections, Decision, Combine model evidence and Automation. |
| Unavailable backends inventoried | PASS | Ports 8790–8792 unavailable; registered external workers remain disabled/unconfigured. |

## Release matrix

- A. Architecture subtraction: PARTIAL. Public Skill API exposes exactly Classification,
  Detection and Segmentation Capability Skills. Model-branded adapters are hidden compatibility
  entries. Model API groups optional SAM, YOLO, RF-DETR and LocateAnything under Labs while ONNX is
  absent. Guided node vocabulary and route consolidation remain M2.
- B. Agent authenticity: OPEN.
- C. Pipeline safety: OPEN.
- D. Offline capability: OPEN.
- E. UX: OPEN.
- F. RoboCup Domain boundary: OPEN.
- G. Course requirements: OPEN.

Evidence is added per milestone; an item is not marked PASS merely because a type or button exists.

## M1 Capability and Model convergence

| Requirement | Status | Evidence |
|---|---|---|
| Exactly three public Capability Skills | PASS | Server API test asserts `annotagent.classification`, `annotagent.detection`, `annotagent.segmentation`. |
| Legacy Projects remain resolvable | PASS | Compatibility aliases remain Registry entries but are filtered from the public API; all 28 Application tests pass. |
| Model brands are not public Skills | PASS | Open-vocabulary/VLM/YOLO manifests are compatibility-only; public API test rejects their appearance. |
| SAM/RF-DETR/LocateAnything/YOLO are Labs while unavailable | PASS | Model Binding `availability_group` plus server API assertions. |
| Segmentation does not claim a runnable model | PASS | Generic Segmentation Skill has no node/template until a compatible healthy backend exists; unit test passes. |
| Example Project migration | PASS | Detection and hybrid examples use generic Capability IDs; old inline fixtures continue to test aliases. |
| Rust M1 regression | PASS | 28 Application tests and 9 Server tests pass; Capability crate tests pass. |
| Web M1 regression | PASS | TypeScript passes and 35 Vitest tests pass. |
