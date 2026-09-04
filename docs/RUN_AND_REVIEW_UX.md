# Run and Review UX

## Run Results

A Dataset Run owns aggregate progress, controls, cost/usage, mixed per-image outcomes, and links to
its child Image Runs. An Image Run owns exactly one stable image and opens in **Results**. Its
canonical URL is `/projects/:projectId/runs/:runId?view=results&image=:imageId`.

Results consumes the server's explicit final projection: committed annotations, current
review-final candidates, valid No Target images, and failures. It never flattens intermediate VLM
boxes, detector candidates, refinements, masks, or fallbacks into annotations. Bounding boxes show
Label and typed confidence when available, and a keyboard-operable `Run result annotations` list is
equivalent to the overlay.

A valid empty inference says **No target found**. It is not presented as a failure.

## Run Debug

Switch explicitly to **Debug** to see Pipeline Steps, selected Node inputs and outputs, Detection
and Crop parent references, intermediate geometry/masks, configuration, latency, usage, recorded
error, redacted Provider context, and Replay. `view=debug`, `image`, `node`, and `artifact` are URL
state. Replay begins at the selected node and reuses valid upstream checkpoint work.

## Review Inbox

Review is a server-owned queue, not a client-side removal list. It shows reviewed/total/remaining progress, the editable Canvas, why the result needs a decision, confidence, source Run, Automation Version, and source step.

Project Review routes verify the persisted owner on the server. Opening a global Review result
enters `/projects/:projectId/review/:reviewId`; a foreign Project ID cannot move or reveal an item.
Run-to-Review and Review-to-Run links retain the stable Project, Image, and return context.

- `A` accepts and advances.
- `R` opens controlled Reject reasons, then rejects and advances.
- `E` opens annotation editing.
- `Space` toggles Original/Result.
- Arrow keys move through the queue or selected geometry where applicable.

Form controls retain their native keyboard behavior. Skill-specific Reject reasons appear only when that Skill is enabled for the Project. Edits append revisions and explain how the correction affects future Review routing.

Local geometry, note, and reason edits are keyed by Review item. Switching items or leaving with
unsaved edits requires an explicit discard decision. Choosing a detector source box changes only
geometry; the source score and its semantics remain provenance rather than overwriting generic
annotation confidence. After the final Project-scoped decision, the completed Inbox persists
across reload and offers **Continue to export**.
