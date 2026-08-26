# Extending Skills

1. Add `skills/<id>/manifest.yaml`, a concise `SKILL.md`, and task-specific resources.
2. Define a project schema using existing task kinds, labels, attributes, dependencies, generic validators, and export preferences.
3. For algorithms, create a crate implementing `DomainSkill` and any required `AgentTool`, `AnnotationValidator`, `AnnotationRefiner`, and `ReviewPolicy` objects.
4. Register the Skill at the binary composition root with `SkillRegistry::register`.
5. Add a test modeled after `crates/annotagent-runtime/tests/skill_extension.rs`.

Tools should provide strict JSON Schema and an `applicable_tasks` list. Validators return structured evidence and suggested action; they must not mutate candidates. Refiners return a new annotation, confidence, issues, and an auditable summary. Prompt resources should be short, task-scoped, and must not contain secrets.

The optional `project_template` method lets a Skill populate the generic GUI without putting domain labels in the frontend or Server. The GUI reads task/resource/taxonomy data from `/api/skills`.

The current registry is compile-time. Do not add a dynamic loader unless deployment, signing, version compatibility, and isolation requirements are first defined.
