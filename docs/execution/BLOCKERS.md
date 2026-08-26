# Workflow Alpha Blockers

## Active external blockers

None.

## Live-conditional checks

These do not block offline implementation or the offline Release Gate, but they cannot be marked passing without external configuration:

- Real Qwen smoke requires a valid API key supplied through the supported secret mechanism. No key from conversation history will be read, restored, or used.
- Real detector/segmenter inference requires a configured local model path or external worker endpoint and model files. Fixture protocol tests will not be described as real inference.

Ordinary missing features and failing tests belong in `STATUS.md`, not here.
