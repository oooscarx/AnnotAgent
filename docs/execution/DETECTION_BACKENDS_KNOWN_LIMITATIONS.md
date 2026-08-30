# Detection Backends Known Limitations

Updated: 2026-08-30

These are honest boundaries after M8. Open Release work remains in
`DETECTION_BACKENDS_ACCEPTANCE.md`.

- Detection Worker profiles are persisted in local Settings and can be enabled, pointed at an
  endpoint, remote-opted-in, and tested. Full arbitrary add/remove and cost editing remains M9;
  the product currently supplies curated disabled-by-default LocateAnything and RF-DETR profiles.
- Match Detection Sets currently accepts exactly two DetectionSets, as required by the Alpha
  contract. It does not perform a global assignment across three or more detector outputs.
- Evidence Gate and Recovery select one route for the image-level Candidate Cluster set.
  Per-candidate mixed routing is not implemented.
- Run Debug explains the persisted decision and previews representative boxes, but Results/Review
  do not yet show the full evidence comparison or let a reviewer choose either source box; those
  Guided product surfaces remain M9 work.
- The Alpha Recovery policy permits at most one open-vocabulary fallback call and never recursively
  calls more detectors. The RoboCup hybrid template routes a successful but unresolved Recovery
  result through a statically published Crop Classification branch; budget/Worker failures go
  directly to Human Review.
- The deterministic generic Advisor applies its detection strategy only to detection-only Projects
  without Domain Validators/Refiners. The RoboCup Ball Domain Project has a dedicated
  capability-bound template; arbitrary mixed-task Domain planning is not claimed.
- Model availability and descriptor label space are considered. Historical Dry Run metrics are
  available to the richer Advisor input contract but are not yet used by this deterministic
  baseline; M9 will present measured recommendation evidence in Guided UX.
- Evidence rule lists are editable as structured node JSON. Purpose-built Guided controls and
  rule summaries remain M9 work.
- Detection Worker v1, its hardened HTTP adapter, Settings registration, Open Vocabulary and
  Object Detection Skills, and both concrete adapters are implemented. Neither real backend was
  executed with model weights on this host.
- The older generic Vision/Pipeline contracts remain for compatibility. New detection Workers use
  the canonical versioned contract; their transports now share the same trust policy and bounds.
- The LocateAnything adapter depends on an explicitly installed official NVIDIA worker source,
  model directory, CUDA runtime, and compatible Python environment. The repository neither vendors
  nor downloads those dependencies. It currently supports text queries only, reports visual prompt
  false, and processes one image per inference request.
- No real LocateAnything inference was executed on this Darwin arm64 host. Offline Mock/contract
  results prove integration semantics, not model quality or throughput.
- The RF-DETR adapter requires an explicitly installed official package, a CUDA runtime and a
  verified local checkpoint plus immutable architecture/version/dataset/label/license metadata.
  It currently implements Object Detection for one image per protocol request. It does not claim
  segmentation, keypoints, training, or protocol-level batch inference.
- No real RF-DETR inference was executed on this Darwin arm64 host. Offline Mock/contract results
  prove integration semantics and score preservation, not model quality, calibration or throughput.
- The generic Object Detection Skill coexists with the legacy YOLO compatibility crate/operation.
  New product registration and the RoboCup Ball hybrid template are capability-based; compatibility
  paths remain for older published Workflows.
- Candidate Cluster Annotation projection selects one deterministic representative rectangle.
  Multi-source confidence remains absent and every original box/score remains in the persisted
  cluster; no box-selection UI is claimed yet.
- No model weight is bundled, downloaded, or inferred from a filename. Real model behavior and
  quality are not implied by Mock fixtures.
- LocateAnything-3B's official released model terms restrict it to non-commercial
  research/evaluation; UI metadata is informational and not legal advice.
- RF-DETR licenses vary by component/model size. A concrete descriptor must identify the exact
  code and weight terms rather than inherit a product-wide string.
- Training remains an external Job. AnnotAgent will prepare versioned export/registration data but
  will not start training from a per-image annotation Workflow.
- Native 200% browser zoom remains a manual acceptance check; automated compact viewport reflow is
  separate.
