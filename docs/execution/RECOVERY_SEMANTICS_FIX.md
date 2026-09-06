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
  candidate through SAM/geometry/review safely — completed below.
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

## M3 — refinement eligibility is not automatic acceptance

Prompt coverage now records three separate facts instead of overloading a single `Covered` value:

- observed coverage state: `covered`, `partially_covered`, `outside_prompt`, or `unknown`;
- prompt-refinement eligibility: `plausible_for_refinement`, `needs_relocalization`, `unknown`, or
  `invalid_prompt`;
- automatic-acceptance eligibility: `not_evaluated` or `human_review_required`.

On the configured final recovery attempt, an explicit policy may allow a valid, non-empty candidate
with `unknown` or partial coverage to enter prompted segmentation as exploratory refinement. This
does not turn uncertainty into coverage evidence: `automatic_acceptance` is set to
`human_review_required`, `localization_uncertain` is retained, and the Mask-to-BBox and geometry
tail preserve that requirement until the terminal Review route. Without the explicit policy, the
same input cannot enter the segmenter.

The RoboCup template passes the active A or B `PromptCoverage` Artifact into Mask-to-BBox. Stable
refined geometry can be accepted by the geometry calculation only when upstream policy permits it;
uncertain exploratory refinement still routes to Human Review. Refiner drift restores the coarse
box, records `refiner_drift`, and also routes to Review. No SAM score is interpreted as semantic
proof that the mask is a football.

Coverage decisions are retained per candidate ID in `candidate_route_decisions`. Because the
current DAG executor routes Artifact sets rather than individual items, a set containing different
candidate routes fails closed to Review with `candidate_route_split_requires_review`. It is never
sent wholesale into the strictest candidate's relocalize/search/refine branch. The football alpha
expects one ball candidate; this conservative fallback prevents cross-candidate corruption until a
typed split/fan-in executor is implemented.

The synthetic published-DAG recovery test now forces A to relocalize, executes B from the changed
search view, then forces B to refine and verifies that the refiner consumes B rather than stale A.
This is execution/lineage evidence only, not an accuracy result.

Regression commands for M3:

```text
cargo fmt --all -- --check
cargo test -p annotagent-core
cargo test -p annotagent-skill-segmentation
cargo test -p annotagent-runtime
cargo test -p annotagent-skill-robocup
cargo test -p annotagent-application
```

Results: Core 112 passed; Segmentation Skill 3 passed; Runtime 36 passed across unit/integration
suites; RoboCup Skill 18 passed; Application 71 passed with one explicitly opt-in billable Provider
smoke ignored. The Application regression also caught and repaired a Registry inconsistency where
the public node catalog accepted `PromptCoverage` at Mask-to-BBox but the static operation catalog
did not.

## M4 — Builder evidence, real Draft and product regression

### Builder-controlled recovery inspection

Pipeline Builder now has three bounded, read-only tools instead of having to infer execution from
node names:

- `validate_recovery_paths` runs the same static recovery semantics used for publication and
  reports the actual execution order and recovery-policy issues;
- `inspect_recovery_execution_summary` reports selected routes, attempt counters, exhaustion,
  candidate route decisions and whether the recovery/refinement nodes actually ran;
- `inspect_model_input_summary` reports persisted model-input dimensions, regions, digests and
  Provider-effective-dimension status without exposing image base64 or credentials.

The Builder system rules require these observations after a Sample Test and explicitly distinguish
a changed original-image view from a same-input repeat. The scripted Builder regression completes
the full tool loop and proves all three tools were called successfully before the Draft was
submitted for human approval. The branch-injection behavior itself remains covered by the M1–M3
runtime tests: first-gate recovery, changed search B input, budget exhaustion, correlated-repeat
handling and consumption of B rather than stale A.

Direct template creation now goes through the same Registry binding helper as LLM and salvage
paths. A newly created recovery Draft therefore selects ready, compatible non-fixture Model
Profiles and Model Instances rather than leaving its Qwen and segmenter nodes unbound or silently
using mock. The regression `direct_recovery_template_creation_binds_ready_registry_models` verifies
all three VLM nodes and the prompted-segmentation node.

### Persisted execution evidence in the product

Every Sample Test node result now retains its runtime metadata. The Test page adds a prominent
**What actually ran** section showing, per image:

- the exact submitted model image, source pixel ROI, Crop and submitted dimensions;
- preprocessing, submitted SHA-256 and Provider-effective dimensions;
- model/backend, node latency, known cost or `Unknown`;
- selected recovery routes, attempt counts, failure reason and whether a Mask was produced.

The image is reconstructed from the immutable source image plus `ModelInputTrace` and is returned by
`/api/workflow-sample-tests/{test}/samples/{index}/nodes/{node}/model-input`. The endpoint verifies
the current source SHA, submitted PNG SHA and normalized RGB digest before returning bytes. It does
not duplicate base64 payloads in SQLite. Sample Test sandbox executions now use the Project's
persisted image identity rather than a transient UUID, so the trace remains resolvable after a
refresh or process restart. VLM Provider usage metadata is also projected into runtime token usage;
an absent price remains unknown rather than being presented in the UI as a real zero-dollar quote.

The first real SAM attempt exposed a compatibility bug that synthetic tests had not found. The
installed EfficientSAM binary correctly implements the declared Image + BoxPromptSet contract but
predates the Core-only `prompt_coverage` Artifact variant. Passing every orchestration Artifact to
the plugin caused HTTP 422 before inference. Prompt Coverage is now consumed and validated inside
Core, then omitted from the model request; the plugin receives only Image and one prompt set. A
contract-strict regression proves that Core control Artifacts cannot cross this boundary. Plugin
HTTP errors now include a bounded local response body, which made the contract rejection
diagnosable without logging secrets.

### Real RoboCup Ball evaluation

No saved history was changed or deleted. The repaired working Draft is
`f1f0fae0-6095-41eb-af6d-b7f4019bdd9a`, revision 2. It binds all Qwen nodes to Model Profile
`b9c5bbe8-e21a-5784-9c52-cade259b434f@2` (`qwen3.7-flash-2026-07-15`) and binds
`refine_validated_prompt` to ready EfficientSAM Model Instance
`ae3efb4b-ef31-59e0-ad8d-e5bc30a6da72`.

The controlled B Draft `260f8762-cb41-4c39-83e1-1b4478d3d274`, revision 2 uses the repaired 96 px
Crop → actual 384 px submission and no same-input verifier, but begins with its recovery budget
exhausted. This makes the no-search fallback explicit (`recovery_budget_exhausted`) rather than
pretending Review is a relocalization. C uses the normal two-attempt recovery policy.

| Case | Saved Sample Test | Images | Actual calls | Result |
| --- | --- | ---: | --- | --- |
| A — preserved historical baseline | `53c4c39d-cb9a-4e3d-9bea-79852a5d130a` | 1 | 3 correlated Qwen observations, 0 SAM | fake relocalize-to-Review; one coarse review candidate |
| B — real 4× local input, no remaining recovery | `77c6aba3-2125-47e9-8993-7ea5938ab3e0` | 4 | 8 Qwen, 0 SAM; 6,260 input / 600 output tokens | 4 candidates, 4 Review, 0 failures; every recovery request ended with explicit budget exhaustion |
| C — B plus real wider search B and SAM | `c21ee7c8-9fdf-4f5b-a411-22df302a3e48` | 4 | 11 Qwen, 4 EfficientSAM; 8,540 input / 824 output tokens | 4 refined candidates, 4 Review, 0 failures; 3 images used search B, all 4 reached SAM |

C took 29.963 seconds versus 13.570 seconds for B. Search A inputs were derived from the root image
and submitted at up to 384 px; search B used a different, wider root ROI and its own submitted PNG.
For `color_1001525.png`, the final audit run
`78cb04a1-fffe-4c83-b8ff-a3a68fddc1af` records:

```text
whole image:   source [0,0,544,448]   -> submitted 544×448
search A:      source [222,186,96,96] -> submitted 384×384
coverage A:    outside_prompt         -> search_tiles
search B:      source [174,109,192,192] -> submitted 384×384
coverage B:    partially_covered      -> refine (human review required)
EfficientSAM:  source [0,0,544,448]   -> submitted 544×448
final bbox:    [0.4963235, 0.4397321, 0.0220588, 0.0267857], needs_review
```

All four model-input preview requests for that test returned verified PNGs. The whole-image Qwen and
SAM input SHA is `d304836b…`; search A and B have distinct SHA values, proving that request IDs alone
did not create the recovery evidence.

### Accuracy status

Engineering correctness **passes** for the repaired path: search B really executes, inputs differ,
SAM really runs, the final projection contains one candidate per lineage, and uncertainty is not
auto-committed.

Visual quality is **still provisional and not demonstrated as a general improvement**. There is no
human-confirmed ground truth in the active database. Against the explicitly unverified historical
YOLO export only, the four-image mean overlap was 0.5694 for B and 0.4082 for C. This proxy therefore
does not support a claim that adding recovery + SAM improved the fixed set. Per-image C overlap was
0.0940, 0.5195, 0.8910 and 0.1285, showing that SAM can tighten a good prompt but cannot repair an
unstable or semantically wrong Qwen localization. A later stochastic audit of the first image
produced overlap 0.6336 with that same unverified export and a visually plausible 12×12 pixel box;
the variation itself is evidence that one successful call is insufficient.

Product inspectability **passes at the API/build level**: results retain a single terminal
candidate, Debug retains all stages, exact model inputs are refresh-safe, and reasons distinguish
search exhaustion, uncertain refinement and actual model failure. Browser automation could not be
visually re-captured during M4 because the host Mac was locked; the checked TypeScript, unit and
production-build results are the UI evidence for this milestone rather than a fabricated
screenshot claim.

### Remaining work outside this repair

- Establish a human-reviewed evaluation set with immutable annotation revisions before reporting
  accuracy, recall or a production auto-accept threshold.
- Calibrate Qwen localization and geometry decisions on that set. The current four-image result
  supports mandatory Review, not automatic acceptance.
- Provider-internal image dimensions remain `unknown` because this Provider does not report them.
- Model Profile pricing is not configured, so remote cost is shown as `Unknown`; token counts and
  node latencies are still recorded.
- Candidate-level route decisions are persisted, but mixed routes in one set intentionally fail
  closed until the executor has a typed item fan-out/fan-in implementation.

### M4 verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
npm --prefix web run typecheck
npm --prefix web run test -- --run
npm --prefix web run build
npm --prefix web run test:e2e
```

Results: formatting and Clippy passed without warnings; the Rust workspace completed 477 tests with
477 passed and five explicitly opt-in real-weight or billable-Provider tests ignored; Web completed
13 test files / 63 tests and 44 Playwright journeys with no failures. TypeScript checking and the
production build passed. Vite reports only the existing advisory that the single application chunk
is larger than 500 kB; it is not a correctness or release failure.

The E2E suite additionally caught two product-level regressions during final verification. A
persisted setup-required Builder result had its recovery actions inside a closed diagnostics panel;
that panel now opens when Provider or Model setup is required. The Sample Test preview regression
now derives the dialog name from the real Project image identity instead of incorrectly treating a
classification label as a filename.
