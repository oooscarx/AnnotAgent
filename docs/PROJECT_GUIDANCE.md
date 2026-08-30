# Project Guidance

Project Guidance is a deterministic Application projection over persisted Project truth. It is not a mutable frontend state machine.

## Inputs

`ProjectWorkspaceSummary` derives guidance from the Project and Dataset, Label Schema, editable and published Workflows, model bindings, latest Sample Test, active Run or Batch, terminal results, Review queue, and Export state.

## Priority

The engine chooses one `ProjectStage` and one `GuidedAction` using this order:

1. Active Run or Batch → open active work.
2. Missing data → add images.
3. Missing Labels → define Labels.
4. Missing Automation → choose Automation.
5. Missing model/configuration → repair the binding or configuration.
6. Untested Draft → test samples.
7. Sample failure or uncertainty → fix Automation or inspect the uncertain sample.
8. Tested Draft not published → activate Automation.
9. Published Version with no full Run → run the Dataset.
10. Pending Review → review results.
11. Completed and reviewed work → export the dataset.

The response also contains blockers, repair destinations, readiness facts, and eight ordered Journey steps. At most two secondary actions may supplement—but never replace—the one primary action.

## Ownership rules

- The backend decides business priority; the GUI decides presentation only.
- Active execution outranks setup or Export actions.
- A Project is not itself “Running” or “Failed”; those states belong to Runs and Batches.
- Sample Test evidence is persisted and remains a sandbox result.
- Export readiness is recalculated from the newest terminal Run per image and current Review truth.
- Global Runs and Review use explicit URL scope, never hidden Project memory.

The three read endpoints—`/guidance`, `/readiness`, and `/summary`—are projections of the same Application logic. The Web workspace uses `/summary` to keep Header, Journey, blockers, and action coherent.
