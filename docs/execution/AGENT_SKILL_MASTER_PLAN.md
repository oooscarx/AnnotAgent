# AnnotAgent Agent + Skill Master Plan

This plan turns the existing Label Pipeline foundation into two real, bounded Agent loops and a
layered Skill system. AnnotAgent remains the product; RoboCup Ball is one optional domain Skill.

## Non-negotiable boundaries

- Tool is one smallest callable action.
- Core Node is domain-neutral deterministic workflow logic.
- Model is one concrete callable provider/model binding.
- Capability Skill contributes reusable abilities such as classification or detection.
- Domain Skill contributes taxonomy, policy, validators, resources, and templates for one domain.
- Skill Pack groups related Skills without becoming a second product shell.
- Advisor output is always an editable Draft and never publishes itself.
- Recovery runs only for risky candidates; normal candidates retain the fast path.
- Geometry is passed through typed Artifact references, never copied through model prose.
- No production code may contain a key copied from an issue, prompt, or conversation.

## Milestones

| Milestone | Deliverable | Release evidence |
|---|---|---|
| M0 | audited baseline, course mapping, boundary and protocol baseline | boundary script and focused tests |
| M1 | layered Skill manifests, unified Skill trait, multi-Skill validation and safe resources | dummy capability/domain/pack tests |
| M2 | strong Artifact envelope and strict tool-call protocol | lineage, cache, multi/missing/duplicate/wrong-id/cancel tests |
| M3 | Classification Capability Skill | whole-image, crop, multi-label, verifier and backend tests |
| M4 | VLM/YOLO detection plus domain-neutral Core crop/gates | DetectionSet → CropSet lineage tests |
| M5 | iterative Workflow Advisor Agent | invalid-draft revision, dry-run revision, approval and budget tests |
| M6 | RoboCup pack and `robocup.ball` Domain Skill | hard-negative, field relation, resources and template tests |
| M7 | correction memory and Annotation Recovery Agent | fast path, recovery, memory isolation and budget tests |
| M8 | Web/TUI surfaces for Skills, Advisor and trace | unit, HTTP and browser acceptance |
| M9 | offline demos, 100-image batch, reliability and release matrix | complete release script and demo guide |

Each milestone updates status, decisions, evidence, blockers and known limitations, runs its scoped
checks, and receives one independent local commit. The remote is never changed or pushed.
