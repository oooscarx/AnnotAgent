# RoboCup Ball Domain Skill

Load this resource only for a Project that enabled `robocup.ball`. The Skill treats the generic
DetectionSet as untrusted candidates, runs the ball hard-negative and optional field-relation
Validators, and routes risky candidates to bounded recovery, crop verification, or human review.
It never implements detection, crop, model loading, storage, or Commit.

The Skill supplies versioned prompt resources for coarse football detection, local
re-localization, and crop verification. These describe a small, tight football target, enforce
crop-relative coordinates during local search, and explicitly reject shoes, socks, penalty marks,
field-line intersections, glare, robots, and turf. A Published Workflow freezes the resource
version and SHA-256 recorded in its node configuration and resource snapshot.

Normal high-confidence candidates with clean deterministic evidence stay on the fast path.
White footwear, penalty marks, line intersections, duplicate boxes, unusual geometry, missing
and learned project corrections increase risk.

The hybrid template requests `object_detection`, `open_vocabulary_detection`, and
`classification` capabilities. Project configuration resolves those capabilities to Registry
model ids; this Skill does not name or call a detector brand. Open-vocabulary candidates with no
score retain `not_provided` score semantics and cannot pass a score-only gate.

The small-object recovery template is model-agnostic. It uses Core expansion, crop, coordinate
projection, prompt-coverage, mask conversion, and geometry-decision nodes; the Registry must bind
real Ready VLM and prompted-segmentation models before publication. Segmentation runs only for a
Covered prompt. Partial, outside, or unknown coverage never invokes the Refiner and ends in a
bounded recovery route or Human Review.
