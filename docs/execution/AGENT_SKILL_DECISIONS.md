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

## D007 — Product views read the layered Registry and persisted Agent state

Skills, Project configuration, Advisor traces, Recovery history and Correction Memory are views of
the application registry/store rather than UI-owned fixtures. Project Skill edits persist manifest
identifiers and declared dependencies. Review requires an enabled Skill that contributes a
correction taxonomy. Agent cancellation is scoped to one persisted session and clears any pending
human action when the session becomes terminal.

Rejected: hard-coded domain cards, fabricated Agent steps, UI-only cancellation, implicit global
correction ownership and exposing hidden model reasoning as a product trace.

## D008 — A failed Dry Run returns the Draft to human editing

When sandbox samples fail, the Advisor may apply one bounded retry-policy hardening to a registered
model node, records that revision as an observable tool action, saves the Draft as Editing and stops
for `edit_failed_dry_run`. It does not request publication approval. A successful Dry Run still
stops at the normal explicit publish boundary.

Rejected: treating a warning-only metadata change as a Draft revision, repeatedly increasing
Agent steps, auto-publishing a failed Dry Run or hiding the failure behind a larger global budget.
