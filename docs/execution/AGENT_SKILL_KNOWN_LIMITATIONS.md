# AnnotAgent Agent + Skill Known Limitations

## M0 baseline

- Layered Skill kinds and resolution are available; bundled legacy Skills still use the
  `DomainSkill` compatibility path until their milestone migrations.
- Strong envelopes are materialized in Published DAG traces; legacy per-task storage continues to
  persist `VisionArtifact` payload rows for backward-compatible history import/export.
- OpenAI-compatible and HTTP Classification are implemented and tested with local fixtures; an
  external live request remains conditional on operator-owned configuration.
- Real YOLO weights are intentionally out of process behind Pipeline Vision Protocol v1; the Mock
  and generic HTTP JSON paths are release-blocking, while a live worker is conditional.
- The offline deterministic Advisor is the release-blocking iterative policy. The external
  OpenAI-compatible advisor adjustment remains live-conditional and still passes through the same
  registry validation and human publication boundary.
- Robot and Field are compatibility/Roadmap areas, not independent layered Domain Skills in this
  Alpha. They continue to supply optional evidence to Ball validation.
- Agent Session trace is persisted and visible in Web/TUI; correlating every deterministic Run
  node and a conditional Recovery Session in one unified chronological canvas remains future UX.
- Legacy broad `robocup` Projects remain on the compatibility adapter until an operator explicitly
  enables the layered Pack/Domain dependencies; the product does not silently rewrite Project
  semantics.
- Real Qwen and real YOLO runs require operator-owned configuration and are live-conditional.
