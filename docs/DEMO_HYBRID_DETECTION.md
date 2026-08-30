# Five-minute Hybrid Detection Demo

This demo uses only tracked Mock models. It needs no API key, GPU, network, checkpoint or external
Worker and demonstrates protocol behavior rather than model accuracy.

## Before the demo

```bash
cargo build --workspace --all-features
npm --prefix web run build
cargo run -p annotagent -- serve --workspace ./workspace --open
```

Import `examples/robocup-ball-hybrid-mock/project.yaml` as a Project or use its configuration when
creating the RoboCup Ball Project. Create the
`robocup.ball.specialist_with_open_vocab_fallback` Draft, test it, then publish it.

## Script

**0:00–0:30 — Problem.** Explain that open-vocabulary detection solves cold start without
target-class training data, while a trained specialist is cheaper once labelled data exists.

**0:30–1:00 — Architecture.** Show Project Labels, the generic Object Detection and Open Vocabulary
capabilities, Model Registry bindings, the RoboCup Ball Domain Skill, and the immutable Workflow
Version. Point out that model brands are Backends rather than Core nodes.

**1:00–1:30 — Cold start.** In New Project, show `Find objects by description` and the explicit
Worker-setup requirement. Do not claim live availability when the Worker is disabled.

**1:30–2:00 — Specialist first.** Show the Recipe and Project-owned capability bindings. The
specialist's 0.92 Mock evidence takes the fast path without a fallback call.

**2:00–2:40 — Bounded fallback.** Use the tracked empty-specialist scenario. Recovery makes exactly
one open-vocabulary call, records why it was invoked, and routes unresolved evidence to Crop
verification / Review without failing the Run.

**2:40–3:20 — Independent evidence.** Open Results and Debug. Show source model, original boxes,
score semantics, agreement/IoU or conflict. The score-less source says confidence was not provided;
no aggregate score is fabricated.

**3:20–4:00 — Domain risk and Review.** Use the white-shoe scenario. Show the structured RoboCup
hard-negative reason, then choose a source box or edit manually. Save/Accept creates an auditable
revision and Correction Memory record.

**4:00–4:35 — Artifacts and Replay.** Inspect Image, DetectionSet, CandidateCluster, Crop and
Classification Artifacts. Replay from a downstream node and show preserved detector ancestors,
re-executed nodes, cache state, duration and cost. Sandbox Replay does not duplicate Commit.

**4:35–5:00 — Reliability.** Show the no-key seven-scenario manifest and the 100-image persistent
Mock Batch test with pause, application restart and resume. Finish on Models, where versions,
checkpoint/license metadata and unavailable Worker status remain truthful.

## Live-conditional extension

Real LocateAnything and RF-DETR smokes require a supported NVIDIA environment, legally obtained
local weights and complete immutable metadata. When those prerequisites are absent, show the exact
`LIVE-CONDITIONAL` record in the acceptance ledger. Never substitute Mock boxes for a live result.
