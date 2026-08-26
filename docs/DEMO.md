# Five-minute Demo

## Preparation

```bash
cargo build --workspace --all-features
npm --prefix web install
npm --prefix web run build
cargo run -p annotagent -- init workspace/robocup-demo --skill robocup
```

Keep two terminals available. Start the GUI in one:

```bash
cargo run -p annotagent -- serve --workspace ./workspace --open
```

## Script

**0:00–0:30 — Problem.** Show the synthetic image. Explain the RoboCup failure modes: white shoe/penalty hard negatives, pixel-accurate field lines, local team-color evidence, and mixed geometry types.

**0:30–1:00 — Core + Skill.** Open the Skills page. Point out that tools, validators, refiners, task DAG, correction taxonomy and resources come from the registered Skill; Core remains domain-neutral.

**1:00–2:00 — Start offline Agent.** In the second terminal run:

```bash
cargo run -p annotagent -- demo robocup
```

Show that it uses Mock Provider yet records seven requests, input/output tokens, exact cost, typed events and SQLite history.

**2:00–2:40 — White shoe.** Highlight `possible_white_shoe` and `policy_retry` in output/history: the first false ball candidate overlaps the robot lower body and has strong white evidence; Runtime retries and the corrected ball is committed.

**2:40–3:20 — Field line.** Open the trace/history for `field_line`. Show refinement-started/completed and the revision before/after. The coarse `y≈0.47` line is moved toward the actual white pixels around `y≈0.50`.

**3:20–4:10 — Human review.** Start a configured low-confidence/real run if available, or use the server integration fixture. In Review, drag a shape/vertex, save the revision, select a Skill-provided correction reason, and accept/reject. Show revision history and the correction record’s influence on later review risk.

**4:10–4:35 — Audit and cost.** Show Agent Trace (visible messages/actions only), live SSE, model/tool/validator events, usage source, requests, token totals and exact decimal cost. Pause/resume/cancel from Project or TUI.

**4:35–4:50 — Export.** Run COCO or YOLO export and show `ExportReport`, including explicit skips/warnings for incompatible task kinds.

```bash
cargo run -p annotagent -- export \
  --project examples/robocup/project.yaml \
  --format coco --output ./exports/coco
```

**4:50–5:00 — Close.** Summarize: typed Agent Loop, deterministic RoboCup evidence, human revision/correction memory, offline repeatability, and a truthful Core/Skill boundary.

## Fallback

If the network provider is unavailable, do not change the story or use unrecorded output. Use `demo robocup`; it is deterministic, needs no GPU/key, and exercises both domain cases with Mock usage marked as such.
