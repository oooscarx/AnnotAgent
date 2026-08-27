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
