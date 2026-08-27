# RoboCup Ball Domain Skill

Load this resource only for a Project that enabled `robocup.ball`. The Skill treats the generic
DetectionSet as untrusted candidates, runs hard-negative and field-relation Validators, and routes
risky candidates to bounded recovery or human review. It never implements detection, crop, model
loading, storage, or Commit.

Normal high-confidence candidates with clean deterministic evidence stay on the fast path.
White footwear, penalty marks, line intersections, duplicate boxes, unusual geometry, missing
field evidence, and learned project corrections increase risk.
