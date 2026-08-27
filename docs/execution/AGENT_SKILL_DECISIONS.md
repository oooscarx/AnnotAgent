# AnnotAgent Agent + Skill Decisions

## D001 — Extend the working runtime

Keep the existing typed workflow, storage, provider and UI foundations. Add the layered Skill and
Agent abstractions beside compatibility adapters, then migrate composition code incrementally.

## D002 — Separate the two Agent loops

Workflow authoring and candidate recovery have different tools, budgets, side effects and stopping
conditions. They will use a shared generic loop protocol/state model but separate tool registries
and policies.

## D003 — Preserve deterministic control

Models propose actions. Rust validates arguments, references, permissions, budgets and state. Only
the application can publish workflows; only Runtime review/commit policy can write annotations.

## D004 — Progressive Skill resource loading

Registry listings expose compact summaries. Full resources are loaded by explicit Skill/resource
identifier, canonicalized under the Skill root, with traversal and undeclared-resource rejection.

## D005 — Compatibility is explicit

`DomainSkill` remains as an adapter during migration so existing RoboCup and Label Pipeline paths
continue to work. New architecture and UI use the layered `Skill` contract.

## D006 — Recovery Memory is scoped evidence, not instruction

Recovery retrieves only records matching the exact Project UUID, Skill, Task and Label. The Agent
trace receives controlled reason codes and timestamps, not free-form notes as instructions. A clean
candidate bypasses the Agent; a risky candidate that cannot finish because of cancellation or
budget is sent to Human Review.

Rejected: global similarity memory, cross-Project fallback, prompt injection through notes, and
turning an unfinished recovery into automatic acceptance.
