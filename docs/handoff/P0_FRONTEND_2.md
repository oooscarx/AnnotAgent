# P0 Frontend 2 handoff

## Result surface

`P0ResultPanel` is the single F2-owned right-side result surface. It consumes a versioned server projection supplied by Frontend 1 and reuses `DeliveryReview`, `DeliveryPackage`, and `AnnotationCanvas`.

Modes are explicit and write targets do not overlap:

- `preparing`: compact real stage, message, and elapsed time; no empty review form.
- `sample_feedback`: real Task/Sample/Artifact terminal candidates, read-only geometry and Sample feedback reference.
- `formal_review`: this Task's Processing → Batch → child Run result and formal review commands.
- `diagnostic`: read-only evidence with distinct capability, transport, unknown, structure, legal-empty, and projection categories.
- `package`: the existing server readiness/status/history/download component.

The component emits no progression command. Journey continuation remains Rust-owned.

## Safety behavior

- A server focus can select an exact image/candidate once per `result_revision`. It does not repeatedly override later user selection.
- Formal editing and image decisions are enabled only by explicit server actions in the P0 projection.
- Viewing a candidate never marks it accepted.
- A legal empty result explicitly says it is not a human negative confirmation and offers no synthetic candidate.
- Sample feedback and formal review continue to use separate identities and endpoints.
- Existing dirty guards, stable command retry IDs, stale snapshot checks, and child Run ownership remain in `DeliveryReview`.

## Integration requirements

Frontend 1 must provide the current P0 projection and inject the existing `DeliveryService`. It must not infer actions from HTTP method availability, mount both the legacy result form and `P0ResultPanel`, or use GET/refresh to trigger Journey execution.

Backend still needs to deliver the persistent automatic Journey wake-up and authoritative result projection. Required evidence includes stable focus ordering, terminal Sample candidates, explicit actions, and diagnostic categories. Until then the component tests are UI evidence only, not proof of the one-consent autonomous HTTP path.

## Verification

- Three Sample images with multi-class/multi-object/empty terminal results render in one surface.
- Exact risky candidate is selected once; rerender does not steal a subsequent manual selection.
- Missing actions keep edit/create/object/image decisions disabled.
- Preparation does not mount the review form.
- Legal empty result is not presented as an accepted negative.
- Existing Delivery review/package regression suite remains green.
