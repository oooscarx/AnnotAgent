# RoboCup Ball Domain Skill

Load this resource only for a Project that enabled `robocup.ball`. The Skill treats the generic
DetectionSet as untrusted candidates, runs the ball hard-negative and optional field-relation
Validators, and routes risky candidates to bounded recovery, crop verification, or human review.
It never implements detection, crop, model loading, storage, or Commit.

Normal high-confidence candidates with clean deterministic evidence stay on the fast path.
White footwear, penalty marks, line intersections, duplicate boxes, unusual geometry, missing
and learned project corrections increase risk.

The hybrid template requests `object_detection`, `open_vocabulary_detection`, and
`classification` capabilities. Project configuration resolves those capabilities to Registry
model ids; this Skill does not name or call a detector brand. Open-vocabulary candidates with no
score retain `not_provided` score semantics and cannot pass a score-only gate.
