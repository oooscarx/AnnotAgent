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

Two checks are live-conditional rather than offline blockers: a real Qwen-compatible call requires a current credential supplied through the supported secret mechanism, and real external detector/segmenter inference requires configured weights or an endpoint. Neither is represented as passing when absent; fixture behavior remains visibly marked mock/degraded.

Release scope remains a trusted, loopback, single-user local product. Authentication, cloud/distributed execution, training, general ONNX execution, video, and dynamic plugin installation are outside Workflow Alpha.

