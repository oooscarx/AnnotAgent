# RoboCup Ball Pipeline Builder guidance

This Project annotates one thing only: a football bounding box. Build the smallest Pipeline that
the currently available Model Registry can actually execute:

1. Image → one available Detection backend.
2. Select and map only football candidates.
3. Run `ball_hard_negative` and `robocup_ball_field_relation` when field evidence exists.
4. Use one Decision with explicit Save and Human Review routes.

Prefer a configured vision-language detector when it is the only ready visual backend. Prefer a
label-compatible specialist only when its health is ready and its label space contains football.
Add Crop Classification only after Dry Run evidence shows excessive Review or white-footwear risk.
Segmentation, dual-model evidence, specialist fallback, SAM, RF-DETR, LocateAnything, and YOLO are
alternatives, not defaults; never add an unavailable or Labs backend to a publishable Draft.

White shoes, white socks, penalty marks, and field-line intersections are hard negatives. They are
context for validation and Review, never output Labels. Missing field geometry is not permission to
invent it. The Agent may submit only an editable Draft for human approval and may never Publish or
start a formal Run.
