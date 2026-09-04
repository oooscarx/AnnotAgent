# Workspace Integrity Summary-Query Evidence

This record describes the bounded list paths introduced in M9. It is not a synthetic throughput claim; the executable fixture and query contracts are the evidence.

## Purpose-built paths

| Product read | Implementation | Bound and expansion rule |
| --- | --- | --- |
| Project index | `Application::list_projects_summary` | Sort stable Project IDs first, then load only the requested page; default 50, maximum 100 |
| Global execution index | `SqliteStore::list_executions_summary` | One bounded summary statement plus one count; order by `updated_at DESC, id DESC` |
| Project Runs | `SqliteStore::list_project_runs_summary` | Same fixed two-statement shape with stable `project_id` scope |
| Batch image list | `SqliteStore::list_batch_images_summary` | One query for image/run status, final count, Review count, and Review IDs; no per-image Run History load |
| Review index | `SqliteStore::list_review_summary` | One bounded owner/Annotation summary statement plus one count; evidence and revision history load only from exact detail |
| Review progress | `SqliteStore::review_counts` | One aggregate query, globally or by stable Project ID |

All SQL-backed list pages use stable tie-break ordering. Migration `0017_bounded_workspace_summaries.sql` provides owner/order indexes for Runs, Batch children, Run images, pending Reviews, revisions, task state, validation issues, usage, model calls, and Sample Test hash lookup.

## Scale fixture

`summary::tests::bounded_summaries_handle_100_projects_1000_runs_and_reviews` creates 100 persisted Project identities, 1,000 Runs, 1,000 stable Run-image relations, and 1,000 Review annotations. Every Run also receives deliberately malformed `run_events.event_json`; the bounded Run and Review pages still succeed, proving that index reads do not deserialize full History.

The fixture asserts:

- a requested 25-Run page returns 25 of 1,000 with `next_offset = 25`;
- a requested 40-Review page at offset 80 returns 40 with `next_offset = 120`;
- aggregate Review progress reports all 1,000 pending items;
- required owner/order indexes exist.

Executed on 2026-09-04:

```text
cargo test -p annotagent-storage summary::tests::bounded_summaries_handle_100_projects_1000_runs_and_reviews -- --exact --nocapture
test result: ok. 1 passed; test execution 0.09s
```

The fixed page/count statement count is encoded by the summary methods themselves; complete events, tool calls, messages, checkpoints, and Artifact payloads have no list-query dependency. Exact Run and Review deep links use separate stable-ID detail paths, so pagination cannot make older objects unreachable.
