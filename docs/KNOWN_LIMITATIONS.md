# Known Limitations

These are current implementation boundaries, not hidden release claims:

- The local server is a loopback, single-user application with strict Host/Origin, local-session,
  CSRF, privileged-confirmation and resource-limit controls. It is not a multi-user authorization
  system and provides no cloud object storage or distributed scheduler. Remote serving requires a
  separate authentication boundary and is not enabled by the local security model.
- The product Runtime requires an exact selected Published Workflow for formal image and Dataset
  starts. Historical unversioned single-Skill AgentRuntime Runs remain readable for migration, but
  the Web API does not create new legacy fallback Runs.
- Executor-level HumanReview checkpoints can be approved and resumed without rerunning completed nodes. In the product image path, review candidates are currently persisted as `NeedsReview` and the image Run becomes terminal `CompletedWithReview`; a later Review decision does not resume that same Run to its Commit node.
- The Model Registry, capability checks, OpenAI-compatible/HTTP/deterministic adapters and native
  Rust plugin lifecycle are implemented. The compatibility Settings field is still serialized as
  `detection_workers` so historical HTTP Vision v1 bindings load unchanged, but a new workspace has
  none. Native ONNX inference is bundled; model training is not.
- Published product DAG nodes can bind separate registered specialist, open-vocabulary and
  classifier Backends. The reference LocateAnything/RF-DETR adapters require external legal weights
  and supported environments; their disabled/unavailable states remain visible without blocking
  startup.
- A real Qwen-compatible smoke is live-conditional. No credential from conversation history is read
  or reused. Expert detector/segmenter inference is conditional on exact installed checkpoint and
  smoke evidence; missing evidence remains NeedsWeights or unavailable rather than fabricating
  success.
- Official Rust SAM and PIDNet process implementations now exist with strict ONNX contracts,
  component/checkpoint identities, typed Artifacts and opt-in real-process tests. Their real-weight
  smoke and accuracy remain live-conditional because the repository contains no accepted
  checkpoints. Offline tensor/contract tests prove implementation behavior, not model accuracy.
- The built-in Prompted Segmentation Model Bundle is an AnnotAgent-generated, non-publishable
  protocol Fixture. It executes through the real Rust SAM Plugin and ONNX Runtime but is not SAM,
  is not selectable by Published Workflows, and provides no quality claim. EfficientSAM-Ti now has
  a legally audited, contract-compatible, reproducible real Bundle and passed macOS ARM64 CPU smoke;
  one-click GUI, remote publication and additional-platform evidence are still pending. SAM 2
  remains Labs. See [SAM Model Provisioning](SAM_MODEL_PROVISIONING.md).
- RF-DETR has a Rust implementation for the official detection ONNX contract but no accepted export
  was real-smoke tested in this repository, so its package is live-conditional. LocateAnything's
  official release has no audited complete Rust-callable runtime; its contract package is explicitly
  UnsupportedPlatform and cannot fall back to the historical Python Worker.
- ZIP image import is deliberately rejected before extraction in Workflow Alpha. This provides a smaller safe boundary than accepting archives; users extract an archive themselves and import a workspace-local folder. There is no archive unpacker whose traversal behavior is implied to be safe.
- COCO string RLE can be represented and exported but is not generally drawable or editable in the Web canvas. COCO, LabelMe, and YOLO cannot express every Native provenance, relation, attribute, mask, or revision field, so their compatibility reports list losses.
- TUI can start/control legacy Project Runs and inspect Project, Workflow, node, Artifact, validation, recovery, model, usage, timeout, checkpoint, and review state from history. Workflow authoring, exact Published Version selection, Batch creation, and annotation geometry editing remain GUI/HTTP/CLI operations.
- TUI Model Bundle support is intentionally observational (`catalogs`, `bundles`, `instances`,
  `references`, `doctor`). License acceptance, download/import, enable/disable, removal and GC use
  the GUI or CLI so the human action and exact target remain explicit.
- Detection and deterministic node Cache Keys are content/config/model aware within an executor,
  and persisted checkpoints support cross-process Replay without rerunning ancestors. Arbitrary
  cross-process reuse of cached model outputs is not implemented. Dynamic plugins, training loops,
  video, mobile, Tauri packaging, and automatic execution of generated code are outside this Alpha.
- The release suite exercises the core Project workspace at the 640 CSS-pixel reflow boundary
  produced by 200% browser zoom on a 1280 px window, as well as 1024 px, 720×450, and 390×844.
  Browser-chrome-specific zoom controls still vary by host and remain appropriate for a final
  platform smoke; no current evidence claims every OS/browser combination.
- Summary APIs are bounded to at most 100 rows per request. Run and Review indexes expose
  incremental loading; the global Project dashboard currently renders its first bounded page
  rather than virtualizing arbitrarily large Project inventories.
- The production Web bundle remains above the 500 kB warning threshold. M9 extracted ownership
  route/service seams without a disruptive rewrite; further route-level code splitting is future
  performance work.
- Complex polygon, mask, and keypoint editing is keyboard-readable and selectable through the structured annotation list, but precise geometry manipulation remains more efficient with a pointer. COCO string RLE remains non-editable as described above.
- The default improvement recommendation floor is ten independent holdout images and the default
  calibration threshold is thirty reviewed references. These are conservative Alpha guards, not a
  universal statistical guarantee; production Projects should set thresholds from their own
  distribution and risk tolerance.
- Improve Automation compares only Runs whose Project image identities can be proven disjoint from
  diagnosis evidence. Sparse demonstrations may create a Patch Draft but remain provisional or
  insufficient and are not recommended.
## Rust plugin Alpha

Native plugins provide process isolation, not a universal per-platform OS sandbox. Production
checkpoints are not bundled. SAM, PIDNet and RF-DETR real-weight smoke tests require user-supplied
compatible files and hardware; LocateAnything is explicitly unsupported until a complete audited
Rust-callable runtime exists. See
[Rust Plugin Known Limitations](execution/RUST_PLUGIN_KNOWN_LIMITATIONS.md).
