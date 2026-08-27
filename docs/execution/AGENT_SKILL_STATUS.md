# AnnotAgent Agent + Skill Status

Last updated: 2026-08-27

## Current milestone

M0 — complete. M1 layered Skill architecture is next.

## Audited baseline

- The repository is on `main`, clean at the start of this effort, and 11 commits ahead of
  `origin/main`.
- Existing strengths: checked geometry, typed vision and pipeline Artifacts, persisted model/tool
  history, bounded annotation Runtime, pause/resume/cancel, immutable workflow versions, Dry Run,
  Replay, Classification/VLM Detection/YOLO crates, HTTP vision adapters, Web workflow authoring,
  and multi-Skill project configuration.
- `ModelMessage` already preserves assistant `tool_calls` and tool-result `tool_call_id`; Runtime
  validates one ordered result for every call.
- `SucceededEmpty` exists for task outcomes.

## Confirmed gaps against this master task

- `DomainSkill` is still the only production extension abstraction; manifests do not distinguish
  Capability, Domain and Pack or declare dependencies/conflicts/capabilities.
- Artifact data is split across `VisionArtifact` and pipeline envelopes rather than one strong,
  project/run/image/node-scoped envelope.
- the LLM Workflow Advisor performs one constrained submission, not an iterative inspect → validate
  → dry-run → revise → approval loop.
- correction memory exists as storage data, but no separate bounded Annotation Recovery Agent owns
  risky-candidate recovery.
- RoboCup is a broad Skill rather than a Pack containing `robocup.ball` with robot/field roadmap
  entries.
- TUI has `/skills` but not the complete Advisor and memory command set required here.

## Safety status

- No remote mutation or push is authorized.
- No conversation-provided key will be read, stored, or used.
- Real Qwen and real YOLO checks remain live-conditional; Mock and local HTTP protocol paths are
  release-blocking.

## M0 verification

- domain boundary scan: passed;
- ordered one-result-per-tool-call protocol baseline: passed;
- the complete master request is archived at `docs/execution/AGENT_SKILL_MASTER_PROMPT.md`.
