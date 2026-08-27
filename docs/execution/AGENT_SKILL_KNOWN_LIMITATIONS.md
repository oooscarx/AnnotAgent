# AnnotAgent Agent + Skill Known Limitations

## M0 baseline

- Layered Skill kinds and resolution are available; bundled legacy Skills still use the
  `DomainSkill` compatibility path until their milestone migrations.
- Workflow Advisor and Annotation Recovery are not yet the required iterative Agent loops.
- RoboCup Ball is not yet isolated as its own Domain Skill inside a Pack.
- Web and TUI do not yet expose the complete Agent sessions, tools and memory views.
- Real Qwen and real YOLO runs require operator-owned configuration and are live-conditional.
