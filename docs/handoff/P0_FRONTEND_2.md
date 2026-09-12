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
- The Sample review layer intersects rendered annotations with the canonical terminal-candidate identities. Legacy aggregate/coarse outcomes and stale references are hidden; an all-invalid projection becomes a read-only `projection_failed` diagnostic instead of an empty review.
- Formal editing and image decisions are enabled only by explicit server actions in the P0 projection.
- A Sample result with a different Project/Task identity is rejected before any image or candidate is mounted. A terminal candidate without a server-issued feedback reference remains visible but its feedback action is disabled.
- Viewing a candidate never marks it accepted.
- A legal empty result explicitly says it is not a human negative confirmation and offers no synthetic candidate.
- Sample feedback and formal review continue to use separate identities and endpoints.
- Existing dirty guards, stable command retry IDs, stale snapshot checks, and child Run ownership remain in `DeliveryReview`. A stale object save retains the edit and reuses the exact command payload on retry.

## Integration requirements

Frontend 1 must provide the current P0 projection and inject the existing `DeliveryService`. It must not infer actions from HTTP method availability, mount both the legacy result form and `P0ResultPanel`, or use GET/refresh to trigger Journey execution.

Backend still needs to deliver the persistent automatic Journey wake-up and authoritative result projection. Required evidence includes stable focus ordering, terminal Sample candidates, explicit actions, and diagnostic categories. Until then the component tests are UI evidence only, not proof of the one-consent autonomous HTTP path.

## Verification

- Three Sample images with multi-class/multi-object/empty terminal results render in one surface.
- A legacy intermediate coarse box cannot enter the terminal Sample annotation list; valid terminal candidates remain visible when mixed input is rejected.
- Exact risky candidate is selected once; rerender does not steal a subsequent manual selection.
- Missing actions keep edit/create/object/image decisions disabled.
- Preparation does not mount the review form.
- Legal empty result is not presented as an accepted negative.
- Existing Delivery review/package regression suite remains green.

Focused verification on the F2 branch:

- `npm run typecheck`
- `npx vitest run src/agent-ui/deliveryVisualSelection.test.ts src/agent-ui/P0ResultPanel.test.tsx` — 7 passed
- `npx playwright test --config playwright.ui-preview.config.ts e2e/ui-preview/p0-result-panel.spec.ts` — 5 passed

This is component/preview evidence only. A1/A2 actual HTTP autonomy, server-issued P0 projection wiring, and real ZIP evidence remain joint Frontend 1/Backend integration work and must not be reported as passed from these tests.

## Canvas first-result fit

The packaged P0 result pane exposed a previously scoped CSS defect: the dimension probe and responsive SVG rules only applied under the legacy `.native-review` container. In the P0 pane the probe image therefore rendered at its natural size and visually covered the fitted SVG, making the first result look cropped at 100%.

- `DeliveryReview` now imports its own domain stylesheet instead of relying on another screen to load it.
- The probe remains a 1×1 transparent measurement element in every Delivery review surface.
- The annotation SVG uses the available review width and a bounded viewport height, preserving source pixels and normalized geometry.
- A newly selected image resets to Fit. Polling or rerendering the same image does not reset user zoom/pan.
- The focused browser suite checks the 1440px start, a 720px viewport equivalent to 200% layout pressure, 390px mobile width, and zoom persistence across rerender. It does not claim native browser chrome zoom automation.
