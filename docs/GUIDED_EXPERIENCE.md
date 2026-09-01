# Guided Experience

AnnotAgent guides one concrete annotation Project from images to an exported dataset while keeping the typed Workflow, immutable Version, Run, Artifact, Review revision, and exporter contracts intact.

## Default journey

```text
Create Project
→ Data
→ Labels
→ Automation
→ Test & Activate
→ Run Dataset
→ Review results
→ Export dataset
```

The Project page is the hub. Its Guidance Hero comes from `ProjectWorkspaceSummary`, identifies the current stage, explains why it is current, lists blockers, and exposes exactly one primary action. React does not reconstruct this decision.

The eight Journey steps remain visible so a user can understand completed work and revisit an allowed earlier step. A direct Build URL is still server-gated: entering a later step cannot bypass missing data, Labels, Automation, or model configuration.

## Guided and technical surfaces

The default experience speaks about Labels, Automation, results, Crops, Review, and Export. It does not require Artifact IDs, Node IDs, graph hashes, registry internals, or Provider payloads.

Technical capability is preserved through progressive disclosure:

- Automation Recipe edits the same Draft as the collapsed Expert Graph.
- Run Results and Run Debug read the same immutable Run and checkpoint.
- Run Debug owns Node Inspector, payloads, lineage, Provider context, errors, and Replay.
- Advanced Project Details retains schemas, bindings, versions, Skill configuration, records, and usage.
- Settings owns Provider, Models, and Capabilities.

Expert Vision setup lives in **Settings → Vision Workers**. Guided setup shows endpoint trust,
discovery, model identity, selected-image conversion and the final registration checklist. Raw
contracts remain under expandable details; unavailable presets are setup choices, not executable
models. Run Results remain outcome-first while Debug/Expert retains the complete Prompt, Mask and
refined-geometry Artifact chain plus Replay.

For bounding-box Projects, New Project reads the Registry instead of asking for a free-text model
name. A compatible enabled specialist produces `Use your trained detector first`; otherwise the
cold-start recommendation is `Find objects by description` and explicitly says that no target-class
training data is required. Both recommendations create an editable Draft and never auto-publish.

Results and Review use user language while preserving technical truth: source models, independent
boxes, missing-score wording, agreement/conflict, fallback count and queue reason are visible.
Review can adopt a source model box through the normal revision flow. Debug retains raw typed
Artifacts, Cache state, lineage and Replay.

## State and recovery

Server and URL state are authoritative. Refresh restores Project Guidance, Build step, Run Results/Debug Image/Node/Artifact, Review item and scope, Runs filters, active execution, and Export readiness. An SSE reconnect always resynchronizes after an interrupted connection. A failed page request explains what failed and reloads the latest server state before retrying.

## Release accessibility contract

- At most one solid primary action appears in a page state.
- Equal first-screen metrics are limited to three.
- `.panel` containers do not nest.
- Every annotation canvas has an equivalent structured annotation list.
- Primary controls are native keyboard controls with a visible focus ring.
- Status always has text; color is supplementary.
- Reduced-motion preference suppresses meaningful transitions.
- The full path has no horizontal overflow at 1024 px or 720×450.

See [Project Guidance](PROJECT_GUIDANCE.md), [Guided Project Setup](GUIDED_PROJECT_SETUP.md), [Run and Review UX](RUN_AND_REVIEW_UX.md), and the [offline demo](DEMO_GUIDED_EXPERIENCE.md).

## Geometry-safe guided language

Run Results and Review present three separate facts: model/semantic score, Box quality and Geometry
verification. Guided Mode says “Needs geometry check”, “Refine box” and “Review uncertain box” rather
than exposing internal enum names. Automation shows a blocking repair card when an uncalibrated box
would be accepted from score alone. **Require human review** creates a safe Draft; **Add compatible
refiner** opens evidence-driven improvement; **Run geometry calibration** opens the exact calibration
workflow. Expert details retain contract revisions, config hashes, quality reports and Artifact
lineage.
