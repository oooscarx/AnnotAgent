# AnnotAgent Lean Agent Alpha — five-minute course demo

This demonstration shows a bounded Pipeline Builder Agent designing a football bounding-box
automation. The LLM/policy chooses registered actions; Rust owns every Draft mutation, validation,
Dry Run and audit record. The Agent cannot Publish or start a formal Run.

## Before presenting

From the repository root, run the deterministic preflight:

```bash
cargo run -p annotagent -- demo lean-agent-robocup
```

The command creates an isolated temporary Project with three labelled synthetic images. Its output
must say `offline ScriptedMock`, show the real Tool sequence, report `dry_run_images=3`, and end with
`published=false formal_runs=0 stop=waiting_for_human`. It deliberately raises the Review threshold
so the first Dry Run produces evidence for a Crop Classification revision. This is Mock evidence,
not a claim about real-world model accuracy.

Start the product separately:

```bash
cargo run -p annotagent -- serve --workspace ./workspace --open
```

If port 8787 is already occupied, keep the existing healthy process or choose another port with
`--port`. Never start a second server against the same workspace merely to continue the demo.

## 0:00–0:30 — the problem

Explain: visual annotation often needs a detector, label selection, domain rules and Human Review.
Users should describe the target and constraints, not manually assemble every technical node. The
published execution path must nevertheless remain deterministic and auditable.

Show `Project → Build → Automation`. Point out that AnnotAgent is the product; RoboCup is one
enabled Domain Skill, not a renamed application.

## 0:30–1:00 — the Project contract

Open or create a Project whose schema contains one bounding-box task and the Label `ball`. Import
three images. In Labels, show that the schema defines what the annotation means. Then return to
Automation to show that the Workflow separately defines how the Label is produced.

For the fully deterministic rehearsal, use the CLI preflight above. For the GUI, select
`ScriptedMock · offline evidence`; do not describe its boxes or quality as live inference.

## 1:00–1:30 — bounded objective and availability

In Pipeline Builder Agent, select `objects · bounding_box`, target `ball`, and a low desired Review
workload. Keep external APIs disabled for the offline demonstration. Explain that the Agent inspects:

- Project Schema and target Label;
- enabled Capability and Domain Skills;
- ready Model Backends in the Registry;
- the RoboCup Domain Advisor resource;
- cost, latency, model-call, Dry Run and human-boundary constraints.

Unavailable SAM, RF-DETR, LocateAnything and YOLO bindings remain Labs alternatives and cannot be
inserted into a publishable recommendation merely because their names are registered.

## 1:30–2:00 — first Draft

Click **Ask AnnotAgent**. Show the live persisted progress and expand **Tool actions**. The initial
lean recipe is:

```text
Image → one ready Detection backend → Select football → RoboCup Validators
      → Decision → Commit / Human Review
```

Point out `load_skill_resource`, capability/model listing and template creation. The default flow
does not add SAM, specialist fallback, Crop or a second detector.

## 2:00–2:25 — Rust rejects an invalid graph

In the Tool trace, show that the ScriptedMock policy disconnects one typed edge to create a genuine
invalid intermediate Draft. `validate_pipeline` returns the Rust Static Validator's issue. The Agent
repairs the exact typed connection and validates again. No client-side progress or error is invented.

## 2:25–3:00 — three-image sandbox Dry Run

Show the first `dry_run_pipeline` and `inspect_dry_run_summary` actions. The run is sandboxed: it may
write temporary Artifact evidence, but it writes no formal annotations and creates no formal Run.
The offline course fixture measures a Review rate on three labelled synthetic images.

## 3:00–3:30 — evidence-based revision

The strict fixture threshold makes the measured Review workload exceed the target. The Agent adds
the registered Crop Classification recipe, validates it, and performs a second three-image Dry Run:

```text
Detection → Select football → Crop → Classification → Attach result → Decision
```

Crop is still a Core node, not part of the detector. The revision is justified by the recorded Dry
Run metric rather than by a hard-coded RoboCup branch or a free-form LLM guess.

## 3:30–4:00 — inspect and apply a Diff

Show **Proposed Changes**, rationale, warnings, usage and the structured Draft Diff. Select a subset
or choose **Apply all**. Applying writes only the editable Current Draft. Demonstrate **Undo Agent
changes** if time permits. Rejecting or undoing does not delete the Agent audit.

## 4:00–4:20 — human activation boundary

The Tool trace ends at `submit_draft_for_human_approval` with `Ready for your review`. Open **Test &
Activate**, run the normal validation/Dry Run checks, and Publish manually. The resulting Workflow
Version is immutable. The Agent has no Publish or Start Run Tool.

## 4:20–4:40 — deterministic Runtime

Start a Run from the Published Version. Explain that every Run pins that exact version. Show node
Artifacts, lineage, latency, model usage and errors in Run Debug. Replay from a downstream node uses
the stored checkpoint and does not rerun completed ancestors.

## 4:40–5:00 — Review, audit and truthful boundaries

Open Review, adjust/accept a box, then continue to Export. Close on the boundaries:

- Pipeline Builder designs, validates, Dry Runs and revises an editable Draft.
- Rust Registry, grammar and Runtime constrain and execute it.
- humans approve publication and ambiguous annotations.
- ScriptedMock is offline labelled evidence; it is not Qwen, SAM or specialist inference.
- a real Qwen smoke test is separate and live-conditional on an operator-supplied local credential.

## Recovery cues

- **Agent budget stop:** increase the relevant bounded limit only after inspecting the Tool trace;
  turn and Tool Call budgets are independent.
- **Cancel:** use **Cancel Agent**. Refreshing the Project restores the terminal Session from SQLite.
- **Unavailable backend:** configure and health-check the Worker in Settings / Models; do not select
  a Labs binding in Expert Mode and present it as ready.
- **No proposal after refresh:** the durable Agent trace and Suggested Draft remain in history; the
  user must explicitly apply a Diff to the Current Draft.
