# Recovery semantics and model-input repair

This log separates graph/runtime correctness from visual accuracy. Historical Drafts, Sample Tests,
Runs, Reviews, annotations and model records are preserved. A saved model prediction or exported
box is not treated as ground truth unless its human-review provenance and matching image revision
can be established.

## M0 — verified failure baseline

### Repository and scope

- Branch: `main`; the branch started this repair 21 commits ahead of `origin/main`.
- Pre-existing modified files are fourteen PNGs under `docs/execution/screenshots/`. They are not
  part of this repair and are not staged or overwritten.
- The active persistence database is `workspace/.annotagent/history.db`.
- The legacy zero-byte `workspace/.annotagent/annotagent.db` is not the active evidence store.

### Saved Sample Test identity

| Field | Verified value |
| --- | --- |
| Sample Test | `53c4c39d-cb9a-4e3d-9bea-79852a5d130a` |
| Draft | `183d793c-c8ce-433e-8fdc-d1bf81186725`, revision 2 |
| Workflow hash | `5db86c94acb29065b2e3a8004683bf3bfdb2b54820a5adec4638ce404c7f1b28` |
| Image-set hash | `2492440a3543d7b791b80a6aea60fccbcc905c0b2474bfcec2529759f8298122` |
| Model-snapshot hash | `fd267f7dd142d44b7ddf0829a2e20525d14f412ab29c87384917a023a8ab53e8` |
| Start / completion | `2026-09-06T06:13:59.864583Z` / `2026-09-06T06:14:03.484017Z` |

The input is image `dbe3d88d-ed0a-55af-b762-b5a1d33d3b35`, relative path
`images/color_1001525.png`, 544×448 pixels, SHA-256
`b0b470a93ed334a52462316029d1cf8794ce4287eaba37a2f551af3848043cfd`.
The PNG has no EXIF orientation value; the decoded orientation is therefore the stored 544×448
pixel raster, not a separately observed camera orientation.

### Frozen model and prompt inputs

- `coarse_localization`, `local_relocalization` and the misleadingly named
  `independent_crop_verification` all resolved to Model Profile
  `b9c5bbe8-e21a-5784-9c52-cade259b434f@2`, remote identity
  `qwen3.7-flash-2026-07-15`.
- `refine_validated_prompt` resolved to ready Model Instance
  `ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`, EfficientSAM-Ti ONNX, plugin
  `org.annotagent.efficientsam-onnx@1.0.0` and Bundle
  `org.annotagent.models.efficientsam-ti-onnx@1.0.0`.
- Coarse prompt: resource version `2.0.0`, SHA-256
  `5530dcdecf14a07f5bcfada7ae03660ad0013631ad0037ded6eac17b48bd9fce`.
- Local prompt: resource version `2.0.0`, SHA-256
  `c10efdd07415809ecbcff79f640ea619aaa293bbeb98b5f35972bb647ed09a9e`.
- Repeated crop-verification prompt: resource version `2.0.0`, SHA-256
  `7d8c2980b405cb1ab2f37e176c4b5f8a9983873d17d7c404e0f30230b57b9516`.

No secret, bearer header or image base64 is recorded here.

### Actual saved execution

1. Whole-image Qwen localization returned normalized `xywh`
   `[0.482, 0.510, 0.024, 0.028]`.
2. `core.expand_region` produced root ROI
   `[0.4057647, 0.41685712, 0.1764706, 0.21428572]`.
3. `core.crop` described a 96×96 search image.
4. Both local Qwen nodes consumed that same source crop. The application runner reconstructs a
   crop from `root_region` and calls `to_model_image` with the crop's own maximum edge, so the
   declared 96×96 input was submitted without an explicit 4× resize.
5. Projected local boxes were
   `[0.4830588, 0.46292856, 0.02152941, 0.03321429]` and
   `[0.48164704, 0.46292856, 0.03000000, 0.03321429]`. Their overlap is correlated
   same-model/same-view agreement, not independent evidence and not target truth.
6. `core.prompt_coverage_gate` selected `PartiallyCovered · ExpandAndRelocalize`.
7. The selected `relocalize` edge targeted `review_final_ball` directly. There was no search view B,
   no second localization and no recovery model input.
8. `refine_validated_prompt` was skipped because only the `refine` route reaches it.
   `refiner_usage_count=0`; this Sample Test contains no evidence about EfficientSAM's accuracy.
9. The terminal candidate remained the first projected local box and was marked `needs_review`.

At M0 the stored model-call trace does not retain an outbound-image digest, decoded submitted
dimensions, color format, interpolation, or transform fingerprint. The 96×96 conclusion is verified
from the saved Image Artifact dimensions plus the runner implementation, not yet from a persisted
provider-boundary trace. M2 adds that direct evidence.

### Reference provenance and visual mismatch

The active database contains no `annotations` or `annotation_revisions` for this image. The file
`workspace/robocup-ball/exports/yolo_detection/color_1001525.txt` contains an older exported box
with normalized center `(.504, .451)` and size `(.024, .032)`, but the export report does not retain
an annotation/revision ID or human-review state. The repository's preserved fixture explicitly sets
`ground_truth: null` and `accuracy_status: provisional_pending_human_calibration`.

Consequently, the low overlap between the current terminal box and that export is recorded only as
**a mismatch with an unverified historical export**. It is not reported as model accuracy or IoU
against ground truth.

### Regression test written before the fix

`relocalization_route_cannot_go_directly_to_review` in the RoboCup Ball Skill inspects the executable
template and rejects a `relocalize` route whose immediate target is Human Review. It is temporarily
ignored in M0 so the repository remains buildable while preserving a test that fails against the
unmodified behavior. M1 removes the ignore after implementing the second bounded search.

Command used to reproduce the red test:

```text
cargo test -p annotagent-skill-robocup relocalization_route_cannot_go_directly_to_review -- --ignored
```

Expected M0 failure: the route target is `review_final_ball`, a `HumanReview` node.

## Milestone map

- M1: machine-checkable route semantics and a real bounded search view B — completed below.
- M2: materialized Resize/model-input trace, shared coordinate transforms and correlated-evidence
  de-duplication — completed below.
- M3: separate refinement eligibility from automatic acceptance and pass the selected recovered
  candidate through SAM/geometry/review safely.
- M4: Builder inspection/validation tools, branch-injection tests, UI execution summary and fixed-set
  real evaluation. Engineering correctness and visual quality will be reported separately.

## M1 — explicit bounded recovery branch

Completed in code without modifying the saved M0 Draft or Sample Test.

### Graph and policy changes

- `WorkflowValidator` now checks every outgoing `relocalize` route. The source Gate must declare
  `recovery_route_policy` with `action=relocalize`,
  `required_effect=produce_new_search_view_and_detection`, and explicit Review fallbacks for an
  unavailable backend or exhausted budget.
- A `relocalize` edge that directly targets Human Review is blocked with
  `relocalization_branch_has_no_search`.
- A branch that cannot reach a detection-producing model after a new Image view derived directly
  from the original Image Input is blocked with `recovery_path_unreachable`.
- An ordinary `review` route remains legal and does not claim that recovery ran.

The RoboCup small-object template now expands the former single search into two bounded attempts:

```text
original image + coarse candidate
  -> 96 px search A -> localize A -> coverage A
       | refine -------------------------------------------> SAM
       | relocalize/search_tiles
       v
original image + candidate A
  -> wider search B (minimum 192 px, factor 8, max 75%)
  -> localize B -> coverage B
       | refine -------------------------------------------> SAM
       | otherwise -> Review (recovery_budget_exhausted when applicable)
```

Search B crops from the original Image Input, not from search A. Its localization node is explicitly
marked as `same_model_multi_view`; it is a changed view, not an independent model observation. Both
possible prompt/coverage pairs feed the same SAM and Mask-to-BBox tail, so only the active route is
consumed. If no prompted-segmentation model is available while Builder applies this template, both
refinement routes explicitly fall back to Review.

The coverage runtime records `requested_route`, `recovery_attempt`,
`maximum_recovery_attempts`, and `recovery_exhausted`. At the configured second-attempt limit,
another search request is not executed; the selected route becomes `review` with
`recovery_budget_exhausted`.

### Behavioral regression evidence

- Core tests reject `relocalize -> HumanReview`, accept a changed original-image Crop followed by a
  detector, and accept an explicitly named direct Review route.
- Runtime coverage tests force a second-attempt `search_tiles` decision and verify it becomes Review
  with `recovery_budget_exhausted`.
- The synthetic published-DAG runtime test forces the first Gate to select `relocalize`, then proves
  that the second scripted detector actually executes and consumes `search-view-b` at 384×384
  instead of the original Artifact. This proves routing only, not real model accuracy or outbound
  pixel materialization.
- The RoboCup template regression test is no longer ignored and proves no `relocalize` edge targets
  Human Review.
- Builder's incremental recovery patch test passes with and without a prompted segmenter; the full
  `annotagent-application` suite passes (71 passed, one explicitly opt-in billable smoke ignored).

Commands run for M1:

```text
cargo test -p annotagent-core workflow::tests::
cargo test -p annotagent-runtime prompt_coverage_gate_
cargo test -p annotagent-runtime relocalize_route_executes_a_detector_with_a_changed_search_view
cargo test -p annotagent-skill-robocup --lib
cargo test -p annotagent-application
```

M1 proves bounded control-flow correctness. It does not yet prove that the HTTP provider received
resized bytes or that visual localization improved; those are M2 and M4 concerns.

## M2 — materialized model input and evidence identity

### Actual submitted pixels

The application detection runner no longer recreates every local input at the raw Crop dimensions.
It now materializes the `ImageArtifact` selected by the DAG from the decoded root image:

1. map `root_region` to one integer root-pixel rectangle;
2. crop those original pixels once;
3. resize to the exact upstream Image Artifact dimensions with Catmull-Rom interpolation;
4. encode that raster to PNG;
5. pass that exact `ModelImage` to the selected Provider/Plugin backend.

The small-object template places `core.resize` after both search crops. A 96×96 search A therefore
requests actual 384×384 bytes, and search B is independently materialized to its declared target
size. The implementation does not claim that upscaling restores detail or improves accuracy; M4
compares it as one controlled strategy.

Each local detection output now retains a typed `ModelInputTrace` in node and DetectionSet metadata:

- source image ID and source SHA-256;
- exact `[x,y,width,height]` source pixels and Crop dimensions;
- submitted dimensions, encoded PNG SHA-256, and decoded RGB pixel digest;
- interpolation, zero letterbox padding, RGB8 format;
- coordinate-frame ID and one typed transform back to the root image;
- bounded Provider image parameters;
- Provider effective dimensions as `unknown` unless the Provider itself reports them.

No full base64 payload or authorization data enters ordinary logs. The source Artifact reference,
submitted digest and deterministic transform make the bytes auditable without duplicating the image
inside SQLite.

### Coordinate contract

`CoordinateTransform` is now the shared projection primitive used by
`core.project_coordinates`. It supports ordinary/non-uniform resize and explicit letterbox content
regions, clamps a partially padded prediction to real content, rejects padding-only boxes, and keeps
floating-point geometry until a raster output boundary. Tests cover a non-square source region,
non-uniform resize, letterbox padding, edge-clamped integer Crop pixels and 4× materialization.

### Correlated evidence

Every materialized local model result also records a typed `ModelEvidenceSource`: resolved model
identity, Profile ID, request-image digest, original image, search region, transform fingerprint,
prompt-resource hash, settings hash, purpose and parent evidence.

Prompt Coverage de-duplicates an observation when resolved model identity, actual request-image
digest and transform fingerprint match. Different Profile IDs, prompts, temperatures or request IDs
do not turn that repeat into an independent source. Same-model/different-view evidence remains
available for bounded search consistency but is explicitly counted as `same_model_multi_view`, not
as statistical independence.

The new RoboCup template removes `independent_crop_verification` entirely. Search A compares its
local candidate only with the coarse multi-view observation; the model call formerly spent on the
same Crop is now reserved for real search B.

### Regression evidence

```text
cargo test -p annotagent-core
cargo test -p annotagent-image-tools
cargo test -p annotagent-provider
cargo test -p annotagent-runtime
cargo test -p annotagent-skill-robocup
cargo test -p annotagent-application
```

Results at M2: Core 112 passed; Image Tools 7 passed; Provider 45 passed; Runtime 46 passed across
unit/integration suites; RoboCup Skill 18 passed; Application 71 passed with one explicitly opt-in
billable Provider smoke ignored. The Provider boundary test decodes the actual image carried in the
`ModelRequest`, verifies 384×384 dimensions and sentinel pixels, rather than checking UI metadata.

These tests establish byte/coordinate/evidence semantics. They do not establish football accuracy.
