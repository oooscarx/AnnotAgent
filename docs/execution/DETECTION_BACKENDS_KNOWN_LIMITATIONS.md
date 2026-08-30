# Detection Backends Known Limitations

Updated: 2026-08-30

These are honest boundaries after M4. Open Release work remains in
`DETECTION_BACKENDS_ACCEPTANCE.md`.

- Detection Worker profiles are persisted in local Settings and can be enabled, pointed at an
  endpoint, remote-opted-in, and tested. Full arbitrary add/remove/metadata editing remains M9;
  the M4 product profile is a disabled-by-default LocateAnything Worker.
- DetectionSet and CandidateClusterSet now preserve optional, semantic, per-model evidence, but
  Candidate Match and Evidence Gate do not execute that contract until M6.
- Results/Review can read the new API shape but do not yet explain score semantics, agreement, or
  selectable source boxes; those product surfaces remain M9 work.
- Existing static fallback support is not an evidence-aware Recovery Agent.
- Detection Worker v1, its hardened HTTP adapter, Settings registration, Open Vocabulary Skill,
  and the concrete LocateAnything adapter are implemented. RF-DETR remains M5 work.
- The older generic Vision/Pipeline contracts remain for compatibility. New detection Workers use
  the canonical versioned contract; their transports now share the same trust policy and bounds.
- The LocateAnything adapter depends on an explicitly installed official NVIDIA worker source,
  model directory, CUDA runtime, and compatible Python environment. The repository neither vendors
  nor downloads those dependencies. It currently supports text queries only, reports visual prompt
  false, and processes one image per inference request.
- No real LocateAnything inference was executed on this Darwin arm64 host. Offline Mock/contract
  results prove integration semantics, not model quality or throughput.
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
