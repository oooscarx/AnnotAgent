# Detection Backends Known Limitations

Updated: 2026-08-30

These are honest boundaries after M3. Open Release work remains in
`DETECTION_BACKENDS_ACCEPTANCE.md`.

- The current Model Settings UI manages one workspace VLM Provider, not a collection of HTTP
  detection workers.
- DetectionSet and CandidateClusterSet now preserve optional, semantic, per-model evidence, but
  Candidate Match and Evidence Gate do not execute that contract until M6.
- Results/Review can read the new API shape but do not yet explain score semantics, agreement, or
  selectable source boxes; those product surfaces remain M9 work.
- Existing static fallback support is not an evidence-aware Recovery Agent.
- Detection Worker v1 and its hardened HTTP adapter are implemented, but Settings registration and
  concrete LocateAnything/RF-DETR Workers remain M4/M5 work. An offline client can be constructed
  without blocking AnnotAgent startup; model-specific availability proof remains open.
- The older generic Vision/Pipeline contracts remain for compatibility. New detection Workers use
  the canonical versioned contract; their transports now share the same trust policy and bounds.
- The tracked reference worker can run an explicitly configured Ultralytics checkpoint and the SAM
  worker can run configured SAM2 weights; neither yet implements LocateAnything or RF-DETR.
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
