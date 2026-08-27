//! `SQLite` persistence for projects, auditable runs, revisions, and correction memory.

mod batch;

pub use batch::BatchClaimResult;

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use annotagent_core::{
    Annotation, AnnotationRevision, AnnotationRevisionId, AnnotationValue, ArtifactId,
    ArtifactValidationState, CorrectionRecord, ImageId, LabelId, ModelMessage, ProjectId,
    PublishedWorkflowVersion, RelationEndpoint, RevisionActor, RunEvent, RunEventPayload, RunId,
    RunStatus, TaskId, TaskRunStatus, ToolCallId, ToolResult, UsageRecord, ValidationIssue,
    VisionArtifact, VisionArtifactValue, WorkflowDraft, WorkflowDraftStatus, WorkflowSnapshot,
};
use annotagent_runtime::{RunRecord, RuntimeStore};
use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/0001_initial.sql");
const BATCH_MIGRATION: &str =
    include_str!("../../../migrations/0003_persistent_dataset_batches.sql");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("history serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("database lock poisoned")]
    Poisoned,
    #[error("run {0} was not found")]
    RunNotFound(RunId),
    #[error("dataset batch {0} was not found")]
    BatchNotFound(annotagent_core::BatchId),
    #[error("dataset batch lease conflict: {0}")]
    BatchLeaseConflict(String),
    #[error("unsupported history schema version {0}")]
    UnsupportedHistoryVersion(u32),
    #[error("invalid stored enum value: {0}")]
    InvalidEnum(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryRun {
    pub id: RunId,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    pub project_name: String,
    pub skill_id: String,
    pub provider: String,
    pub model: String,
    pub status: RunStatus,
    pub project_schema_json: String,
    #[serde(default)]
    pub workflow_snapshot_json: Option<String>,
    pub terminal_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryToolCall {
    pub call_id: ToolCallId,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<ToolResult>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryModelMessage {
    pub image_id: Option<ImageId>,
    pub task_id: Option<TaskId>,
    pub message: ModelMessage,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryTaskRun {
    pub run_id: RunId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub status: TaskRunStatus,
    pub reason: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryDocument {
    pub schema_version: u32,
    pub run: HistoryRun,
    pub events: Vec<RunEvent>,
    pub annotations: Vec<Annotation>,
    pub revisions: Vec<AnnotationRevision>,
    #[serde(default)]
    pub task_runs: Vec<HistoryTaskRun>,
    pub usage: Vec<UsageRecord>,
    #[serde(default)]
    pub model_messages: Vec<HistoryModelMessage>,
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
    pub tool_calls: Vec<HistoryToolCall>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryImportReport {
    pub run_id: RunId,
    pub ids_remapped: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStartReservation {
    Reserved,
    Idempotent { run_id: RunId, status: RunStatus },
    Conflict { run_id: RunId, status: RunStatus },
}

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    pub fn update_run_workflow_snapshot(
        &self,
        run_id: RunId,
        snapshot_json: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE runs SET workflow_snapshot_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id.to_string(), snapshot_json, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        let store = Self {
            connection: Mutex::new(connection),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let store = Self {
            connection: Mutex::new(Connection::open_in_memory()?),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute_batch(INITIAL_MIGRATION)?;
            let has_project_id = {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|name| name == "project_id")
            };
            if !has_project_id {
                connection.execute("ALTER TABLE runs ADD COLUMN project_id TEXT", [])?;
            }
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_runs_project_created ON runs(project_id, created_at DESC)",
                [],
            )?;
            let has_distinct_review_state = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 1)",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if !has_distinct_review_state {
                let transaction = connection.unchecked_transaction()?;
                transaction.execute(
                    "UPDATE runs SET status = 'completed_with_review' WHERE status = 'awaiting_review'",
                    [],
                )?;
                transaction.execute(
                    "UPDATE active_project_runs SET status = 'completed_with_review' WHERE status = 'awaiting_review'",
                    [],
                )?;
                transaction.execute(
                    "DELETE FROM active_project_runs WHERE status = 'completed_with_review'",
                    [],
                )?;
                transaction.execute(
                    "INSERT INTO schema_migrations(version, name, applied_at) VALUES (1, ?1, ?2)",
                    params!["distinct_awaiting_review_state", Utc::now().to_rfc3339()],
                )?;
                transaction.commit()?;
            }
            let has_workflow_snapshot = {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|name| name == "workflow_snapshot_json")
            };
            if !has_workflow_snapshot {
                connection.execute(
                    "ALTER TABLE runs ADD COLUMN workflow_snapshot_json TEXT",
                    [],
                )?;
            }
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (2, ?1, ?2)",
                params!["immutable_workflow_run_snapshot", Utc::now().to_rfc3339()],
            )?;
            connection.execute_batch(BATCH_MIGRATION)?;
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (3, ?1, ?2)",
                params!["persistent_dataset_batches", Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn schema_tables(&self) -> Result<Vec<String>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )?;
            let names = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(names)
        })
    }

    pub fn list_events(&self, run_id: RunId) -> Result<Vec<RunEvent>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT event_json FROM run_events WHERE run_id = ?1 ORDER BY sequence")?;
            statement
                .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn list_annotations(&self, run_id: RunId) -> Result<Vec<Annotation>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT annotation_json FROM annotations WHERE run_id = ?1 ORDER BY created_at",
            )?;
            statement
                .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn list_artifacts(&self, run_id: RunId) -> Result<Vec<VisionArtifact>, StorageError> {
        self.with_connection(|connection| {
            query_json_rows::<VisionArtifact>(
                connection,
                "SELECT artifact_json FROM vision_artifacts WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )
        })
    }

    pub fn find_annotation(
        &self,
        annotation_id: annotagent_core::AnnotationId,
    ) -> Result<Option<(RunId, Annotation)>, StorageError> {
        self.with_connection(|connection| {
            let row = connection
                .query_row(
                    "SELECT run_id, annotation_json FROM annotations WHERE id = ?1",
                    [annotation_id.to_string()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            row.map(|(run_id, annotation)| {
                Ok((
                    run_id.parse().map_err(|error| {
                        StorageError::InvalidEnum(format!("invalid run id: {error}"))
                    })?,
                    serde_json::from_str(&annotation)?,
                ))
            })
            .transpose()
        })
    }

    pub fn update_annotation(
        &self,
        annotation: &Annotation,
        reason: Option<&str>,
    ) -> Result<AnnotationRevision, StorageError> {
        annotation
            .validate()
            .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let before_json: String = transaction
                .query_row(
                    "SELECT annotation_json FROM annotations WHERE id = ?1",
                    [annotation.id.to_string()],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("annotation {} was not found", annotation.id))
                })?;
            let before: Annotation = serde_json::from_str(&before_json)?;
            let parent_revision_id = transaction
                .query_row(
                    "SELECT revision_id FROM annotation_revisions
                     WHERE annotation_id = ?1 ORDER BY created_at DESC LIMIT 1",
                    [annotation.id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|id| id.parse())
                .transpose()
                .map_err(|error| {
                    StorageError::InvalidEnum(format!("invalid revision id: {error}"))
                })?;
            let revision = AnnotationRevision {
                revision_id: AnnotationRevisionId::new(),
                annotation_id: annotation.id,
                parent_revision_id,
                before: Some(before.snapshot()),
                after: Some(annotation.snapshot()),
                actor: RevisionActor::Human,
                reason: reason.map(str::to_owned),
                created_at: Utc::now(),
            };
            revision
                .validate()
                .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
            transaction.execute(
                "UPDATE annotations SET label = ?2, review_status = ?3, annotation_json = ?4
                 WHERE id = ?1",
                params![
                    annotation.id.to_string(),
                    annotation.label.as_ref().map(LabelId::as_str),
                    enum_string(annotation.review_status)?,
                    serde_json::to_string(annotation)?,
                ],
            )?;
            transaction.execute(
                "INSERT INTO annotation_revisions
                 (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    revision.revision_id.to_string(),
                    revision.annotation_id.to_string(),
                    revision.parent_revision_id.map(|id| id.to_string()),
                    serde_json::to_string(&revision)?,
                    revision.created_at.to_rfc3339(),
                ],
            )?;
            transaction.execute(
                "UPDATE review_queue SET status = ?2, resolved_at = ?3 WHERE annotation_id = ?1",
                params![
                    annotation.id.to_string(),
                    enum_string(annotation.review_status)?,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(revision)
        })
    }

    pub fn list_revisions(
        &self,
        annotation_id: annotagent_core::AnnotationId,
    ) -> Result<Vec<AnnotationRevision>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT revision_json FROM annotation_revisions
                 WHERE annotation_id = ?1 ORDER BY created_at",
            )?;
            statement
                .query_map([annotation_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn run_status(&self, run_id: RunId) -> Result<RunStatus, StorageError> {
        self.with_connection(|connection| {
            let status = connection
                .query_row(
                    "SELECT status FROM runs WHERE id = ?1",
                    [run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::RunNotFound(run_id))?;
            serde_json::from_value(serde_json::Value::String(status)).map_err(StorageError::from)
        })
    }

    pub fn list_runs(&self) -> Result<Vec<HistoryRun>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, project_id, project_name, skill_id, provider, model, status,
                        project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at
                 FROM runs ORDER BY created_at DESC",
            )?;
            statement
                .query_map([], history_run_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn list_task_runs(&self, run_id: RunId) -> Result<Vec<HistoryTaskRun>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT image_id, task_id, status, reason, updated_at
                 FROM task_runs WHERE run_id = ?1 ORDER BY updated_at, task_id",
            )?;
            statement
                .query_map([run_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let status_text = row.get::<_, String>(2)?;
                    let status = serde_json::from_value(serde_json::Value::String(status_text))
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(HistoryTaskRun {
                        run_id,
                        image_id,
                        task_id: TaskId::from(row.get::<_, String>(1)?),
                        status,
                        reason: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                })?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn reserve_project_run(
        &self,
        project_id: ProjectId,
        run_id: RunId,
        idempotency_key: Option<&str>,
    ) -> Result<RunStartReservation, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            if let Some(key) = idempotency_key {
                let existing = transaction
                    .query_row(
                        "SELECT r.run_id, COALESCE(a.status, runs.status, 'interrupted')
                         FROM run_start_requests r
                         LEFT JOIN active_project_runs a ON a.run_id = r.run_id
                         LEFT JOIN runs ON runs.id = r.run_id
                         WHERE r.project_id = ?1 AND r.idempotency_key = ?2",
                        params![project_id.to_string(), key],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                if let Some((existing_id, status)) = existing {
                    transaction.commit()?;
                    return Ok(RunStartReservation::Idempotent {
                        run_id: parse_run_id(&existing_id)?,
                        status: parse_run_status(&status)?,
                    });
                }
            }
            let active = transaction
                .query_row(
                    "SELECT run_id, status FROM active_project_runs WHERE project_id = ?1",
                    [project_id.to_string()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            if let Some((active_id, status)) = active {
                transaction.commit()?;
                return Ok(RunStartReservation::Conflict {
                    run_id: parse_run_id(&active_id)?,
                    status: parse_run_status(&status)?,
                });
            }
            let now = Utc::now().to_rfc3339();
            if let Some(key) = idempotency_key {
                transaction.execute(
                    "INSERT INTO run_start_requests (project_id, idempotency_key, run_id, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![project_id.to_string(), key, run_id.to_string(), now],
                )?;
            }
            transaction.execute(
                "INSERT INTO active_project_runs (project_id, run_id, status, idempotency_key, updated_at)
                 VALUES (?1, ?2, 'pending', ?3, ?4)",
                params![project_id.to_string(), run_id.to_string(), idempotency_key, now],
            )?;
            transaction.commit()?;
            Ok(RunStartReservation::Reserved)
        })
    }

    pub fn reconcile_interrupted_runs(&self) -> Result<Vec<RunId>, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let run_ids = {
                let mut statement =
                    transaction.prepare("SELECT run_id FROM active_project_runs")?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .map(|row| parse_run_id(&row?))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let now = Utc::now().to_rfc3339();
            for run_id in &run_ids {
                transaction.execute(
                    "UPDATE runs SET status = 'interrupted', terminal_reason = ?2, updated_at = ?3
                     WHERE id = ?1 AND status IN ('pending', 'running', 'paused', 'awaiting_review')",
                    params![
                        run_id.to_string(),
                        "run was interrupted because no worker lease survived server startup",
                        now
                    ],
                )?;
            }
            transaction.execute("DELETE FROM active_project_runs", [])?;
            transaction.commit()?;
            Ok(run_ids)
        })
    }

    pub fn save_workflow_draft(&self, draft: &WorkflowDraft) -> Result<(), StorageError> {
        let status = enum_string(draft.status)?;
        let draft_json = serde_json::to_string(draft)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO workflow_drafts
                 (id, project_id, status, draft_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   status = excluded.status,
                   draft_json = excluded.draft_json,
                   updated_at = excluded.updated_at",
                params![
                    draft.id,
                    draft.project_id,
                    status,
                    draft_json,
                    draft.created_at.to_rfc3339(),
                    draft.updated_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_workflow_drafts(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<WorkflowDraft>, StorageError> {
        self.with_connection(|connection| {
            let (sql, parameter) = project_id.map_or(
                ("SELECT draft_json FROM workflow_drafts ORDER BY updated_at DESC", None),
                |project_id| {
                    (
                        "SELECT draft_json FROM workflow_drafts WHERE project_id = ?1 ORDER BY updated_at DESC",
                        Some(project_id),
                    )
                },
            );
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(project_id) = parameter {
                statement
                    .query_map([project_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn get_workflow_draft(&self, id: &str) -> Result<WorkflowDraft, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT draft_json FROM workflow_drafts WHERE id = ?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("workflow draft {id:?} was not found"))
                })?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn publish_workflow_draft(
        &self,
        draft: &WorkflowDraft,
        content_hash: String,
        snapshot: WorkflowSnapshot,
    ) -> Result<PublishedWorkflowVersion, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let next_version: u32 = transaction.query_row(
                "SELECT COALESCE(MAX(version), 0) + 1 FROM workflow_versions WHERE workflow_id = ?1",
                [&draft.id],
                |row| row.get(0),
            )?;
            let mut published_draft = draft.clone();
            published_draft.status = WorkflowDraftStatus::Published;
            published_draft.updated_at = Utc::now();
            let version = PublishedWorkflowVersion {
                workflow_id: draft.id.clone(),
                version: next_version,
                project_id: draft.project_id.clone(),
                source_draft_id: draft.id.clone(),
                content_hash,
                draft: published_draft.clone(),
                snapshot,
                published_at: Utc::now(),
            };
            transaction.execute(
                "INSERT INTO workflow_versions
                 (workflow_id, version, project_id, source_draft_id, version_json, published_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    version.workflow_id,
                    version.version,
                    version.project_id,
                    version.source_draft_id,
                    serde_json::to_string(&version)?,
                    version.published_at.to_rfc3339()
                ],
            )?;
            transaction.execute(
                "UPDATE workflow_drafts SET status = 'published', draft_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    draft.id,
                    serde_json::to_string(&published_draft)?,
                    published_draft.updated_at.to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(version)
        })
    }

    pub fn list_published_workflow_versions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<PublishedWorkflowVersion>, StorageError> {
        self.with_connection(|connection| {
            let (sql, parameter) = project_id.map_or(
                (
                    "SELECT version_json FROM workflow_versions ORDER BY project_id, workflow_id, version",
                    None,
                ),
                |project_id| {
                    (
                        "SELECT version_json FROM workflow_versions WHERE project_id = ?1 ORDER BY workflow_id, version",
                        Some(project_id),
                    )
                },
            );
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(project_id) = parameter {
                statement
                    .query_map([project_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn get_published_workflow_version(
        &self,
        workflow_id: &str,
        version: u32,
    ) -> Result<PublishedWorkflowVersion, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT version_json FROM workflow_versions WHERE workflow_id = ?1 AND version = ?2",
                    params![workflow_id, version],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!(
                        "published workflow {workflow_id:?} version {version} was not found"
                    ))
                })?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn history(&self, run_id: RunId) -> Result<HistoryDocument, StorageError> {
        self.with_connection(|connection| {
            let run = connection
                .query_row(
                    "SELECT id, project_id, project_name, skill_id, provider, model, status,
                            project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at
                     FROM runs WHERE id = ?1",
                    [run_id.to_string()],
                    history_run_from_row,
                )
                .optional()?
                .ok_or(StorageError::RunNotFound(run_id))?;
            let events = query_json_rows::<RunEvent>(
                connection,
                "SELECT event_json FROM run_events WHERE run_id = ?1 ORDER BY sequence",
                run_id,
            )?;
            let annotations = query_json_rows::<Annotation>(
                connection,
                "SELECT annotation_json FROM annotations WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )?;
            let revisions = query_json_rows::<AnnotationRevision>(
                connection,
                "SELECT r.revision_json FROM annotation_revisions r
                 JOIN annotations a ON a.id = r.annotation_id
                 WHERE a.run_id = ?1 ORDER BY r.created_at",
                run_id,
            )?;
            let mut task_statement = connection.prepare(
                "SELECT image_id, task_id, status, reason, updated_at
                 FROM task_runs WHERE run_id = ?1 ORDER BY updated_at, task_id",
            )?;
            let task_runs = task_statement
                .query_map([run_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let status_text = row.get::<_, String>(2)?;
                    let status = serde_json::from_value(serde_json::Value::String(status_text))
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(HistoryTaskRun {
                        run_id,
                        image_id,
                        task_id: TaskId::from(row.get::<_, String>(1)?),
                        status,
                        reason: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let usage = query_json_rows::<UsageRecord>(
                connection,
                "SELECT usage_json FROM usage_records WHERE run_id = ?1 ORDER BY id",
                run_id,
            )?;
            let mut message_statement = connection.prepare(
                "SELECT image_id, task_id, message_json, created_at
                 FROM model_messages WHERE run_id = ?1 ORDER BY sequence",
            )?;
            let model_messages = message_statement
                .query_map([run_id.to_string()], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .map(|row| {
                    let (image_id, task_id, message, created_at) = row?;
                    Ok(HistoryModelMessage {
                        image_id: image_id.map(|value| value.parse()).transpose().map_err(
                            |error| StorageError::InvalidEnum(format!("invalid image id: {error}")),
                        )?,
                        task_id: task_id.map(TaskId::from),
                        message: serde_json::from_str(&message)?,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            let artifacts = query_json_rows::<VisionArtifact>(
                connection,
                "SELECT artifact_json FROM vision_artifacts WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )?;
            let mut statement = connection.prepare(
                "SELECT call_id, name, arguments_json, result_json, error, created_at
                 FROM tool_calls WHERE run_id = ?1 ORDER BY created_at",
            )?;
            let tool_calls = statement
                .query_map([run_id.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?
                .map(|row| {
                    let (call_id, name, arguments, result, error, created_at) = row?;
                    Ok(HistoryToolCall {
                        call_id: ToolCallId::new(call_id),
                        name,
                        arguments: sanitize_trace_value(&serde_json::from_str(&arguments)?),
                        result: result
                            .map(|value| serde_json::from_str(&value))
                            .transpose()?,
                        error,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            Ok(HistoryDocument {
                schema_version: annotagent_core::HISTORY_SCHEMA_VERSION,
                run,
                events,
                annotations,
                revisions,
                task_runs,
                usage,
                model_messages,
                artifacts,
                tool_calls,
            })
        })
    }

    pub fn export_history(&self, run_id: RunId, path: &Path) -> Result<(), StorageError> {
        let bytes = serde_json::to_vec_pretty(&self.history(run_id)?)?;
        std::fs::write(path, bytes)
            .map_err(|error| StorageError::Serialization(serde_json::Error::io(error)))
    }

    pub fn import_history(
        &self,
        mut document: HistoryDocument,
    ) -> Result<HistoryImportReport, StorageError> {
        if document.schema_version != annotagent_core::HISTORY_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedHistoryVersion(
                document.schema_version,
            ));
        }
        self.with_connection(|connection| {
            let original_run_id = document.run.id;
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1)",
                [original_run_id.to_string()],
                |row| row.get(0),
            )?;
            let target_run_id = if exists { RunId::new() } else { original_run_id };
            let ids_remapped = target_run_id != original_run_id;
            document.run.id = target_run_id;
            let mut annotation_ids = BTreeMap::new();
            let mut artifact_ids = BTreeMap::new();
            let mut tool_call_ids = BTreeMap::new();
            if ids_remapped {
                for annotation in &mut document.annotations {
                    let replacement = annotagent_core::AnnotationId::new();
                    annotation_ids.insert(annotation.id, replacement);
                    annotation.id = replacement;
                }
                for artifact in &mut document.artifacts {
                    let replacement = ArtifactId::new();
                    artifact_ids.insert(artifact.id, replacement);
                    artifact.id = replacement;
                }
                for call in &mut document.tool_calls {
                    let replacement = ToolCallId::new(format!("imported-{}", Uuid::new_v4()));
                    tool_call_ids.insert(call.call_id.clone(), replacement.clone());
                    call.call_id = replacement;
                }
                for annotation in &mut document.annotations {
                    remap_annotation_value(
                        &mut annotation.value,
                        &annotation_ids,
                        &artifact_ids,
                    );
                    for artifact_id in &mut annotation.provenance.artifact_ids {
                        if let Some(replacement) = artifact_ids.get(artifact_id) {
                            *artifact_id = *replacement;
                        }
                    }
                }
                for artifact in &mut document.artifacts {
                    if let Some(replaces) = artifact.replaces_artifact_id
                        && let Some(replacement) = artifact_ids.get(&replaces)
                    {
                        artifact.replaces_artifact_id = Some(*replacement);
                    }
                    for input in &mut artifact.provenance.input_artifact_ids {
                        if let Some(replacement) = artifact_ids.get(input) {
                            *input = *replacement;
                        }
                    }
                    if let VisionArtifactValue::Relations { relations } = &mut artifact.value {
                        remap_relations(relations, &annotation_ids, &artifact_ids);
                    }
                }
                let mut revision_ids = BTreeMap::new();
                for revision in &mut document.revisions {
                    let replacement = annotagent_core::AnnotationRevisionId::new();
                    revision_ids.insert(revision.revision_id, replacement);
                    revision.revision_id = replacement;
                    if let Some(annotation_id) = annotation_ids.get(&revision.annotation_id) {
                        revision.annotation_id = *annotation_id;
                    }
                }
                for revision in &mut document.revisions {
                    if let Some(parent) = revision.parent_revision_id
                        && let Some(replacement) = revision_ids.get(&parent)
                    {
                        revision.parent_revision_id = Some(*replacement);
                    }
                    if let Some(before) = &mut revision.before {
                        remap_annotation_value(
                            &mut before.value,
                            &annotation_ids,
                            &artifact_ids,
                        );
                    }
                    if let Some(after) = &mut revision.after {
                        remap_annotation_value(
                            &mut after.value,
                            &annotation_ids,
                            &artifact_ids,
                        );
                    }
                }
                let string_ids = annotation_ids
                    .iter()
                    .map(|(source, target)| (source.to_string(), target.to_string()))
                    .chain(
                        artifact_ids
                            .iter()
                            .map(|(source, target)| (source.to_string(), target.to_string())),
                    )
                    .chain(tool_call_ids.iter().map(|(source, target)| {
                        (source.as_str().to_owned(), target.as_str().to_owned())
                    }))
                    .collect::<BTreeMap<_, _>>();
                for entry in &mut document.model_messages {
                    if let Some(call_id) = &entry.message.tool_call_id
                        && let Some(replacement) = tool_call_ids.get(call_id)
                    {
                        entry.message.tool_call_id = Some(replacement.clone());
                    }
                    for call in &mut entry.message.tool_calls {
                        if let Some(replacement) = tool_call_ids.get(&call.id) {
                            call.id = replacement.clone();
                        }
                        remap_json_strings(&mut call.arguments, &string_ids);
                    }
                    if let Ok(mut value) = serde_json::from_str(&entry.message.content) {
                        remap_json_strings(&mut value, &string_ids);
                        entry.message.content = serde_json::to_string(&value)?;
                    }
                }
                for call in &mut document.tool_calls {
                    remap_json_strings(&mut call.arguments, &string_ids);
                    if let Some(result) = &mut call.result {
                        remap_json_strings(&mut result.persisted_result, &string_ids);
                        remap_json_strings(&mut result.model_result, &string_ids);
                        for artifact in &mut result.artifacts {
                            if let Some(replacement) = artifact_ids.get(&artifact.id) {
                                artifact.id = *replacement;
                            }
                            if let Some(replaces) = artifact.replaces_artifact_id
                                && let Some(replacement) = artifact_ids.get(&replaces)
                            {
                                artifact.replaces_artifact_id = Some(*replacement);
                            }
                            remap_copy_ids(
                                &mut artifact.provenance.input_artifact_ids,
                                &artifact_ids,
                            );
                            if let VisionArtifactValue::Relations { relations } =
                                &mut artifact.value
                            {
                                remap_relations(relations, &annotation_ids, &artifact_ids);
                            }
                        }
                    }
                }
            }
            for event in &mut document.events {
                event.run_id = target_run_id;
                if ids_remapped {
                    event.event_id = annotagent_core::EventId::new();
                    match &mut event.payload {
                        RunEventPayload::Tool { call_id, .. } => {
                            if let Some(replacement) = tool_call_ids.get(call_id) {
                                *call_id = replacement.clone();
                            }
                        }
                        RunEventPayload::Annotation { annotation_ids: ids, .. } => {
                            remap_copy_ids(ids, &annotation_ids);
                        }
                        RunEventPayload::Artifact { artifact_ids: ids, .. } => {
                            remap_copy_ids(ids, &artifact_ids);
                        }
                        _ => {}
                    }
                }
            }
            for task_run in &mut document.task_runs {
                task_run.run_id = target_run_id;
            }
            let transaction = connection.unchecked_transaction()?;
            insert_history_run(&transaction, &document.run)?;
            for task_run in &document.task_runs {
                transaction.execute(
                    "INSERT INTO task_runs
                     (run_id, image_id, task_id, status, reason, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        target_run_id.to_string(),
                        task_run.image_id.to_string(),
                        task_run.task_id.as_str(),
                        enum_string(task_run.status)?,
                        task_run.reason,
                        task_run.updated_at,
                    ],
                )?;
            }
            for event in &document.events {
                transaction.execute(
                    "INSERT INTO run_events
                     (event_id, run_id, event_kind, event_json, occurred_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        event.event_id.to_string(),
                        target_run_id.to_string(),
                        enum_string(event.kind)?,
                        serde_json::to_string(event)?,
                        event.occurred_at.to_rfc3339()
                    ],
                )?;
            }
            for annotation in &document.annotations {
                transaction.execute(
                    "INSERT INTO annotations
                     (id, run_id, image_id, task_id, label, review_status, annotation_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        annotation.id.to_string(),
                        target_run_id.to_string(),
                        annotation.image_id.to_string(),
                        annotation.task_id.as_str(),
                        annotation.label.as_ref().map(LabelId::as_str),
                        enum_string(annotation.review_status)?,
                        serde_json::to_string(annotation)?,
                        annotation.created_at.to_rfc3339()
                    ],
                )?;
            }
            for revision in &document.revisions {
                transaction.execute(
                    "INSERT INTO annotation_revisions
                     (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        revision.revision_id.to_string(),
                        revision.annotation_id.to_string(),
                        revision.parent_revision_id.map(|id| id.to_string()),
                        serde_json::to_string(revision)?,
                        revision.created_at.to_rfc3339()
                    ],
                )?;
            }
            for usage in &document.usage {
                transaction.execute(
                    "INSERT INTO usage_records
                     (run_id, usage_json, input_tokens, output_tokens, total_tokens, cost, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        target_run_id.to_string(),
                        serde_json::to_string(usage)?,
                        usage.tokens.input_tokens.map(sqlite_u64),
                        usage.tokens.output_tokens.map(sqlite_u64),
                        usage.tokens.total_tokens.map(sqlite_u64),
                        usage.cost.total.to_string(),
                        usage.completed_at.to_rfc3339()
                    ],
                )?;
            }
            for entry in &document.model_messages {
                transaction.execute(
                    "INSERT INTO model_messages
                     (run_id, image_id, task_id, message_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        target_run_id.to_string(),
                        entry.image_id.map(|id| id.to_string()),
                        entry.task_id.as_ref().map(TaskId::as_str),
                        serde_json::to_string(&entry.message)?,
                        entry.created_at
                    ],
                )?;
            }
            for artifact in &document.artifacts {
                transaction.execute(
                    "INSERT INTO vision_artifacts
                     (artifact_id, run_id, image_id, task_id, source_node, validation_state,
                      artifact_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        artifact.id.to_string(),
                        target_run_id.to_string(),
                        artifact.image_id.to_string(),
                        artifact.task_id.as_ref().map(TaskId::as_str),
                        artifact.source_node,
                        enum_string(artifact.validation_state)?,
                        serde_json::to_string(artifact)?,
                        artifact.created_at.to_rfc3339()
                    ],
                )?;
            }
            for call in &document.tool_calls {
                transaction.execute(
                    "INSERT INTO tool_calls
                     (call_id, run_id, name, arguments_json, result_json, error, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        call.call_id.as_str(),
                        target_run_id.to_string(),
                        call.name,
                        serde_json::to_string(&sanitize_trace_value(&call.arguments))?,
                        call.result.as_ref().map(serde_json::to_string).transpose()?,
                        call.error,
                        call.created_at
                    ],
                )?;
            }
            transaction.commit()?;
            let warnings = if document.annotations.is_empty() {
                Vec::new()
            } else {
                vec![
                    "history import preserves image IDs and hashes but does not restore missing image files"
                        .to_owned(),
                ]
            };
            Ok(HistoryImportReport {
                run_id: target_run_id,
                ids_remapped,
                warnings,
            })
        })
    }

    pub fn save_correction(&self, record: &CorrectionRecord) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO correction_records
                 (id, project_id, skill_id, task_id, predicted_label, corrected_label,
                  reason_code, record_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.id.to_string(),
                    record.project_id.to_string(),
                    record.skill_id,
                    record.task_id.as_str(),
                    record.predicted_label.as_ref().map(LabelId::as_str),
                    record.corrected_label.as_ref().map(LabelId::as_str),
                    record.reason_code,
                    serde_json::to_string(record)?,
                    record.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        operation(&connection)
    }
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn history_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRun> {
    let id = row.get::<_, String>(0)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let status_text = row.get::<_, String>(6)?;
    let status =
        serde_json::from_value(serde_json::Value::String(status_text)).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(HistoryRun {
        id,
        project_id: row
            .get::<_, Option<String>>(1)?
            .map(|value| value.parse())
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        project_name: row.get(2)?,
        skill_id: row.get(3)?,
        provider: row.get(4)?,
        model: row.get(5)?,
        status,
        project_schema_json: row.get(7)?,
        workflow_snapshot_json: row.get(8)?,
        terminal_reason: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn query_json_rows<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    run_id: RunId,
) -> Result<Vec<T>, StorageError> {
    let mut statement = connection.prepare(sql)?;
    statement
        .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect()
}

fn enum_string<T: serde::Serialize>(value: T) -> Result<String, StorageError> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StorageError::InvalidEnum("enum did not serialize as a string".to_owned()))
}

fn parse_run_id(value: &str) -> Result<RunId, StorageError> {
    value
        .parse()
        .map_err(|error| StorageError::InvalidEnum(format!("invalid run id: {error}")))
}

fn parse_run_status(value: &str) -> Result<RunStatus, StorageError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(StorageError::from)
}

fn insert_history_run(
    transaction: &rusqlite::Transaction<'_>,
    run: &HistoryRun,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO runs
         (id, project_id, project_name, skill_id, provider, model, status, project_schema_json,
          workflow_snapshot_json, terminal_reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            run.id.to_string(),
            run.project_id.map(|id| id.to_string()),
            run.project_name,
            run.skill_id,
            run.provider,
            run.model,
            enum_string(run.status)?,
            run.project_schema_json,
            run.workflow_snapshot_json,
            run.terminal_reason,
            run.created_at,
            run.updated_at,
        ],
    )?;
    Ok(())
}

fn remap_copy_ids<T: Copy + Ord>(ids: &mut [T], replacements: &BTreeMap<T, T>) {
    for id in ids {
        if let Some(replacement) = replacements.get(id) {
            *id = *replacement;
        }
    }
}

fn remap_relations(
    relations: &mut [annotagent_core::RelationValue],
    annotation_ids: &BTreeMap<annotagent_core::AnnotationId, annotagent_core::AnnotationId>,
    artifact_ids: &BTreeMap<ArtifactId, ArtifactId>,
) {
    for relation in relations {
        for endpoint in [&mut relation.source, &mut relation.target] {
            match endpoint {
                RelationEndpoint::Annotation(id) => {
                    if let Some(replacement) = annotation_ids.get(id) {
                        *id = *replacement;
                    }
                }
                RelationEndpoint::Artifact(id) => {
                    if let Some(replacement) = artifact_ids.get(id) {
                        *id = *replacement;
                    }
                }
            }
        }
    }
}

fn remap_annotation_value(
    value: &mut AnnotationValue,
    annotation_ids: &BTreeMap<annotagent_core::AnnotationId, annotagent_core::AnnotationId>,
    artifact_ids: &BTreeMap<ArtifactId, ArtifactId>,
) {
    match value {
        AnnotationValue::Relation { source, target, .. } => {
            if let Some(replacement) = annotation_ids.get(source) {
                *source = *replacement;
            }
            if let Some(replacement) = annotation_ids.get(target) {
                *target = *replacement;
            }
        }
        AnnotationValue::Relations { relations } => {
            remap_relations(relations, annotation_ids, artifact_ids);
        }
        _ => {}
    }
}

fn remap_json_strings(value: &mut serde_json::Value, replacements: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::String(string) => {
            if let Some(replacement) = replacements.get(string) {
                string.clone_from(replacement);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remap_json_strings(value, replacements);
            }
        }
        serde_json::Value::Object(object) => {
            for value in object.values_mut() {
                remap_json_strings(value, replacements);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn sanitize_trace_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("authorization")
                        || lower.contains("api_key")
                        || lower.contains("secret")
                    {
                        (
                            key.clone(),
                            serde_json::Value::String("[REDACTED]".to_owned()),
                        )
                    } else if lower.contains("base64") || lower == "data_url" {
                        (
                            key.clone(),
                            serde_json::Value::String("[BINARY OMITTED]".to_owned()),
                        )
                    } else {
                        (key.clone(), sanitize_trace_value(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(sanitize_trace_value).collect())
        }
        _ => value.clone(),
    }
}

#[async_trait]
impl RuntimeStore for SqliteStore {
    async fn create_run(&self, run: &RunRecord) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let status = serde_json::to_value(run.status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("pending")
            .to_owned();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO runs
                 (id, project_id, project_name, skill_id, provider, model, status, project_schema_json, workflow_snapshot_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    run.id.to_string(),
                    run.project_id.to_string(),
                    run.project_name,
                    run.skill_id,
                    run.provider,
                    run.model,
                    status,
                    run.project_schema_json,
                    run.workflow_snapshot_json,
                    now
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_run_status(
        &self,
        run_id: RunId,
        status: RunStatus,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let status = serde_json::to_value(status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("failed")
            .to_owned();
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "UPDATE runs SET status = ?2, terminal_reason = ?3, updated_at = ?4 WHERE id = ?1",
                params![run_id.to_string(), status, reason, Utc::now().to_rfc3339()],
            )?;
            if matches!(
                status.as_str(),
                "pending" | "running" | "paused" | "awaiting_review"
            ) {
                transaction.execute(
                    "UPDATE active_project_runs SET status = ?2, updated_at = ?3 WHERE run_id = ?1",
                    params![run_id.to_string(), status, Utc::now().to_rfc3339()],
                )?;
            } else {
                transaction.execute(
                    "DELETE FROM active_project_runs WHERE run_id = ?1",
                    [run_id.to_string()],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_task_run_status(
        &self,
        run_id: RunId,
        image_id: ImageId,
        task_id: &TaskId,
        status: TaskRunStatus,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let status = serde_json::to_value(status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("failed")
            .to_owned();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO task_runs (run_id, image_id, task_id, status, reason, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(run_id, image_id, task_id) DO UPDATE SET
                   status = excluded.status,
                   reason = excluded.reason,
                   updated_at = excluded.updated_at",
                params![
                    run_id.to_string(),
                    image_id.to_string(),
                    task_id.as_str(),
                    status,
                    reason,
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_event(&self, event: &RunEvent) -> Result<(), String> {
        let event_json = json(event)?;
        let kind = serde_json::to_value(event.kind)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO run_events (event_id, run_id, event_kind, event_json, occurred_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event.event_id.to_string(),
                    event.run_id.to_string(),
                    kind,
                    event_json,
                    event.occurred_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_usage(&self, run_id: RunId, usage: &UsageRecord) -> Result<(), String> {
        let usage_json = json(usage)?;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO model_calls
                 (run_id, provider, model, endpoint_summary, request_id, success, retry_count,
                  started_at, completed_at, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run_id.to_string(),
                    usage.provider,
                    usage.model,
                    usage.endpoint_summary,
                    usage.request_id,
                    usage.success,
                    usage.retry_count,
                    usage.started_at.to_rfc3339(),
                    usage.completed_at.to_rfc3339(),
                    sqlite_u64(usage.duration_ms)
                ],
            )?;
            transaction.execute(
                "INSERT INTO usage_records
                 (run_id, usage_json, input_tokens, output_tokens, total_tokens, cost, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run_id.to_string(),
                    usage_json,
                    usage.tokens.input_tokens.map(sqlite_u64),
                    usage.tokens.output_tokens.map(sqlite_u64),
                    usage.tokens.total_tokens.map(sqlite_u64),
                    usage.cost.total.to_string(),
                    usage.completed_at.to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_model_message(
        &self,
        run_id: RunId,
        image_id: Option<ImageId>,
        task_id: Option<&TaskId>,
        message: &ModelMessage,
    ) -> Result<(), String> {
        let message_json = json(message)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO model_messages
                 (run_id, image_id, task_id, message_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    run_id.to_string(),
                    image_id.map(|id| id.to_string()),
                    task_id.map(TaskId::as_str),
                    message_json,
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_tool_call(
        &self,
        run_id: RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: &serde_json::Value,
        result: Option<&ToolResult>,
        error: Option<&str>,
    ) -> Result<(), String> {
        let arguments = json(arguments)?;
        let result = result.map(json).transpose()?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR REPLACE INTO tool_calls
                 (call_id, run_id, name, arguments_json, result_json, error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    call_id.as_str(),
                    run_id.to_string(),
                    name,
                    arguments,
                    result,
                    error,
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_artifact(
        &self,
        run_id: RunId,
        artifact: &VisionArtifact,
    ) -> Result<(), String> {
        artifact.validate().map_err(|error| error.to_string())?;
        let state = serde_json::to_value(artifact.validation_state)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("unvalidated")
            .to_owned();
        let artifact_json = json(artifact)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR REPLACE INTO vision_artifacts
                 (artifact_id, run_id, image_id, task_id, source_node, validation_state,
                  artifact_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    artifact.id.to_string(),
                    run_id.to_string(),
                    artifact.image_id.to_string(),
                    artifact.task_id.as_ref().map(TaskId::as_str),
                    artifact.source_node,
                    state,
                    artifact_json,
                    artifact.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_artifact_validation_state(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
        state: ArtifactValidationState,
    ) -> Result<(), String> {
        self.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT artifact_json FROM vision_artifacts
                     WHERE artifact_id = ?1 AND run_id = ?2",
                    params![artifact_id.to_string(), run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("artifact {artifact_id} was not found"))
                })?;
            let mut artifact: VisionArtifact = serde_json::from_str(&stored)?;
            artifact.validation_state = state;
            connection.execute(
                "UPDATE vision_artifacts SET validation_state = ?2, artifact_json = ?3
                 WHERE artifact_id = ?1 AND run_id = ?4",
                params![
                    artifact_id.to_string(),
                    enum_string(state)?,
                    serde_json::to_string(&artifact)?,
                    run_id.to_string()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn find_artifact(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
    ) -> Result<Option<VisionArtifact>, String> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT artifact_json FROM vision_artifacts
                     WHERE artifact_id = ?1 AND run_id = ?2",
                    params![artifact_id.to_string(), run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
                .transpose()
        })
        .map_err(|error| error.to_string())
    }

    async fn record_validation(
        &self,
        run_id: RunId,
        issues: &[ValidationIssue],
    ) -> Result<(), String> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            for issue in issues {
                let severity = serde_json::to_value(issue.severity)?
                    .as_str()
                    .unwrap_or("warning")
                    .to_owned();
                transaction.execute(
                    "INSERT INTO validation_issues
                     (run_id, code, severity, issue_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        run_id.to_string(),
                        issue.code,
                        severity,
                        serde_json::to_string(issue)?,
                        Utc::now().to_rfc3339()
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn commit_annotation(
        &self,
        run_id: RunId,
        annotation: &Annotation,
    ) -> Result<(), String> {
        let annotation_json = json(annotation)?;
        let status = serde_json::to_value(annotation.review_status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("draft")
            .to_owned();
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO annotations
                 (id, run_id, image_id, task_id, label, review_status, annotation_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    annotation.id.to_string(),
                    run_id.to_string(),
                    annotation.image_id.to_string(),
                    annotation.task_id.as_str(),
                    annotation
                        .label
                        .as_ref()
                        .map(annotagent_core::LabelId::as_str),
                    status,
                    annotation_json,
                    annotation.created_at.to_rfc3339()
                ],
            )?;
            if annotation.review_status == annotagent_core::ReviewStatus::NeedsReview {
                transaction.execute(
                    "INSERT INTO review_queue
                     (run_id, annotation_id, status, reasons_json, created_at)
                     VALUES (?1, ?2, 'pending', '[]', ?3)",
                    params![
                        run_id.to_string(),
                        annotation.id.to_string(),
                        Utc::now().to_rfc3339()
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn correction_risk(
        &self,
        project_id: ProjectId,
        skill_id: &str,
        task_id: &TaskId,
        label: Option<&LabelId>,
    ) -> Result<f32, String> {
        self.with_connection(|connection| {
            let count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM (
                    SELECT id FROM correction_records
                    WHERE project_id = ?1 AND skill_id = ?2 AND task_id = ?3
                      AND (?4 IS NULL OR predicted_label = ?4)
                    ORDER BY created_at DESC LIMIT 20
                 )",
                params![
                    project_id.to_string(),
                    skill_id,
                    task_id.as_str(),
                    label.map(LabelId::as_str)
                ],
                |row| row.get(0),
            )?;
            let bounded = u16::try_from(count.clamp(0, 20)).unwrap_or(20);
            Ok(f32::from(bounded) / 20.0)
        })
        .map_err(|error| error.to_string())
    }

    async fn record_revision(&self, revision: &AnnotationRevision) -> Result<(), String> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO annotation_revisions
                 (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    revision.revision_id.to_string(),
                    revision.annotation_id.to_string(),
                    revision.parent_revision_id.map(|id| id.to_string()),
                    serde_json::to_string(revision)?,
                    revision.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }
}

fn sqlite_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use annotagent_core::{
        ArtifactProvenance, ArtifactRole, AttributeValue, RunEventKind, RunEventPayload,
        VisionArtifactValue, WorkflowDraftNode, WorkflowNodeKind,
    };

    use super::*;

    #[test]
    fn migration_creates_required_tables() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let tables = store.schema_tables().expect("schema tables");
        for required in [
            "schema_migrations",
            "projects",
            "project_snapshots",
            "images",
            "tasks",
            "annotations",
            "annotation_revisions",
            "runs",
            "active_project_runs",
            "run_start_requests",
            "run_images",
            "task_runs",
            "run_steps",
            "run_events",
            "model_calls",
            "model_messages",
            "tool_calls",
            "vision_artifacts",
            "validation_issues",
            "usage_records",
            "correction_records",
            "review_queue",
            "settings_metadata",
            "workflow_drafts",
            "workflow_versions",
            "dataset_batches",
            "batch_images",
            "batch_events",
        ] {
            assert!(
                tables.iter().any(|table| table == required),
                "missing {required}"
            );
        }
    }

    #[test]
    fn project_persists_multiple_workflow_drafts_and_published_versions() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let now = Utc::now();
        let make_draft = |id: &str| WorkflowDraft {
            schema_version: 2,
            id: id.to_owned(),
            project_id: "multi-workflow-project".to_owned(),
            name: id.to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![WorkflowDraftNode {
                id: "commit".to_owned(),
                node_type: "commit".to_owned(),
                kind: WorkflowNodeKind::Commit,
                ..WorkflowDraftNode::default()
            }],
            edges: Vec::new(),
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            allow_unvalidated_commit: true,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        for id in ["workflow-a", "workflow-b"] {
            let draft = make_draft(id);
            store.save_workflow_draft(&draft).expect("draft");
            let snapshot = WorkflowSnapshot {
                schema_version: 2,
                draft: Some(draft.clone()),
                ..WorkflowSnapshot::default()
            };
            store
                .publish_workflow_draft(&draft, format!("hash-{id}"), snapshot)
                .expect("published version");
        }
        assert_eq!(
            store
                .list_workflow_drafts(Some("multi-workflow-project"))
                .expect("drafts")
                .len(),
            2
        );
        assert_eq!(
            store
                .list_published_workflow_versions(Some("multi-workflow-project"))
                .expect("versions")
                .len(),
            2
        );
    }

    #[test]
    fn legacy_terminal_awaiting_review_is_migrated_once() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE runs (
                    id TEXT PRIMARY KEY,
                    project_id TEXT,
                    project_name TEXT NOT NULL,
                    skill_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    status TEXT NOT NULL,
                    project_schema_json TEXT NOT NULL,
                    terminal_reason TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE active_project_runs (
                    project_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL UNIQUE,
                    status TEXT NOT NULL,
                    idempotency_key TEXT,
                    updated_at TEXT NOT NULL
                );
                INSERT INTO runs VALUES (
                    'legacy', NULL, 'legacy', 'legacy', 'mock', 'mock',
                    'awaiting_review', '{}', NULL, '2026-01-01T00:00:00Z',
                    '2026-01-01T00:00:00Z'
                );
                INSERT INTO active_project_runs VALUES (
                    'legacy-project', 'legacy', 'awaiting_review', NULL,
                    '2026-01-01T00:00:00Z'
                );",
            )
            .expect("legacy schema");
        let store = SqliteStore {
            connection: Mutex::new(connection),
        };
        store.migrate().expect("upgrade legacy schema");
        let migrated = store
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT status FROM runs WHERE id = 'legacy'",
                    [],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .expect("migrated status");
        assert_eq!(migrated, "completed_with_review");
        let workflow_snapshot_column = store
            .with_connection(|connection| {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                let columns = statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(columns
                    .into_iter()
                    .any(|name| name == "workflow_snapshot_json"))
            })
            .expect("run columns");
        assert!(workflow_snapshot_column);
        let active_count =
            store
                .with_connection(|connection| {
                    Ok(connection.query_row(
                        "SELECT COUNT(*) FROM active_project_runs",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?)
                })
                .expect("active rows");
        assert_eq!(active_count, 0);

        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO runs
                     (id, project_id, project_name, skill_id, provider, model, status,
                      project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at)
                     VALUES (
                        'suspended', NULL, 'new', 'generic', 'mock', 'mock',
                        'awaiting_review', '{}', NULL, NULL, '2026-01-02T00:00:00Z',
                        '2026-01-02T00:00:00Z'
                    )",
                    [],
                )?;
                Ok(())
            })
            .expect("new suspended run");
        store.migrate().expect("idempotent migration");
        let suspended = store
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT status FROM runs WHERE id = 'suspended'",
                    [],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .expect("suspended status");
        assert_eq!(suspended, "awaiting_review");
    }

    #[tokio::test]
    async fn event_history_round_trips() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let run_id = RunId::new();
        let frozen_workflow = r#"{"workflow_id":"workflow-a","version":1,"name":"before edit"}"#;
        store
            .create_run(&RunRecord {
                id: run_id,
                project_id: ProjectId::new(),
                project_name: "test".to_owned(),
                skill_id: "dummy".to_owned(),
                provider: "mock".to_owned(),
                model: "mock".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: "{}".to_owned(),
                workflow_snapshot_json: Some(frozen_workflow.to_owned()),
            })
            .await
            .expect("create run");
        let event = RunEvent::new(
            run_id,
            RunEventKind::RunCreated,
            RunEventPayload::State {
                from: None,
                to: RunStatus::Pending,
                reason: None,
            },
        );
        store.record_event(&event).await.expect("record event");
        assert_eq!(store.list_events(run_id).expect("events"), vec![event]);
        let history = store.history(run_id).expect("history");
        assert_eq!(
            history.run.workflow_snapshot_json.as_deref(),
            Some(frozen_workflow)
        );

        let artifact = VisionArtifact {
            id: ArtifactId::new(),
            image_id: ImageId::new(),
            task_id: Some(TaskId::from("attributes")),
            label: None,
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::Attributes {
                values: BTreeMap::from([("verified".to_owned(), AttributeValue::Boolean(true))]),
            },
            source_node: "generic.attributes".to_owned(),
            confidence: Some(0.9),
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance::default(),
            revision: 1,
            replaces_artifact_id: None,
            created_at: Utc::now(),
        };
        store
            .record_artifact(run_id, &artifact)
            .await
            .expect("record artifact");
        assert_eq!(
            store.list_artifacts(run_id).expect("artifacts"),
            vec![artifact]
        );
    }

    #[test]
    fn project_run_reservation_is_transactional_and_idempotent() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        let run_id = RunId::new();
        assert_eq!(
            store
                .reserve_project_run(project_id, run_id, Some("request-1"))
                .expect("reservation"),
            RunStartReservation::Reserved
        );
        assert_eq!(
            store
                .reserve_project_run(project_id, RunId::new(), Some("request-1"))
                .expect("idempotent replay"),
            RunStartReservation::Idempotent {
                run_id,
                status: RunStatus::Pending,
            }
        );
        assert_eq!(
            store
                .reserve_project_run(project_id, RunId::new(), Some("request-2"))
                .expect("conflict"),
            RunStartReservation::Conflict {
                run_id,
                status: RunStatus::Pending,
            }
        );
    }
}
