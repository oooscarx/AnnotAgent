# Detection Backends Known Limitations

Updated: 2026-08-30

These are honest boundaries at the M0 baseline. Open Release work remains in
`DETECTION_BACKENDS_ACCEPTANCE.md`.

- The current Model Settings UI manages one workspace VLM Provider, not a collection of HTTP
  detection workers.
- Current DetectionSet items require a numeric confidence and cannot represent score semantics or
  independent multi-model evidence.
- Existing static fallback support is not an evidence-aware Recovery Agent.
- The tracked reference worker can run an explicitly configured Ultralytics checkpoint and the SAM
  worker can run configured SAM2 weights; neither implements LocateAnything or RF-DETR.
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
