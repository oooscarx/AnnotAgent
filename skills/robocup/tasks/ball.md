# Football candidates

Submit bounding boxes only for the football, using label `ball`. A small white region beside a
robot's lower body is a hard negative and must be locally rechecked. Penalty marks, socks, shoes,
robots, people and white-line intersections are context only and must never be emitted as labels.

Whole-image VLM boxes are coarse search proposals. For a 10–20 pixel football, re-localize inside
an enlarged crop from the original image and return crop-relative coordinates, then project them
back to the root image. A segmentation Refiner may tighten geometry only after independent prompt
coverage is `Covered`; otherwise return the best candidate for Review or an empty result.
