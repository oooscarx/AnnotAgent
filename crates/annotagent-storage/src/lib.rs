//! `SQLite` persistence for projects, auditable runs, revisions, and correction memory.

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use annotagent_core::{
    Annotation, AnnotationRevision, AnnotationValue, CorrectionRecord, LabelId, ProjectId,
    RunEvent, RunId, RunStatus, TaskId, ToolCallId, ToolResult, UsageRecord, ValidationIssue,
};
use annotagent_runtime::{RunRecord, RuntimeStore};
use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/0001_initial.sql");

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
    #[error("unsupported history schema version {0}")]
    UnsupportedHistoryVersion(u32),
    #[error("invalid stored enum value: {0}")]
    InvalidEnum(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryRun {
    pub id: RunId,
    pub project_name: String,
    pub skill_id: String,
    pub provider: String,
    pub model: String,
    pub status: RunStatus,
    pub project_schema_json: String,
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
pub struct HistoryDocument {
    pub schema_version: u32,
    pub run: HistoryRun,
    pub events: Vec<RunEvent>,
    pub annotations: Vec<Annotation>,
    pub revisions: Vec<AnnotationRevision>,
    pub usage: Vec<UsageRecord>,
    pub tool_calls: Vec<HistoryToolCall>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryImportReport {
    pub run_id: RunId,
    pub ids_remapped: bool,
    pub warnings: Vec<String>,
}

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
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
                "SELECT id, project_name, skill_id, provider, model, status,
                        project_schema_json, terminal_reason, created_at, updated_at
                 FROM runs ORDER BY created_at DESC",
            )?;
            statement
                .query_map([], history_run_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn history(&self, run_id: RunId) -> Result<HistoryDocument, StorageError> {
        self.with_connection(|connection| {
            let run = connection
                .query_row(
                    "SELECT id, project_name, skill_id, provider, model, status,
                            project_schema_json, terminal_reason, created_at, updated_at
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
            let usage = query_json_rows::<UsageRecord>(
                connection,
                "SELECT usage_json FROM usage_records WHERE run_id = ?1 ORDER BY id",
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
                usage,
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
            if ids_remapped {
                for annotation in &mut document.annotations {
                    let replacement = annotagent_core::AnnotationId::new();
                    annotation_ids.insert(annotation.id, replacement);
                    annotation.id = replacement;
                }
                for annotation in &mut document.annotations {
                    if let AnnotationValue::Relation { source, target, .. } = &mut annotation.value {
                        if let Some(replacement) = annotation_ids.get(source) {
                            *source = *replacement;
                        }
                        if let Some(replacement) = annotation_ids.get(target) {
                            *target = *replacement;
                        }
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
                }
            }
            for event in &mut document.events {
                event.run_id = target_run_id;
                if ids_remapped {
                    event.event_id = annotagent_core::EventId::new();
                }
            }
            let transaction = connection.unchecked_transaction()?;
            insert_history_run(&transaction, &document.run)?;
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
            for call in &document.tool_calls {
                let call_id = if ids_remapped {
                    ToolCallId::new(format!("imported-{}", Uuid::new_v4()))
                } else {
                    call.call_id.clone()
                };
                transaction.execute(
                    "INSERT INTO tool_calls
                     (call_id, run_id, name, arguments_json, result_json, error, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        call_id.as_str(),
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

    fn with_connection<T>(
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
    let status_text = row.get::<_, String>(5)?;
    let status =
        serde_json::from_value(serde_json::Value::String(status_text)).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(HistoryRun {
        id,
        project_name: row.get(1)?,
        skill_id: row.get(2)?,
        provider: row.get(3)?,
        model: row.get(4)?,
        status,
        project_schema_json: row.get(6)?,
        terminal_reason: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
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

fn insert_history_run(
    transaction: &rusqlite::Transaction<'_>,
    run: &HistoryRun,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO runs
         (id, project_name, skill_id, provider, model, status, project_schema_json,
          terminal_reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            run.id.to_string(),
            run.project_name,
            run.skill_id,
            run.provider,
            run.model,
            enum_string(run.status)?,
            run.project_schema_json,
            run.terminal_reason,
            run.created_at,
            run.updated_at,
        ],
    )?;
    Ok(())
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
                 (id, project_name, skill_id, provider, model, status, project_schema_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    run.id.to_string(),
                    run.project_name,
                    run.skill_id,
                    run.provider,
                    run.model,
                    status,
                    run.project_schema_json,
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
            connection.execute(
                "UPDATE runs SET status = ?2, terminal_reason = ?3, updated_at = ?4 WHERE id = ?1",
                params![run_id.to_string(), status, reason, Utc::now().to_rfc3339()],
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
    use annotagent_core::{RunEventKind, RunEventPayload};

    use super::*;

    #[test]
    fn migration_creates_required_tables() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let tables = store.schema_tables().expect("schema tables");
        for required in [
            "projects",
            "project_snapshots",
            "images",
            "tasks",
            "annotations",
            "annotation_revisions",
            "runs",
            "run_images",
            "run_steps",
            "run_events",
            "model_calls",
            "tool_calls",
            "validation_issues",
            "usage_records",
            "correction_records",
            "review_queue",
            "settings_metadata",
        ] {
            assert!(
                tables.iter().any(|table| table == required),
                "missing {required}"
            );
        }
    }

    #[tokio::test]
    async fn event_history_round_trips() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let run_id = RunId::new();
        store
            .create_run(&RunRecord {
                id: run_id,
                project_name: "test".to_owned(),
                skill_id: "dummy".to_owned(),
                provider: "mock".to_owned(),
                model: "mock".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: "{}".to_owned(),
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
    }
}
