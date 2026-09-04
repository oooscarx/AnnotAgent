use annotagent_core::{
    Annotation, BatchId, BatchProgress, BatchRecord, ImageId, ProjectId, RunId, TaskRunStatus,
};
use rusqlite::{OptionalExtension, params};

use crate::{HistoryRun, SqliteStore, StorageError, batch::batch_from_row, history_run_from_row};

pub const DEFAULT_PAGE_LIMIT: usize = 50;
pub const MAX_PAGE_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: usize,
    pub offset: usize,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: DEFAULT_PAGE_LIMIT,
            offset: 0,
        }
    }
}

impl PageRequest {
    #[must_use]
    pub fn bounded(limit: Option<usize>, offset: Option<usize>) -> Self {
        Self {
            limit: limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT),
            offset: offset.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SummaryPage<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

impl<T> SummaryPage<T> {
    fn new(items: Vec<T>, total: usize, request: PageRequest) -> Self {
        let consumed = request.offset.saturating_add(items.len());
        Self {
            items,
            total,
            limit: request.limit,
            offset: request.offset,
            next_offset: (consumed < total).then_some(consumed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredRunSummary {
    pub run: HistoryRun,
    pub image_id: Option<ImageId>,
    pub image_count: usize,
    pub batch_id: Option<BatchId>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: String,
    pub retry_count: u32,
    pub current_node: Option<String>,
    pub current_node_status: Option<TaskRunStatus>,
    pub artifact_count: usize,
    pub validation_issue_codes: Vec<String>,
    pub timed_out: bool,
    pub review_suspended: bool,
    pub result_count: usize,
    pub ready_count: usize,
    pub needs_review_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectExecutionHead {
    pub active_run: Option<HistoryRun>,
    pub last_run: Option<HistoryRun>,
}

#[derive(Debug, Clone)]
pub struct StoredBatchSummary {
    pub batch: BatchRecord,
    pub progress: BatchProgress,
    pub child_run_ids: Vec<RunId>,
}

#[derive(Debug, Clone)]
pub struct StoredReviewSummary {
    pub run: HistoryRun,
    pub annotation: Annotation,
    pub validation_issue_codes: Vec<String>,
    pub image_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReviewCountSummary {
    pub reviewed_count: usize,
    pub remaining_count: usize,
}

impl SqliteStore {
    /// Global execution-index query. The bounded page and its total use two SQL statements,
    /// independent of the number of Runs in the workspace.
    pub fn list_executions_summary(
        &self,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredRunSummary>, StorageError> {
        self.list_run_summaries(None, request)
    }

    /// Project-owned Run-index query. The bounded page and its total use two SQL statements,
    /// independent of the number of Runs owned by the Project.
    pub fn list_project_runs_summary(
        &self,
        project_id: ProjectId,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredRunSummary>, StorageError> {
        self.list_run_summaries(Some(project_id), request)
    }

    pub fn project_execution_head(
        &self,
        project_id: ProjectId,
    ) -> Result<ProjectExecutionHead, StorageError> {
        self.with_connection(|connection| {
            let select = "SELECT id, project_id, project_name, skill_id, provider, model, status,
                                 project_schema_json, workflow_snapshot_json, terminal_reason,
                                 created_at, updated_at
                          FROM runs WHERE project_id = ?1";
            let active_sql = format!(
                "{select} AND status IN ('pending', 'running', 'paused', 'awaiting_review')
                 ORDER BY updated_at DESC, id DESC LIMIT 1"
            );
            let last_sql = format!(
                "{select} AND status NOT IN ('pending', 'running', 'paused', 'awaiting_review')
                 ORDER BY updated_at DESC, id DESC LIMIT 1"
            );
            let parameter = project_id.to_string();
            Ok(ProjectExecutionHead {
                active_run: connection
                    .query_row(&active_sql, [&parameter], history_run_from_row)
                    .optional()?,
                last_run: connection
                    .query_row(&last_sql, [&parameter], history_run_from_row)
                    .optional()?,
            })
        })
    }

    pub fn run_event_count(&self, run_id: RunId) -> Result<usize, StorageError> {
        self.with_connection(|connection| {
            let count = connection.query_row(
                "SELECT COUNT(*) FROM run_events WHERE run_id = ?1",
                [run_id.to_string()],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(i64_to_usize(count))
        })
    }

    pub fn run_has_pending_review(&self, run_id: RunId) -> Result<bool, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM annotations WHERE run_id = ?1 AND review_status = 'needs_review')",
                    [run_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(StorageError::from)
        })
    }

    pub fn project_has_completed_run(
        &self,
        project_id: ProjectId,
        published_workflow: Option<(&str, u32)>,
    ) -> Result<bool, StorageError> {
        self.with_connection(|connection| {
            let completed = if let Some((workflow_id, version)) = published_workflow {
                connection.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM runs
                        WHERE project_id = ?1
                          AND status IN ('completed', 'completed_with_review', 'partial')
                          AND json_extract(workflow_snapshot_json, '$.selected_workflow.workflow_id') = ?2
                          AND json_extract(workflow_snapshot_json, '$.selected_workflow.version') = ?3
                    )",
                    params![project_id.to_string(), workflow_id, i64::from(version)],
                    |row| row.get(0),
                )?
            } else {
                connection.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM runs
                        WHERE project_id = ?1
                          AND status IN ('completed', 'completed_with_review', 'partial')
                    )",
                    [project_id.to_string()],
                    |row| row.get(0),
                )?
            };
            Ok(completed)
        })
    }

    /// Read Dataset execution list rows, aggregate progress, and child identities in a bounded
    /// page. Batch image payloads and child Run histories are not loaded.
    pub fn list_batch_summaries(
        &self,
        project_id: Option<&str>,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredBatchSummary>, StorageError> {
        self.with_connection(|connection| {
            let filter = project_id.map_or("", |_| "WHERE b.project_id = ?1");
            let sql = format!(
                "SELECT b.id, b.project_id, b.project_path, b.provider, b.status,
                        b.max_concurrency, b.workflow_version, b.workflow_snapshot_json,
                        b.project_snapshot_json, b.budget_limits_json, b.budget_ledger_json,
                        b.lease_owner, b.lease_expires_at, b.event_sequence, b.created_at,
                        b.updated_at,
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status IN ('pending', 'leased')),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status = 'running'),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status = 'completed'),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status = 'failed'),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status = 'awaiting_review'),
                        (SELECT COUNT(*) FROM batch_images bi WHERE bi.batch_id = b.id AND bi.status = 'cancelled'),
                        (SELECT GROUP_CONCAT(child_run_id, ',') FROM batch_images bi WHERE bi.batch_id = b.id AND child_run_id IS NOT NULL)
                 FROM dataset_batches b {filter}
                 ORDER BY b.updated_at DESC, b.id DESC
                 LIMIT ?{limit_parameter} OFFSET ?{offset_parameter}",
                limit_parameter = if project_id.is_some() { 2 } else { 1 },
                offset_parameter = if project_id.is_some() { 3 } else { 2 },
            );
            let mut statement = connection.prepare(&sql)?;
            let items = if let Some(project_id) = project_id {
                statement
                    .query_map(
                        params![
                            project_id,
                            usize_to_i64(request.limit),
                            usize_to_i64(request.offset)
                        ],
                        stored_batch_summary_from_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map(
                        params![usize_to_i64(request.limit), usize_to_i64(request.offset)],
                        stored_batch_summary_from_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let total = if let Some(project_id) = project_id {
                connection.query_row(
                    "SELECT COUNT(*) FROM dataset_batches WHERE project_id = ?1",
                    [project_id],
                    |row| row.get::<_, i64>(0),
                )?
            } else {
                connection.query_row("SELECT COUNT(*) FROM dataset_batches", [], |row| {
                    row.get::<_, i64>(0)
                })?
            };
            Ok(SummaryPage::new(items, i64_to_usize(total), request))
        })
    }

    /// Read the complete data needed by a Run list row in two SQL statements: one bounded page
    /// and one count. It never deserializes History events, Artifacts, messages, or tool calls.
    fn list_run_summaries(
        &self,
        project_id: Option<ProjectId>,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredRunSummary>, StorageError> {
        self.with_connection(|connection| {
            let filter = project_id.map_or("", |_| "WHERE r.project_id = ?1");
            let sql = format!(
                "SELECT r.id, r.project_id, r.project_name, r.skill_id, r.provider, r.model,
                        r.status, r.project_schema_json, r.workflow_snapshot_json,
                        r.terminal_reason, r.created_at, r.updated_at,
                        CASE WHEN (SELECT COUNT(*) FROM run_images ri WHERE ri.run_id = r.id) = 1
                             THEN (SELECT ri.image_id FROM run_images ri WHERE ri.run_id = r.id LIMIT 1)
                             ELSE NULL END,
                        (SELECT COUNT(*) FROM run_images ri WHERE ri.run_id = r.id),
                        (SELECT bi.batch_id FROM batch_images bi WHERE bi.child_run_id = r.id LIMIT 1),
                        COALESCE((SELECT SUM(u.input_tokens) FROM usage_records u WHERE u.run_id = r.id), 0),
                        COALESCE((SELECT SUM(u.output_tokens) FROM usage_records u WHERE u.run_id = r.id), 0),
                        COALESCE((SELECT SUM(CAST(u.cost AS REAL)) FROM usage_records u WHERE u.run_id = r.id), 0.0),
                        COALESCE((SELECT SUM(mc.retry_count) FROM model_calls mc WHERE mc.run_id = r.id), 0),
                        (SELECT tr.task_id FROM task_runs tr WHERE tr.run_id = r.id ORDER BY tr.updated_at DESC, tr.task_id DESC LIMIT 1),
                        (SELECT tr.status FROM task_runs tr WHERE tr.run_id = r.id ORDER BY tr.updated_at DESC, tr.task_id DESC LIMIT 1),
                        (SELECT COUNT(*) FROM vision_artifacts va WHERE va.run_id = r.id),
                        (SELECT GROUP_CONCAT(code, ',') FROM (SELECT DISTINCT vi.code AS code FROM validation_issues vi WHERE vi.run_id = r.id ORDER BY vi.code)),
                        CASE WHEN lower(COALESCE(r.terminal_reason, '')) LIKE '%timeout%'
                                  OR EXISTS(SELECT 1 FROM run_events re WHERE re.run_id = r.id AND lower(re.event_json) LIKE '%timeout%')
                             THEN 1 ELSE 0 END,
                        CASE WHEN r.status = 'awaiting_review'
                                  OR EXISTS(SELECT 1 FROM task_runs tr WHERE tr.run_id = r.id AND tr.status = 'needs_review')
                             THEN 1 ELSE 0 END,
                        (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status IN ('auto_accepted', 'human_accepted', 'needs_review')),
                        (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status IN ('auto_accepted', 'human_accepted')),
                        (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status = 'needs_review')
                 FROM runs r {filter}
                 ORDER BY r.updated_at DESC, r.id DESC
                 LIMIT ?{limit_parameter} OFFSET ?{offset_parameter}",
                limit_parameter = if project_id.is_some() { 2 } else { 1 },
                offset_parameter = if project_id.is_some() { 3 } else { 2 },
            );
            let mut statement = connection.prepare(&sql)?;
            let map_row = |row: &rusqlite::Row<'_>| stored_run_summary_from_row(row);
            let items = if let Some(project_id) = project_id {
                statement
                    .query_map(
                        params![
                            project_id.to_string(),
                            usize_to_i64(request.limit),
                            usize_to_i64(request.offset)
                        ],
                        map_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map(
                        params![usize_to_i64(request.limit), usize_to_i64(request.offset)],
                        map_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let total = if let Some(project_id) = project_id {
                connection.query_row(
                    "SELECT COUNT(*) FROM runs WHERE project_id = ?1",
                    [project_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?
            } else {
                connection.query_row("SELECT COUNT(*) FROM runs", [], |row| row.get::<_, i64>(0))?
            };
            Ok(SummaryPage::new(items, i64_to_usize(total), request))
        })
    }

    pub fn get_run_summary(&self, run_id: RunId) -> Result<StoredRunSummary, StorageError> {
        let page = self.list_run_summaries_by_id(run_id)?;
        page.ok_or(StorageError::RunNotFound(run_id))
    }

    fn list_run_summaries_by_id(
        &self,
        run_id: RunId,
    ) -> Result<Option<StoredRunSummary>, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT r.id, r.project_id, r.project_name, r.skill_id, r.provider, r.model,
                            r.status, r.project_schema_json, r.workflow_snapshot_json,
                            r.terminal_reason, r.created_at, r.updated_at,
                            CASE WHEN (SELECT COUNT(*) FROM run_images ri WHERE ri.run_id = r.id) = 1
                                 THEN (SELECT ri.image_id FROM run_images ri WHERE ri.run_id = r.id LIMIT 1)
                                 ELSE NULL END,
                            (SELECT COUNT(*) FROM run_images ri WHERE ri.run_id = r.id),
                            (SELECT bi.batch_id FROM batch_images bi WHERE bi.child_run_id = r.id LIMIT 1),
                            COALESCE((SELECT SUM(u.input_tokens) FROM usage_records u WHERE u.run_id = r.id), 0),
                            COALESCE((SELECT SUM(u.output_tokens) FROM usage_records u WHERE u.run_id = r.id), 0),
                            COALESCE((SELECT SUM(CAST(u.cost AS REAL)) FROM usage_records u WHERE u.run_id = r.id), 0.0),
                            COALESCE((SELECT SUM(mc.retry_count) FROM model_calls mc WHERE mc.run_id = r.id), 0),
                            (SELECT tr.task_id FROM task_runs tr WHERE tr.run_id = r.id ORDER BY tr.updated_at DESC, tr.task_id DESC LIMIT 1),
                            (SELECT tr.status FROM task_runs tr WHERE tr.run_id = r.id ORDER BY tr.updated_at DESC, tr.task_id DESC LIMIT 1),
                            (SELECT COUNT(*) FROM vision_artifacts va WHERE va.run_id = r.id),
                            (SELECT GROUP_CONCAT(code, ',') FROM (SELECT DISTINCT vi.code AS code FROM validation_issues vi WHERE vi.run_id = r.id ORDER BY vi.code)),
                            CASE WHEN lower(COALESCE(r.terminal_reason, '')) LIKE '%timeout%'
                                      OR EXISTS(SELECT 1 FROM run_events re WHERE re.run_id = r.id AND lower(re.event_json) LIKE '%timeout%')
                                 THEN 1 ELSE 0 END,
                            CASE WHEN r.status = 'awaiting_review'
                                      OR EXISTS(SELECT 1 FROM task_runs tr WHERE tr.run_id = r.id AND tr.status = 'needs_review')
                                 THEN 1 ELSE 0 END,
                            (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status IN ('auto_accepted', 'human_accepted', 'needs_review')),
                            (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status IN ('auto_accepted', 'human_accepted')),
                            (SELECT COUNT(*) FROM annotations a WHERE a.run_id = r.id AND a.review_status = 'needs_review')
                     FROM runs r WHERE r.id = ?1",
                    [run_id.to_string()],
                    stored_run_summary_from_row,
                )
                .optional()
                .map_err(StorageError::from)
        })
    }

    /// A Review queue page reads only pending annotation payloads and their owning Run rows.
    /// Artifact/evidence expansion is deferred to the exact Review detail endpoint.
    pub fn list_review_summary(
        &self,
        project_id: Option<ProjectId>,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredReviewSummary>, StorageError> {
        self.with_connection(|connection| {
            let filter = project_id.map_or(
                "a.review_status = 'needs_review'",
                |_| "a.review_status = 'needs_review' AND r.project_id = ?1",
            );
            let sql = format!(
                "SELECT r.id, r.project_id, r.project_name, r.skill_id, r.provider, r.model,
                        r.status, r.project_schema_json, r.workflow_snapshot_json,
                        r.terminal_reason, r.created_at, r.updated_at, a.annotation_json,
                        (SELECT GROUP_CONCAT(code, ',') FROM (
                            SELECT DISTINCT vi.code AS code FROM validation_issues vi
                            WHERE vi.run_id = r.id
                              AND (json_array_length(json_extract(vi.issue_json, '$.annotation_ids')) = 0
                                   OR vi.issue_json LIKE '%' || a.id || '%')
                            ORDER BY vi.code
                        )),
                        (SELECT COUNT(*) FROM images before_image
                         WHERE before_image.project_id = r.project_id
                           AND before_image.relative_path < (
                               SELECT current_image.relative_path FROM images current_image
                               WHERE current_image.id = a.image_id LIMIT 1
                           ))
                 FROM annotations a
                 JOIN runs r ON r.id = a.run_id
                 WHERE {filter}
                 ORDER BY a.created_at, a.id
                 LIMIT ?{limit_parameter} OFFSET ?{offset_parameter}",
                limit_parameter = if project_id.is_some() { 2 } else { 1 },
                offset_parameter = if project_id.is_some() { 3 } else { 2 },
            );
            let mut statement = connection.prepare(&sql)?;
            let items = if let Some(project_id) = project_id {
                statement
                    .query_map(
                        params![
                            project_id.to_string(),
                            usize_to_i64(request.limit),
                            usize_to_i64(request.offset)
                        ],
                        stored_review_summary_from_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map(
                        params![usize_to_i64(request.limit), usize_to_i64(request.offset)],
                        stored_review_summary_from_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let total = review_count_query(connection, project_id, "needs_review")?;
            Ok(SummaryPage::new(items, total, request))
        })
    }

    pub fn list_run_review_summaries(
        &self,
        run_id: RunId,
        request: PageRequest,
    ) -> Result<SummaryPage<StoredReviewSummary>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT r.id, r.project_id, r.project_name, r.skill_id, r.provider, r.model,
                        r.status, r.project_schema_json, r.workflow_snapshot_json,
                        r.terminal_reason, r.created_at, r.updated_at, a.annotation_json,
                        (SELECT GROUP_CONCAT(code, ',') FROM (
                            SELECT DISTINCT vi.code AS code FROM validation_issues vi
                            WHERE vi.run_id = r.id
                              AND (json_array_length(json_extract(vi.issue_json, '$.annotation_ids')) = 0
                                   OR vi.issue_json LIKE '%' || a.id || '%')
                            ORDER BY vi.code
                        )),
                        (SELECT COUNT(*) FROM images before_image
                         WHERE before_image.project_id = r.project_id
                           AND before_image.relative_path < (
                               SELECT current_image.relative_path FROM images current_image
                               WHERE current_image.id = a.image_id LIMIT 1
                           ))
                 FROM annotations a
                 JOIN runs r ON r.id = a.run_id
                 WHERE a.review_status = 'needs_review' AND r.id = ?1
                 ORDER BY a.created_at, a.id
                 LIMIT ?2 OFFSET ?3",
            )?;
            let items = statement
                .query_map(
                    params![
                        run_id.to_string(),
                        usize_to_i64(request.limit),
                        usize_to_i64(request.offset)
                    ],
                    stored_review_summary_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            let total = connection.query_row(
                "SELECT COUNT(*) FROM annotations WHERE run_id = ?1 AND review_status = 'needs_review'",
                [run_id.to_string()],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(SummaryPage::new(items, i64_to_usize(total), request))
        })
    }

    pub fn review_counts(
        &self,
        project_id: Option<ProjectId>,
    ) -> Result<ReviewCountSummary, StorageError> {
        self.with_connection(|connection| {
            let filter = project_id.map_or("", |_| "WHERE r.project_id = ?1");
            let sql = format!(
                "SELECT
                    COALESCE(SUM(CASE WHEN a.review_status IN ('human_accepted', 'rejected') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.review_status = 'needs_review' THEN 1 ELSE 0 END), 0)
                 FROM annotations a JOIN runs r ON r.id = a.run_id {filter}"
            );
            let (reviewed, remaining) = if let Some(project_id) = project_id {
                connection.query_row(&sql, [project_id.to_string()], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })?
            } else {
                connection.query_row(&sql, [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })?
            };
            Ok(ReviewCountSummary {
                reviewed_count: i64_to_usize(reviewed),
                remaining_count: i64_to_usize(remaining),
            })
        })
    }

    pub fn review_counts_for_run(&self, run_id: RunId) -> Result<ReviewCountSummary, StorageError> {
        self.with_connection(|connection| {
            let (reviewed, remaining) = connection.query_row(
                "SELECT
                    COALESCE(SUM(CASE WHEN review_status IN ('human_accepted', 'rejected') THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN review_status = 'needs_review' THEN 1 ELSE 0 END), 0)
                 FROM annotations WHERE run_id = ?1",
                [run_id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )?;
            Ok(ReviewCountSummary {
                reviewed_count: i64_to_usize(reviewed),
                remaining_count: i64_to_usize(remaining),
            })
        })
    }
}

fn stored_batch_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredBatchSummary> {
    let batch = batch_from_row(row)?;
    let child_run_ids = row
        .get::<_, Option<String>>(23)?
        .map(|value| {
            value
                .split(',')
                .filter(|id| !id.is_empty())
                .map(|id| id.parse().map_err(|error| conversion_error(23, error)))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(StoredBatchSummary {
        batch,
        progress: BatchProgress {
            total_images: i64_to_u64(row.get(16)?),
            pending_images: i64_to_u64(row.get(17)?),
            running_images: i64_to_u64(row.get(18)?),
            completed_images: i64_to_u64(row.get(19)?),
            failed_images: i64_to_u64(row.get(20)?),
            review_images: i64_to_u64(row.get(21)?),
            cancelled_images: i64_to_u64(row.get(22)?),
        },
        child_run_ids,
    })
}

fn stored_run_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRunSummary> {
    let run = history_run_from_row(row)?;
    let image_id = row
        .get::<_, Option<String>>(12)?
        .map(|value| value.parse())
        .transpose()
        .map_err(|error| conversion_error(12, error))?;
    let batch_id = row
        .get::<_, Option<String>>(14)?
        .map(|value| value.parse())
        .transpose()
        .map_err(|error| conversion_error(14, error))?;
    let current_node_status = row
        .get::<_, Option<String>>(20)?
        .map(|value| serde_json::from_value(serde_json::Value::String(value)))
        .transpose()
        .map_err(|error| conversion_error(20, error))?;
    let mut validation_issue_codes = row
        .get::<_, Option<String>>(22)?
        .map(|value| {
            value
                .split(',')
                .filter(|code| !code.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    validation_issue_codes.sort();
    validation_issue_codes.dedup();
    Ok(StoredRunSummary {
        run,
        image_id,
        image_count: i64_to_usize(row.get(13)?),
        batch_id,
        input_tokens: i64_to_u64(row.get(15)?),
        output_tokens: i64_to_u64(row.get(16)?),
        cost: format!("{:.6}", row.get::<_, f64>(17)?),
        retry_count: i64_to_u32(row.get(18)?),
        current_node: row.get(19)?,
        current_node_status,
        artifact_count: i64_to_usize(row.get(21)?),
        validation_issue_codes,
        timed_out: row.get::<_, i64>(23)? != 0,
        review_suspended: row.get::<_, i64>(24)? != 0,
        result_count: i64_to_usize(row.get(25)?),
        ready_count: i64_to_usize(row.get(26)?),
        needs_review_count: i64_to_usize(row.get(27)?),
    })
}

fn stored_review_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredReviewSummary> {
    let run = history_run_from_row(row)?;
    let annotation_json = row.get::<_, String>(12)?;
    let annotation =
        serde_json::from_str(&annotation_json).map_err(|error| conversion_error(12, error))?;
    let validation_issue_codes = row
        .get::<_, Option<String>>(13)?
        .map(|value| {
            value
                .split(',')
                .filter(|code| !code.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let image_index = row.get::<_, Option<i64>>(14)?.map(i64_to_usize);
    Ok(StoredReviewSummary {
        run,
        annotation,
        validation_issue_codes,
        image_index,
    })
}

fn review_count_query(
    connection: &rusqlite::Connection,
    project_id: Option<ProjectId>,
    review_status: &str,
) -> Result<usize, StorageError> {
    let count = if let Some(project_id) = project_id {
        connection.query_row(
            "SELECT COUNT(*) FROM annotations a JOIN runs r ON r.id = a.run_id
             WHERE a.review_status = ?1 AND r.project_id = ?2",
            params![review_status, project_id.to_string()],
            |row| row.get::<_, i64>(0),
        )?
    } else {
        connection.query_row(
            "SELECT COUNT(*) FROM annotations WHERE review_status = ?1",
            [review_status],
            |row| row.get::<_, i64>(0),
        )?
    };
    Ok(i64_to_usize(count))
}

fn conversion_error(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

fn i64_to_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

fn i64_to_u64(value: i64) -> u64 {
    u64::try_from(value.max(0)).unwrap_or(u64::MAX)
}

fn i64_to_u32(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{
        AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, LabelId,
        NormalizedRect, ReviewStatus, TaskId,
    };
    use chrono::Utc;
    use rusqlite::params;

    use super::*;

    #[test]
    fn bounded_summaries_handle_100_projects_1000_runs_and_reviews() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        store
            .with_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                for index in 0..100 {
                    transaction.execute(
                        "INSERT INTO projects (id, name, config_json, created_at)
                         VALUES (?1, ?2, '{}', ?3)",
                        params![
                            format!("project-{index:03}"),
                            format!("Project {index:03}"),
                            format!("2026-01-01T00:00:{index:03}Z"),
                        ],
                    )?;
                }
                for index in 0..1000 {
                    let run_id = RunId::new();
                    let image_id = ImageId::new();
                    let annotation = Annotation {
                        id: AnnotationId::new(),
                        image_id,
                        task_id: TaskId::from("objects"),
                        label: Some(LabelId::from("ball")),
                        value: AnnotationValue::BoundingBox {
                            rect: NormalizedRect::new(0.1, 0.2, 0.1, 0.1).expect("normalized box"),
                        },
                        attributes: BTreeMap::new(),
                        confidence: Some(0.5),
                        source: AnnotationSource::Model,
                        review_status: ReviewStatus::NeedsReview,
                        provenance: AnnotationProvenance::default(),
                        created_at: Utc::now(),
                    };
                    let timestamp = format!("2026-02-01T00:{index:04}:00Z");
                    transaction.execute(
                        "INSERT INTO runs
                         (id, project_id, project_name, skill_id, provider, model, status,
                          project_schema_json, workflow_snapshot_json, terminal_reason,
                          created_at, updated_at)
                         VALUES (?1, ?2, 'Fixture', 'none', 'core', 'none',
                                 'completed_with_review', '{}', NULL, NULL, ?3, ?3)",
                        params![run_id.to_string(), project_id.to_string(), timestamp],
                    )?;
                    transaction.execute(
                        "INSERT INTO run_images (run_id, image_id, status)
                         VALUES (?1, ?2, 'completed_with_review')",
                        params![run_id.to_string(), image_id.to_string()],
                    )?;
                    transaction.execute(
                        "INSERT INTO annotations
                         (id, run_id, image_id, task_id, label, review_status,
                          annotation_json, created_at)
                         VALUES (?1, ?2, ?3, 'objects', 'ball', 'needs_review', ?4, ?5)",
                        params![
                            annotation.id.to_string(),
                            run_id.to_string(),
                            image_id.to_string(),
                            serde_json::to_string(&annotation)?,
                            annotation.created_at.to_rfc3339(),
                        ],
                    )?;
                    // A list query must not deserialize History payloads. The exact detail API
                    // would reject this deliberately malformed fixture, while summaries remain
                    // independently readable.
                    transaction.execute(
                        "INSERT INTO run_events
                         (event_id, run_id, event_kind, event_json, occurred_at)
                         VALUES (?1, ?2, 'task_started', '{malformed-history', ?3)",
                        params![
                            uuid::Uuid::new_v4().to_string(),
                            run_id.to_string(),
                            timestamp
                        ],
                    )?;
                }
                transaction.commit()?;
                Ok(())
            })
            .expect("large summary fixture");

        let runs = store
            .list_project_runs_summary(project_id, PageRequest::bounded(Some(25), None))
            .expect("bounded Runs");
        assert_eq!(runs.total, 1000);
        assert_eq!(runs.items.len(), 25);
        assert_eq!(runs.next_offset, Some(25));
        assert!(runs.items.iter().all(|summary| summary.image_count == 1));

        let reviews = store
            .list_review_summary(Some(project_id), PageRequest::bounded(Some(40), Some(80)))
            .expect("bounded Reviews");
        assert_eq!(reviews.total, 1000);
        assert_eq!(reviews.items.len(), 40);
        assert_eq!(reviews.next_offset, Some(120));
        assert_eq!(
            store
                .review_counts(Some(project_id))
                .expect("Review counts"),
            ReviewCountSummary {
                reviewed_count: 0,
                remaining_count: 1000,
            }
        );

        let indexes = store
            .with_connection(|connection| {
                let mut statement = connection
                    .prepare("SELECT name FROM sqlite_master WHERE type = 'index' ORDER BY name")?;
                Ok(statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?)
            })
            .expect("summary indexes");
        for required in [
            "idx_runs_project_updated_id",
            "idx_annotations_review_created",
            "idx_annotations_run_review_created",
            "idx_batch_images_child_run",
            "idx_workflow_sample_tests_draft_hash_completed",
        ] {
            assert!(
                indexes.iter().any(|index| index == required),
                "missing {required}"
            );
        }
    }
}
