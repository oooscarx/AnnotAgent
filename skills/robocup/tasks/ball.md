# Football candidates

Submit bounding boxes only for the football, using label `ball`. A small white region beside a
robot's lower body is a hard negative and must be locally rechecked. Penalty marks, socks, shoes,
robots, people and white-line intersections are context only and must never be emitted as labels.
