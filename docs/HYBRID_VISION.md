# Hybrid vision execution

AnnotAgent should use a vision-language model as a semantic planner and ambiguity resolver, not as the only source of pixel geometry. Auxiliary vision models produce typed evidence; Runtime remains responsible for fusion, validation, provenance, review, and commit.

## Recommended RoboCup graph

```text
Image
  ├─ PIDNet / task-specific segmenter ──> field-region probability mask
  ├─ YOLO / task-specific detector ─────> ball, robot, person boxes
  ├─ OpenCV pixel tools ────────────────> white-line and color evidence
  └─ VLM ───────────────────────────────> scene type, coarse prompts, attributes
                                           │
                      boxes / points ──────┘
                                           v
                                  SAM mask refinement
                                           │
                                           v
                         typed evidence fusion + validators
                                           │
                                  commit / review queue
```

| Component | Correct responsibility | Must not be treated as |
|---|---|---|
| VLM | Scene classification, coarse geometry, relation reasoning, robot attributes, resolving conflicting evidence | A precise segmentation or line-fitting engine |
| YOLO | High-recall object proposals with class scores and boxes | A field mask or attribute reasoner |
| PIDNet or another semantic segmenter | Dense field/background/line probabilities after domain training | A useful RoboCup model without task-specific weights |
| SAM | Boundary refinement from a box or point prompt | An object detector or semantic classifier |
| OpenCV/Rust tools | Deterministic color, line, geometry, containment, and continuity measurements | The final semantic decision maker |

## Core boundary

Auxiliary inference is exposed through the domain-neutral `VisionModelBackend`, separate from the chat-oriented `VisionModelProvider`. `ModelRegistry` binds stable model IDs to `openai_compatible`, `http_json`, or `mock` backends; `onnx` is a declared backend kind whose in-process implementation is intentionally deferred. A result is a typed `VisionArtifact` containing:

- Artifact kind (classification, bounding box, keypoints, polyline, polygon, semantic mask, or instance mask);
- normalized geometry, label, role, confidence, metadata, and validation state;
- source node plus provider/model/tool/request/digest provenance;
- parent Artifact IDs and stable references for model-facing messages;
- an explicit backend error rather than an empty successful inference.

The RoboCup Skill maps evidence labels to its Annotation Schema. Core must not contain RoboCup labels or model-specific post-processing.

## Execution and fusion rules

1. Execute registered nodes under task and Provider timeouts with cancellation and split budgets.
2. Preserve every raw proposal in audit history; never silently replace VLM geometry with an auxiliary output.
3. Fuse by task-specific deterministic policy. Examples: clip YOLO boxes to the PIDNet field mask, seed SAM with the chosen box, and snap a submitted field polyline to white-pixel support.
4. Treat disagreement as review evidence. It must lower confidence or open a human-review gate, not be hidden by averaging.
5. Allow a configured fallback. An unavailable auxiliary model may route to VLM-only review, but it must not be reported as a fully validated success.

## Implemented boundary and deployment order

The repository now includes Mock, generic HTTP JSON, and OpenAI-compatible backends, a registry-bound hybrid executor, deterministic tool caching, and the example graph at `examples/robocup/hybrid-workflow.yaml`. The HTTP JSON wire schema is the integration point for external Python workers.

1. Connect and evaluate a YOLO sidecar for `objects`, because boxes have the clearest typed contract and measurable mAP/recall.
2. A RoboCup-trained semantic segmenter for `field_region`; PIDNet is suitable only after relevant training and evaluation.
3. SAM refinement seeded by accepted boxes/points for mask tasks.
4. Evidence-fusion nodes, cache-by-image/model digest, GPU scheduling, and UI trace/preview.

The practical first deployment is a local Python inference sidecar (PyTorch/ONNX) behind a versioned loopback API, with a Rust adapter enforcing timeouts, payload bounds, hashes, and typed conversion. Moving stable models to ONNX Runtime inside the process can follow after outputs are parity-tested.

## Acceptance metrics

Compare VLM-only and hybrid runs on the same locked dataset using candidate-production rate, per-task validation-pass rate, object mAP/recall, field-mask IoU, line pixel F1, review rate, failure rate, p95 latency, and cost per accepted image. A hybrid path is enabled by default only when it improves task quality without hiding failures.
