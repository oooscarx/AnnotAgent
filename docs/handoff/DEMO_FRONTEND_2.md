# Demo Frontend 2 handoff

## Scope and commits

- Role branch: `codex/frontend-2-delivery`
- Shared integration baseline supplied by Frontend 1: `8a7d8007904a00af62f3b179c216b8a3b2732a1d`
- This role branch diverges from that baseline at `d3220cb54bd50589efed86b6c0753bf4ea8db0db`; Frontend 1 must cherry-pick the listed F2 commits rather than merge the branch.
- First Demo component seam: `551a9420d7f7b0e44a31ebefaed3a758b6eb72d0`
- No `App.tsx`, HttpAdapter, public route/type/API, global CSS, Rust, Demo manifest, or licensed image content is changed here.

## Registration seams

`DemoReviewPanel.tsx` exports:

- `DemoReviewPanelProps { projectId, taskId, reviewId?, onOpenArtifact?, onReady? }`
- `createDemoReviewPanel(service)` for Frontend 1 to register an injected HTTP-backed service.

`DemoDeliveryPanel.tsx` exports:

- `DemoDeliveryPanelProps { projectId, taskId, deliveryId?, onReady?, onDownload? }`
- `createDemoDeliveryPanel(service)` for Frontend 1 to register an injected HTTP-backed service.

The service types live in `deliveryService.ts`. `GET` reads are explicit and abortable. `DeliveryReview` remains the only editor and formal review writer. `DeliveryPackage` remains the only readiness, consent, package-status, cancellation, history, and download presenter. Neither Demo panel starts inference, accepts an object, confirms an image, authorizes packaging, or creates a package on mount.

## Source semantics

The view distinguishes three persisted origins:

- `preset_candidate`: “预置候选 · 无本次模型推理”
- `live_model_prediction`: “本次模型预测” plus the server model display name when supplied
- `human_revision`: “人工修订” plus the server actor display name when supplied

Origin is presentation evidence only. It never changes `review_status`. Preset import must still return `needs_review`; clicking Start Demo is not represented as an acceptance or whole-image confirmation.

The Backend HTTP contract currently fixes `source_mode` as `preset_candidates | live_model` and preserves Task processing operation → Batch → child Run for formal results. Exact Demo review/delivery read endpoints and package manifest projection are still pending the Backend implementation commit; Frontend 1 must not invent routes or fall back to Fixture success.

## Evidence matrix

| Scenario | State | Evidence |
|---|---|---|
| Six server-owned images render | Passed (Fixture component) | `demo-delivery-panels.spec.ts` |
| Preview and full image URL remain the corresponding image | Passed (Fixture component) | same test; original URL is passed unchanged to `AnnotationCanvas` |
| Pending-review, negative, excluded, failed and complete state vocabulary | Passed (unit/component) | `demoDeliveryPresentation.test.ts`, panel status rendering |
| Preset origin is not shown as model inference | Passed | unit + browser assertion |
| Live prediction and human revision have distinct labels | Passed (unit) | `demoDeliveryPresentation.test.ts` |
| Panel mount/Start does not write review | Passed | `confirmWrites: 0` browser assertion |
| Formal reads use the Task-bound child Run | Passed | `delivery-review.spec.ts` |
| Sample feedback never writes formal review | Passed | `delivery-review.spec.ts` |
| Box geometry edit sends changed normalized coordinates | Passed (Fixture component) | `demo-delivery-panels.spec.ts` |
| Category edit is submitted with the object command | Passed (Fixture component) | same test |
| Missing object uses the existing create-object command | Passed (Fixture component) | same test |
| Object save does not confirm the whole image | Passed | same test, `confirms: 0` |
| Positive review advances only after save receipt | Passed | `delivery-review.spec.ts` |
| Negative sample requires an explicit whole-image command | Passed | `delivery-review.spec.ts` |
| Exclusion requires a reason and explicit command | Passed | `delivery-review.spec.ts` |
| Review-complete counts remain separate from package admission | Passed | `deliveryReviewState.test.ts`, `delivery-package.spec.ts` |
| Authorization does not scan images or create a package in React | Passed | `delivery-package.spec.ts` |
| Active package cancellation does not create a replacement | Passed (Fixture component) | `demo-delivery-panels.spec.ts` |
| Refresh is read-only | Passed (Fixture component) | package tests |
| Old immutable package can still be selected after a new package exists | Passed (Fixture component) | `delivery-package.spec.ts` |
| Failed/cancelled package has no download | Passed (Fixture component) | Demo cancellation test |
| Ready Demo receipt missing frozen source/version/hash/review evidence is not downloadable | Passed | unit + browser test |
| Ready card shows image/object scope, labels, split counts, source, bytes and hash | Passed (Fixture component) | Demo Ready browser test |
| Six actual licensed images are unique and decode | Blocked on PM pack + Backend catalog delivery | Must use the packaged asset endpoints, not this Fixture |
| YOLO coordinates round-trip against original dimensions | Blocked on Backend Rust package test | UI only proves changed normalized coordinates reach the service seam |
| Fixed train/val members contain no duplicates | Blocked on Backend Rust package test | Counts alone are not claimed as membership proof |
| Explicit negative image produces empty label file plus review evidence | Blocked on Backend Rust package test | UI sends `negative_confirmed` correctly |
| Edited box changes YOLO coordinates and creates a new ZIP/hash | Blocked on Backend Rust package test | UI does not generate ZIP or auto-start a second package |
| Unicode/space paths unzip and package contains no credential/absolute path | Blocked on Backend Rust package test | Must inspect actual generated ZIP |
| Real HTTP refresh/cancel/download owner checks | Pending integration | Run after Backend route commit and Frontend 1 HttpAdapter registration |

## Expected package evidence

For Demo delivery, a `ready` phase alone is insufficient to expose download. The persisted result must also carry:

- non-empty ZIP SHA-256, byte size, and image count;
- Demo id, version, and data SHA-256 matching the frozen task;
- matching `source_mode` and `live_inference_occurred` receipt;
- non-empty label mapping;
- split counts whose sum equals the packaged image count;
- at least one human review source.

This is a display/admission guard, not a substitute for Rust ZIP validation. Original hashes, member-level split uniqueness, YOLO coordinate bounds, negative label contents, and path safety remain Backend evidence.

## Commands run

```text
cd web
npm run typecheck
npm test -- --run src/agent-ui/demoDeliveryPresentation.test.ts src/agent-ui/deliveryReviewState.test.ts
npx playwright test --config playwright.ui-preview.config.ts e2e/ui-preview/demo-delivery-panels.spec.ts e2e/ui-preview/delivery-review.spec.ts e2e/ui-preview/delivery-package.spec.ts
```

Fixture/component tests prove UI behavior and fail-closed semantics. They do not prove real model accuracy, six-image licensing, Rust package contents, or live HTTP ownership.
