# Run and Review UX

## Run Results

A Run opens in **Results**. The first level answers how many images were processed, how many results were found, and how many need Review. It then shows the Image Browser, Label totals, and Original/Result/Compare/Crop views.

Bounding boxes show Label and confidence. Detection and Crop Artifacts preserve parent references, so selecting a box selects its Crop and selecting a Crop selects its source box. Downstream pass-through Artifacts are geometrically de-duplicated for presentation without changing checkpoint history. A keyboard-operable `Run result annotations` list is equivalent to the overlay.

A valid empty inference says **No target found**. It is not presented as a failure.

## Run Debug

Switch explicitly to **Debug** to see Pipeline Steps, selected Node inputs and outputs, configuration, latency, usage, recorded error, redacted Provider context, and Replay. `view=debug`, `image`, `node`, and `artifact` are URL state. Replay begins at the selected node and reuses valid upstream checkpoint work.

## Review Inbox

Review is a server-owned queue, not a client-side removal list. It shows reviewed/total/remaining progress, the editable Canvas, why the result needs a decision, confidence, source Run, Automation Version, and source step.

- `A` accepts and advances.
- `R` opens controlled Reject reasons, then rejects and advances.
- `E` opens annotation editing.
- `Space` toggles Original/Result.
- Arrow keys move through the queue or selected geometry where applicable.

Form controls retain their native keyboard behavior. Skill-specific Reject reasons appear only when that Skill is enabled for the Project. Edits append revisions and explain how the correction affects future Review routing.

The source Run and Review item link in both directions. After the final Project-scoped decision, the completed Inbox persists across reload and offers **Continue to export**.
