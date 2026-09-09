# Presentation session navigation cleanup

Based on `6819b4d`, matching the provider recovery UI used on port 8788. This branch adds optional `conversation_task_display` metadata. Navigation hides archived tasks and uses an explicit display title; no original message, authorization, model-call ledger, feedback, annotation, sample or workflow is changed. Archived tasks remain addressable for history and audit references. This is navigation archival, not destructive history deletion.

The additive idempotent migration deliberately has no sequential schema_migrations entry, allowing it to be integrated alongside the independent history-scope and dataset-delivery migration branches without reusing a version number.

A guarded one-time utility `scripts/organize_presentation_sessions.py` reads an online SQLite backup and its captured task list, rejects changed task lists and active calls/samples, and writes only display metadata. It emits `restore-navigation.sql` for reversing just these overrides, without restoring the whole database or losing later work.

Applied to the real workspace on 2026-09-10 at explicit user request:
- 9 sessions archived from navigation.
- Kept task `357155d7-3073-4dea-b07b-90adfa10ed4e`, display title `B-Human 足球标注`.
- All preexisting table checksums unchanged after restart; foreign_key_check returned no violations.
- Navigation HTTP returned zero visible tasks for the robot project and exactly the kept task for RoboCup.
- Browser refresh confirmed the title, removed sidebar entries, and retained original task content, sample predictions and plan details.
- Images, projects, model providers and original history were retained.

Backup and restoration records: `/Users/oscar/Documents/my_workspace/AnnotAgent/workspace/.annotagent/backups/session-cleanup-20260910-024650/`.

Port 8788 uses the standalone executable `/Users/oscar/Documents/my_workspace/AnnotAgent/workspace/.annotagent/bin/annotagent-session-cleanup` and the existing provider recovery web build (working directory `/Users/oscar/Documents/my_workspace/AnnotAgent-provider-recovery`). Port 8787 was not restarted. Do not overwrite the live executable or rebuild other frontend branches for this change.

Validation: targeted storage regression for owned keyset pagination, hidden tasks, display-title overrides, preserved original messages and restoration passed. CLI/server build passed. No real model calls, publication, annotation acceptance or export was triggered. Main and the developers' source branches were not edited; integrate this commit to retain the navigation behavior in a later server build.
