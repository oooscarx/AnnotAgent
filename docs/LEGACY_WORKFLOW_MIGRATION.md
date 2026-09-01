# Legacy Workflow Geometry Migration

Historical Published Workflow Versions and Runs remain immutable and viewable. AnnotAgent
conservatively interprets legacy VLM scores as semantic or relative confidence, VLM boxes as coarse
hypotheses, and specialist boxes as uncalibrated predicted geometry.

If a historical Version used model confidence as its only bbox acceptance signal:

- historical Runs, Artifacts and Sandbox Replay remain available;
- the original serialized Version and hash are not changed;
- a new formal Run is blocked as `unsafe_legacy_workflow`;
- **Create geometry-safe Draft** clones the Version and adds a mandatory Review boundary;
- any risk acceptance is explicit and auditable, never inferred during migration.

Model/Profile and workflow migrations are transactional and idempotent. Secrets and API keys are not
part of geometry metadata or migration records.
