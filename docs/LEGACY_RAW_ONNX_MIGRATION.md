# Legacy Raw ONNX Migration

Previously provisioned Plugin weight files remain visible as `LegacyUnbundledModel`. Their hashes
and historical references are preserved, but they are not treated as trusted installed Bundles or
selectable Model Instances because source, license, Contract, and reproducible smoke identity were
not recorded together.

Under **Settings → Expert Model Plugins → Legacy manual provisioning**, **Create local model
bundle** asks for Bundle identity, upstream source/revision, exporter/opset, exact license text and
acceptance, plus a versioned tensor Contract covering every legacy role. Rust then packs, verifies,
installs, binds, and smoke-tests the new data-only Bundle.

Migration never deletes or rewrites the original files, historical Runs, or immutable Workflow
Versions. Malformed/incompatible ONNX stops at Contract mismatch and produces no selectable
profile. To use the new instance, clone a historical Workflow to a Draft, bind the Ready instance,
Dry Run selected images, and publish a new Version.
