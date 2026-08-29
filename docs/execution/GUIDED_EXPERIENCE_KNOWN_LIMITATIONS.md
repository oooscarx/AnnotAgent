# Guided Experience Alpha Known Limitations

Updated: 2026-08-30

This file records honest product limitations, not unfinished Release Blocking work. Open Release requirements remain in `GUIDED_EXPERIENCE_ACCEPTANCE.md`.

- Live inference quality depends on the configured provider/model and is not implied by deterministic Mock acceptance.
- The Rust process orchestrates external detector/segmenter workers through the versioned HTTP Vision Protocol; it does not directly load arbitrary model weights.
- Format conversion can be lossy when the destination format cannot represent source geometry, attributes, relations, provenance, or revisions. Compatibility and export reports must disclose this.
- Canvas accessibility uses an equivalent structured annotation list; editing complex geometry remains most efficient with a pointer.
- Actual browser 200% zoom remains a manual verification when the automation environment cannot set native zoom.

