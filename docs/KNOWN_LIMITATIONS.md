# Known Limitations

These are current implementation boundaries, not hidden release claims:

- The local server is a trusted loopback, single-user application. It has permissive development CORS and no login, authorization, multi-user isolation, cloud object storage, or distributed scheduler.
- The product Runtime executes an exact selected Published Workflow for image and Dataset Batch starts. Starting without a selected version retains the legacy single-Skill AgentRuntime for compatibility; zero- or multi-Skill Projects must select a Published Workflow.
- Executor-level HumanReview checkpoints can be approved and resumed without rerunning completed nodes. In the product image path, review candidates are currently persisted as `NeedsReview` and the image Run becomes terminal `CompletedWithReview`; a later Review decision does not resume that same Run to its Commit node.
- The Model Registry, capability checks, OpenAI-compatible/HTTP/mock/deterministic adapters, and
  external Worker contract are implemented. Settings can discover, sample and register an
  arbitrary collection of versioned Expert Vision Workers; the compatibility Settings field is
  still named `detection_workers`. General ONNX inference and model training are not bundled.
- Published product DAG nodes can bind separate registered specialist, open-vocabulary and
  classifier Backends. The reference LocateAnything/RF-DETR adapters require external legal weights
  and supported environments; their disabled/unavailable states remain visible without blocking
  startup.
- A real Qwen-compatible smoke is live-conditional. No credential from conversation history is read or reused. External detector/segmenter inference is also conditional on configured endpoints or model weights; the reference worker reports `weights_unavailable` rather than fabricating success.
- Real SAM, PIDNet and Grounding DINO inference remains live-conditional because the repository
  contains no downloaded checkpoint. Presets and deterministic contract fixtures prove integration,
  not model accuracy.
- ZIP image import is deliberately rejected before extraction in Workflow Alpha. This provides a smaller safe boundary than accepting archives; users extract an archive themselves and import a workspace-local folder. There is no archive unpacker whose traversal behavior is implied to be safe.
- COCO string RLE can be represented and exported but is not generally drawable or editable in the Web canvas. COCO, LabelMe, and YOLO cannot express every Native provenance, relation, attribute, mask, or revision field, so their compatibility reports list losses.
- TUI can start/control legacy Project Runs and inspect Project, Workflow, node, Artifact, validation, recovery, model, usage, timeout, checkpoint, and review state from history. Workflow authoring, exact Published Version selection, Batch creation, and annotation geometry editing remain GUI/HTTP/CLI operations.
- Detection and deterministic node Cache Keys are content/config/model aware within an executor,
  and persisted checkpoints support cross-process Replay without rerunning ancestors. Arbitrary
  cross-process reuse of cached model outputs is not implemented. Dynamic plugins, training loops,
  video, mobile, Tauri packaging, and automatic execution of generated code are outside this Alpha.
- Native browser 200% zoom remains a manual Release check because the automated Chromium environment can set viewport and motion preferences but cannot prove the browser chrome's native zoom level. The full journey is automated at 1024 px and 720×450, and responsive CSS remains usable under reflow.
- Complex polygon, mask, and keypoint editing is keyboard-readable and selectable through the structured annotation list, but precise geometry manipulation remains more efficient with a pointer. COCO string RLE remains non-editable as described above.
