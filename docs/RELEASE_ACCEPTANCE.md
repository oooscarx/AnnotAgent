# Workflow Alpha Release Acceptance

AnnotAgent Workflow Alpha is accepted by `./scripts/acceptance.sh`. The script is fail-fast and runs Rust formatting, strict all-target/all-feature Clippy, the complete Rust suite and build, Web typecheck/tests/build, doctor, and both offline demos.

The blocking evidence is maintained in [execution/ACCEPTANCE_EVIDENCE.md](execution/ACCEPTANCE_EVIDENCE.md). It covers:

- Core/Skill boundary and zero/multi-Skill Projects;
- tool-call replay, typed Artifacts, immutable Workflow versions, and product DAG execution;
- retry, fallback, timeout, cancellation, review, Commit, checkpoint, and exact budgets;
- OpenAI-compatible, JSON-only, HTTP worker, mock, and deterministic CV backend contracts;
- Workflow Advisor/editor/Dry Run/publish/version selection;
- 100-image pause/restart/resume without duplicate work;
- RoboCup hybrid hard-negative review and ground-truth evaluation;
- Review editing and Native/COCO/LabelMe/YOLO import/export behavior;
- path, symlink, archive, pixel, endpoint, untrusted output, and secret boundaries;
- GUI browser journeys and TUI audit visibility.

A real Qwen-compatible call remains live-conditional and requires a current credential supplied
through the supported secret mechanism. Prompted segmentation is no longer only conditional:
EfficientSAM-Ti has a verified non-Fixture Model Bundle and a Rust ONNX CPU Plugin, with real
macOS ARM64 install, smoke, Workflow, Review and Replay evidence. Other detector/segmenter models
still require their own verified Bundle or endpoint. Fixture behavior remains visibly marked and
non-publishable.

Release scope remains a trusted, loopback, single-user local product. Authentication, cloud/distributed execution, training, general ONNX execution, video, and dynamic plugin installation are outside Workflow Alpha.
