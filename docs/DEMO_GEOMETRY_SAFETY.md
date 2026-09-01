# Geometry Safety Demo

The deterministic release demo proves the safety boundary without claiming live model accuracy.

1. Create or open a generic bounding-box Project.
2. Configure a VLM Model Profile and inspect **Score and box quality**. VLM Detection shows semantic
   confidence, coarse geometry and “never from score alone”.
3. Ask AnnotAgent for a Pipeline. With no healthy prompted segmenter, the Draft includes mandatory
   Human Review.
4. Dry Run one image. Run Results shows model score, box quality and geometry verification as three
   separate facts.
5. Edit a box in Review and choose Too loose, Too tight or Shifted. The revision stores structured
   geometry evidence.
6. Open **Improve Automation**, select diagnosis Runs and different holdout Runs, create a Patch
   Draft, then run Before / After.
7. Inspect semantic recall, robust IoU/center-shift metrics, manual-resize and review rates, cost,
   latency, failures and small-object buckets.
8. Apply selected changes to an editable Draft. Publishing remains a separate human action.

Real Qwen, SAM and specialist accuracy tests are live-conditional. Offline protocol fixtures prove
contracts, conversion, safety and product flow only; they are never presented as real inference.
