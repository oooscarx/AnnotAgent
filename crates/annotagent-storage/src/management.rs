use std::{cmp::Reverse, collections::BTreeSet};

use annotagent_core::{
    BatchStatus, LifecycleUsageTotals, ManagementAction, ManagementBlocker, ManagementImpact,
    ManagementObjectKind, ManagementObjectRef, ManagementOperationStatus, ManagementPreview,
    ManagementReceipt, ManagementRequest, ManagementUsageSummary, PipelineLifecycleSummary,
    ProjectId, PurgeReport, RunProvenanceSummary, RunStatus, TrashEntry, WorkflowLifecycleItem,
    WorkflowVersionRef,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use uuid::Uuid;

use crate::{SqliteStore, StorageError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagementScope {
    pub project_id: String,
    pub stable_project_id: ProjectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchLifecycleMetadata {
    pub lifecycle_revision: u64,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub deletion_operation_id: Option<String>,
    pub deleted_child_runs: usize,
}

#[derive(Debug)]
struct RunLifecycle {
    id: String,
    project_id: Option<String>,
    status: RunStatus,
    revision: u64,
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

#[derive(Debug)]
struct BatchLifecycle {
    id: String,
    project_id: String,
    status: BatchStatus,
    revision: u64,
    lease_owner: Option<String>,
    lease_expires_at: Option<String>,
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

#[derive(Debug)]
struct DraftLifecycle {
    id: String,
    project_id: String,
    name: String,
    authoring_revision: u64,
    lifecycle_revision: u64,
    content_hash: String,
    archived_at: Option<String>,
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

#[derive(Debug)]
struct VersionLifecycle {
    workflow_id: String,
    version: u32,
    project_id: String,
    name: String,
    lifecycle_revision: u64,
    content_hash: String,
    archived_at: Option<String>,
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

#[derive(Debug)]
struct PipelineLifecycle {
    workflow_id: String,
    project_id: String,
    name: String,
    lifecycle_revision: u64,
    archived_at: Option<String>,
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

impl SqliteStore {
    pub fn acquire_management_lease(
        &self,
        scope: &ManagementScope,
        object: &ManagementObjectRef,
        lease_kind: &str,
        owner: &str,
        ttl: chrono::Duration,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let now = Utc::now();
            transaction.execute(
                "DELETE FROM management_entity_leases WHERE expires_at <= ?1",
                [now.to_rfc3339()],
            )?;
            let existing = transaction.query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM management_entity_leases
                   WHERE project_id = ?1 AND object_kind = ?2 AND object_id = ?3
                     AND object_version = ?4
                 )",
                params![
                    scope.project_id,
                    enum_json(object.kind)?,
                    object.id,
                    object.version.unwrap_or(0),
                ],
                |row| row.get::<_, bool>(0),
            )?;
            if existing {
                return Err(management_error(
                    "operation_in_progress",
                    "another operation already holds this lifecycle lease",
                ));
            }
            transaction.execute(
                "INSERT INTO management_entity_leases
                 (project_id, object_kind, object_id, object_version, lease_kind, owner,
                  expires_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    scope.project_id,
                    enum_json(object.kind)?,
                    object.id,
                    object.version.unwrap_or(0),
                    lease_kind,
                    owner,
                    (now + ttl).to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn release_management_lease(
        &self,
        scope: &ManagementScope,
        object: &ManagementObjectRef,
        lease_kind: &str,
        owner: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "DELETE FROM management_entity_leases
                 WHERE project_id = ?1 AND object_kind = ?2 AND object_id = ?3
                   AND object_version = ?4 AND lease_kind = ?5 AND owner = ?6",
                params![
                    scope.project_id,
                    enum_json(object.kind)?,
                    object.id,
                    object.version.unwrap_or(0),
                    lease_kind,
                    owner,
                ],
            )?;
            Ok(())
        })
    }

    pub fn project_workflow_default(
        &self,
        project_id: &str,
    ) -> Result<Option<WorkflowVersionRef>, StorageError> {
        self.with_connection(|connection| read_default(connection, project_id))
    }

    pub fn ensure_published_workflow_available(
        &self,
        project_id: &str,
        workflow_id: &str,
        version: u32,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let lifecycle = read_version(connection, workflow_id, version)?;
            let pipeline = read_pipeline(connection, workflow_id)?;
            if lifecycle.project_id != project_id || pipeline.project_id != project_id {
                return Err(management_error(
                    "foreign_project_object",
                    "Published Version does not belong to this Project",
                ));
            }
            if lifecycle.deleted_at.is_some() || pipeline.deleted_at.is_some() {
                return Err(management_error(
                    "entity_in_trash",
                    "Published Version or its Pipeline is in Trash",
                ));
            }
            if lifecycle.archived_at.is_some() || pipeline.archived_at.is_some() {
                return Err(management_error(
                    "entity_archived",
                    "Published Version or its Pipeline is archived",
                ));
            }
            Ok(())
        })
    }

    pub fn list_project_pipeline_lifecycle(
        &self,
        scope: &ManagementScope,
        include_archived: bool,
        include_deleted: bool,
    ) -> Result<Vec<PipelineLifecycleSummary>, StorageError> {
        self.with_connection(|connection| {
            let mut sql =
                "SELECT workflow_id FROM workflow_pipelines WHERE project_id = ?1".to_owned();
            if !include_archived {
                sql.push_str(" AND archived_at IS NULL");
            }
            if !include_deleted {
                sql.push_str(" AND deleted_at IS NULL");
            }
            sql.push_str(" ORDER BY updated_at DESC, workflow_id");
            let pipeline_ids = connection
                .prepare(&sql)?
                .query_map([&scope.project_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            pipeline_ids
                .into_iter()
                .map(|id| {
                    pipeline_summary(connection, scope, &id, include_archived, include_deleted)
                })
                .collect()
        })
    }

    pub fn preview_management(
        &self,
        scope: &ManagementScope,
        request: &ManagementRequest,
    ) -> Result<ManagementPreview, StorageError> {
        request
            .validate()
            .map_err(|message| management_error("invalid_management_request", message))?;
        if request.project_id != scope.project_id {
            return Err(management_error(
                "foreign_project_object",
                "request Project does not match the resolved Project scope",
            ));
        }
        self.with_connection(|connection| preview(connection, scope, request))
    }

    pub fn execute_management(
        &self,
        scope: &ManagementScope,
        request: &ManagementRequest,
    ) -> Result<ManagementReceipt, StorageError> {
        request
            .validate()
            .map_err(|message| management_error("invalid_management_request", message))?;
        if request.project_id != scope.project_id {
            return Err(management_error(
                "foreign_project_object",
                "request Project does not match the resolved Project scope",
            ));
        }
        self.with_connection(|connection| {
            if let Some((stored_request, result)) = connection
                .query_row(
                    "SELECT request_json, result_json FROM management_operations
                     WHERE project_id = ?1 AND idempotency_key = ?2",
                    params![scope.project_id, request.idempotency_key],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
            {
                let incoming = canonical_request_json(request)?;
                if stored_request != incoming {
                    return Err(management_error(
                        "idempotency_conflict",
                        "this idempotency key was already used for a different management request",
                    ));
                }
                let Some(result) = result else {
                    return Err(management_error(
                        "operation_in_progress",
                        "the matching management operation is still in progress",
                    ));
                };
                return serde_json::from_str(&result).map_err(StorageError::from);
            }

            let confirmed = request.confirmation_token.as_deref().ok_or_else(|| {
                management_error(
                    "confirmation_required",
                    "execute the operation with the confirmation token returned by preview",
                )
            })?;
            let current_preview = preview(connection, scope, request)?;
            if confirmed != current_preview.confirmation_token {
                return Err(management_error(
                    "revision_conflict",
                    "the management preview is stale; preview the current state and confirm again",
                ));
            }
            if !current_preview.can_execute {
                let blocker = current_preview.blockers.first().ok_or_else(|| {
                    management_error("management_blocked", "the operation is blocked")
                })?;
                return Err(management_error(&blocker.code, &blocker.message));
            }

            let transaction = connection.unchecked_transaction()?;
            let now = Utc::now();
            let operation_id = Uuid::new_v4().to_string();
            transaction.execute(
                "INSERT INTO management_operations
                 (id, project_id, idempotency_key, action, status, request_json, preview_json,
                  confirmation_token, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7, ?8, ?8)",
                params![
                    operation_id,
                    scope.project_id,
                    request.idempotency_key,
                    enum_json(request.action)?,
                    canonical_request_json(request)?,
                    serde_json::to_string(&current_preview)?,
                    current_preview.confirmation_token,
                    now.to_rfc3339(),
                ],
            )?;

            apply_default_transition(&transaction, scope, request, &current_preview.objects, &now)?;
            let (affected, purge) = match request.action {
                ManagementAction::MoveToTrash => (
                    move_to_trash(
                        &transaction,
                        scope,
                        &current_preview.objects,
                        &operation_id,
                        now,
                    )?,
                    None,
                ),
                ManagementAction::Restore => (
                    restore(
                        &transaction,
                        scope,
                        &current_preview.objects,
                        &operation_id,
                        now,
                    )?,
                    None,
                ),
                ManagementAction::Archive | ManagementAction::Unarchive => (
                    change_archive_state(
                        &transaction,
                        scope,
                        &current_preview.objects,
                        &operation_id,
                        now,
                        request.action == ManagementAction::Archive,
                    )?,
                    None,
                ),
                ManagementAction::Rename => (
                    rename_pipeline(
                        &transaction,
                        scope,
                        &current_preview.objects[0],
                        &operation_id,
                        now,
                        request
                            .display_name
                            .as_deref()
                            .expect("validated display name"),
                    )?,
                    None,
                ),
                ManagementAction::SetDefault | ManagementAction::ClearDefault => (
                    record_default_action(
                        &transaction,
                        scope,
                        &current_preview.objects,
                        &operation_id,
                    )?,
                    None,
                ),
                ManagementAction::Purge => purge_objects(
                    &transaction,
                    scope,
                    &current_preview.objects,
                    &operation_id,
                    now,
                )?,
                ManagementAction::CancelAndDelete => {
                    return Err(management_error(
                        "unsupported_management_action",
                        "this lifecycle action is not implemented for Run or Dataset Run objects",
                    ));
                }
            };
            let receipt = ManagementReceipt {
                operation_id: operation_id.clone(),
                project_id: scope.project_id.clone(),
                action: request.action,
                status: ManagementOperationStatus::Completed,
                affected_objects: affected,
                impact: current_preview.impact.clone(),
                purge,
                error: None,
                created_at: now,
                updated_at: now,
            };
            transaction.execute(
                "UPDATE management_operations
                 SET status = 'completed', result_json = ?2, updated_at = ?3, completed_at = ?3
                 WHERE id = ?1",
                params![
                    operation_id,
                    serde_json::to_string(&receipt)?,
                    now.to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(receipt)
        })
    }

    pub fn prepare_cancel_and_delete(
        &self,
        scope: &ManagementScope,
        request: &ManagementRequest,
    ) -> Result<ManagementReceipt, StorageError> {
        request
            .validate()
            .map_err(|message| management_error("invalid_management_request", message))?;
        if request.action != ManagementAction::CancelAndDelete {
            return Err(management_error(
                "invalid_management_request",
                "prepare_cancel_and_delete requires the cancel_and_delete action",
            ));
        }
        self.with_connection(|connection| {
            if let Some((stored_request, result)) = connection
                .query_row(
                    "SELECT request_json, result_json FROM management_operations
                     WHERE project_id = ?1 AND idempotency_key = ?2",
                    params![scope.project_id, request.idempotency_key],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
            {
                if stored_request != canonical_request_json(request)? {
                    return Err(management_error(
                        "idempotency_conflict",
                        "this idempotency key was already used for a different management request",
                    ));
                }
                return result
                    .ok_or_else(|| {
                        management_error(
                            "operation_in_progress",
                            "the cancellation operation has not persisted a status",
                        )
                    })
                    .and_then(|value| serde_json::from_str(&value).map_err(StorageError::from));
            }
            let confirmed = request.confirmation_token.as_deref().ok_or_else(|| {
                management_error(
                    "confirmation_required",
                    "execute the operation with the confirmation token returned by preview",
                )
            })?;
            let current_preview = preview(connection, scope, request)?;
            if confirmed != current_preview.confirmation_token {
                return Err(management_error(
                    "revision_conflict",
                    "the management preview is stale; preview the current state and confirm again",
                ));
            }
            if !current_preview.can_execute {
                let blocker = current_preview
                    .blockers
                    .first()
                    .ok_or_else(|| management_error("management_blocked", "operation blocked"))?;
                return Err(management_error(&blocker.code, &blocker.message));
            }
            let now = Utc::now();
            let operation_id = Uuid::new_v4().to_string();
            let receipt = ManagementReceipt {
                operation_id: operation_id.clone(),
                project_id: scope.project_id.clone(),
                action: ManagementAction::CancelAndDelete,
                status: ManagementOperationStatus::WaitingForCancellation,
                affected_objects: current_preview.objects.clone(),
                impact: current_preview.impact.clone(),
                purge: None,
                error: None,
                created_at: now,
                updated_at: now,
            };
            connection.execute(
                "INSERT INTO management_operations
                 (id, project_id, idempotency_key, action, status, request_json, preview_json,
                  result_json, confirmation_token, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'waiting_for_cancellation', ?5, ?6, ?7, ?8, ?9, ?9)",
                params![
                    operation_id,
                    scope.project_id,
                    request.idempotency_key,
                    enum_json(request.action)?,
                    canonical_request_json(request)?,
                    serde_json::to_string(&current_preview)?,
                    serde_json::to_string(&receipt)?,
                    current_preview.confirmation_token,
                    now.to_rfc3339(),
                ],
            )?;
            Ok(receipt)
        })
    }

    pub fn complete_cancel_and_delete(
        &self,
        scope: &ManagementScope,
        request: &ManagementRequest,
    ) -> Result<ManagementReceipt, StorageError> {
        self.with_connection(|connection| {
            let (operation_id, stored_request, created_at) = connection
                .query_row(
                    "SELECT id, request_json, created_at FROM management_operations
                     WHERE project_id = ?1 AND idempotency_key = ?2",
                    params![scope.project_id, request.idempotency_key],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| {
                    management_error(
                        "operation_not_found",
                        "cancellation operation was not prepared",
                    )
                })?;
            if stored_request != canonical_request_json(request)? {
                return Err(management_error(
                    "idempotency_conflict",
                    "the cancellation operation request changed",
                ));
            }
            let mut deletion_request = request.clone();
            deletion_request.action = ManagementAction::MoveToTrash;
            deletion_request.confirmation_token = None;
            let deletion_preview = preview(connection, scope, &deletion_request)?;
            if !deletion_preview.can_execute {
                return Err(management_error(
                    "cancellation_not_completed",
                    deletion_preview
                        .blockers
                        .iter()
                        .map(|blocker| blocker.message.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                ));
            }
            let transaction = connection.unchecked_transaction()?;
            let now = Utc::now();
            let affected = move_to_trash(
                &transaction,
                scope,
                &deletion_preview.objects,
                &operation_id,
                now,
            )?;
            let receipt = ManagementReceipt {
                operation_id: operation_id.clone(),
                project_id: scope.project_id.clone(),
                action: ManagementAction::CancelAndDelete,
                status: ManagementOperationStatus::Completed,
                affected_objects: affected,
                impact: deletion_preview.impact,
                purge: None,
                error: None,
                created_at: parse_datetime(&created_at)?,
                updated_at: now,
            };
            transaction.execute(
                "UPDATE management_operations
                 SET status = 'completed', result_json = ?2, updated_at = ?3, completed_at = ?3,
                     error = NULL WHERE id = ?1",
                params![
                    operation_id,
                    serde_json::to_string(&receipt)?,
                    now.to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(receipt)
        })
    }

    pub fn fail_cancel_and_delete(
        &self,
        scope: &ManagementScope,
        request: &ManagementRequest,
        message: &str,
    ) -> Result<ManagementReceipt, StorageError> {
        self.with_connection(|connection| {
            let result = connection.query_row(
                "SELECT result_json FROM management_operations
                 WHERE project_id = ?1 AND idempotency_key = ?2",
                params![scope.project_id, request.idempotency_key],
                |row| row.get::<_, String>(0),
            )?;
            let mut receipt: ManagementReceipt = serde_json::from_str(&result)?;
            receipt.status = ManagementOperationStatus::Failed;
            receipt.error = Some(message.to_owned());
            receipt.updated_at = Utc::now();
            connection.execute(
                "UPDATE management_operations SET status = 'failed', result_json = ?2,
                   error = ?3, updated_at = ?4, completed_at = ?4 WHERE id = ?1",
                params![
                    receipt.operation_id,
                    serde_json::to_string(&receipt)?,
                    message,
                    receipt.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(receipt)
        })
    }

    pub fn list_project_trash(
        &self,
        scope: &ManagementScope,
        kind: Option<ManagementObjectKind>,
    ) -> Result<Vec<TrashEntry>, StorageError> {
        self.with_connection(|connection| {
            let mut entries = Vec::new();
            if kind.is_none() || kind == Some(ManagementObjectKind::Run) {
                let mut statement = connection.prepare(
                    "SELECT id, project_name, lifecycle_revision, deleted_at,
                            deletion_operation_id
                     FROM runs
                     WHERE project_id = ?1 AND deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC, id",
                )?;
                let rows = statement.query_map([scope.stable_project_id.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?;
                for row in rows {
                    let (id, project_name, revision, deleted_at, operation_id) = row?;
                    entries.push(TrashEntry {
                        project_id: scope.project_id.clone(),
                        object: ManagementObjectRef {
                            kind: ManagementObjectKind::Run,
                            id: id.clone(),
                            version: None,
                            expected_revision: to_u64(revision),
                        },
                        display_name: format!("Run {}", short_id(&id)),
                        deleted_at: parse_datetime(&deleted_at)?,
                        deletion_operation_id: operation_id,
                        source_project: project_name,
                        recoverable: true,
                    });
                }
            }
            if kind.is_none() || kind == Some(ManagementObjectKind::Batch) {
                let mut statement = connection.prepare(
                    "SELECT id, lifecycle_revision, deleted_at, deletion_operation_id
                     FROM dataset_batches
                     WHERE project_id = ?1 AND deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC, id",
                )?;
                let rows = statement.query_map([&scope.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                for row in rows {
                    let (id, revision, deleted_at, operation_id) = row?;
                    entries.push(TrashEntry {
                        project_id: scope.project_id.clone(),
                        object: ManagementObjectRef {
                            kind: ManagementObjectKind::Batch,
                            id: id.clone(),
                            version: None,
                            expected_revision: to_u64(revision),
                        },
                        display_name: format!("Dataset Run {}", short_id(&id)),
                        deleted_at: parse_datetime(&deleted_at)?,
                        deletion_operation_id: operation_id,
                        source_project: scope.project_id.clone(),
                        recoverable: true,
                    });
                }
            }
            if kind.is_none() || kind == Some(ManagementObjectKind::WorkflowDraft) {
                let mut statement = connection.prepare(
                    "SELECT id, COALESCE(json_extract(draft_json, '$.name'), id),
                            lifecycle_revision, deleted_at, deletion_operation_id
                     FROM workflow_drafts
                     WHERE project_id = ?1 AND deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC, id",
                )?;
                let rows = statement.query_map([&scope.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?;
                for row in rows {
                    let (id, name, revision, deleted_at, operation_id) = row?;
                    entries.push(trash_entry(
                        scope,
                        ManagementObjectRef {
                            kind: ManagementObjectKind::WorkflowDraft,
                            id,
                            version: None,
                            expected_revision: to_u64(revision),
                        },
                        name,
                        &deleted_at,
                        operation_id,
                    )?);
                }
            }
            if kind.is_none() || kind == Some(ManagementObjectKind::WorkflowVersion) {
                let mut statement = connection.prepare(
                    "SELECT workflow_id, version,
                            COALESCE(display_name, json_extract(version_json, '$.draft.name'), workflow_id),
                            lifecycle_revision, deleted_at, deletion_operation_id
                     FROM workflow_versions
                     WHERE project_id = ?1 AND deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC, workflow_id, version",
                )?;
                let rows = statement.query_map([&scope.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?;
                for row in rows {
                    let (id, version, name, revision, deleted_at, operation_id) = row?;
                    entries.push(trash_entry(
                        scope,
                        ManagementObjectRef {
                            kind: ManagementObjectKind::WorkflowVersion,
                            id,
                            version: Some(version),
                            expected_revision: to_u64(revision),
                        },
                        format!("{name} · v{version}"),
                        &deleted_at,
                        operation_id,
                    )?);
                }
            }
            if kind.is_none() || kind == Some(ManagementObjectKind::Pipeline) {
                let mut statement = connection.prepare(
                    "SELECT workflow_id, display_name, lifecycle_revision, deleted_at,
                            deletion_operation_id
                     FROM workflow_pipelines
                     WHERE project_id = ?1 AND deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC, workflow_id",
                )?;
                let rows = statement.query_map([&scope.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?;
                for row in rows {
                    let (id, name, revision, deleted_at, operation_id) = row?;
                    entries.push(trash_entry(
                        scope,
                        ManagementObjectRef {
                            kind: ManagementObjectKind::Pipeline,
                            id,
                            version: None,
                            expected_revision: to_u64(revision),
                        },
                        name,
                        &deleted_at,
                        operation_id,
                    )?);
                }
            }
            entries.sort_by_key(|entry| Reverse(entry.deleted_at));
            Ok(entries)
        })
    }

    pub fn get_management_operation(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
    ) -> Result<ManagementReceipt, StorageError> {
        self.with_connection(|connection| {
            let result = connection
                .query_row(
                    "SELECT result_json FROM management_operations
                     WHERE id = ?1 AND project_id = ?2",
                    params![operation_id, scope.project_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    management_error(
                        "foreign_project_object",
                        "management operation was not found in this Project",
                    )
                })?
                .ok_or_else(|| {
                    management_error(
                        "operation_in_progress",
                        "management operation has not completed",
                    )
                })?;
            serde_json::from_str(&result).map_err(StorageError::from)
        })
    }

    pub fn run_provenance_summary(
        &self,
        run_id: &str,
    ) -> Result<Option<RunProvenanceSummary>, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT run_id, project_id, workflow_id, workflow_version,
                            workflow_content_hash, provider, model, summary_json, purged_at
                     FROM run_provenance_tombstones WHERE run_id = ?1",
                    [run_id],
                    |row| {
                        let summary = row.get::<_, String>(7)?;
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<u32>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            summary,
                            row.get::<_, String>(8)?,
                        ))
                    },
                )
                .optional()?
                .map(
                    |(
                        run_id,
                        project_id,
                        workflow_id,
                        workflow_version,
                        workflow_content_hash,
                        provider,
                        model,
                        summary,
                        purged_at,
                    )| {
                        Ok(RunProvenanceSummary {
                            run_id,
                            project_id,
                            source_deleted: true,
                            workflow_id,
                            workflow_version,
                            workflow_content_hash,
                            provider,
                            model,
                            summary: serde_json::from_str(&summary)?,
                            purged_at: parse_datetime(&purged_at)?,
                        })
                    },
                )
                .transpose()
        })
    }

    pub fn batch_lifecycle_metadata(
        &self,
        batch_id: &str,
    ) -> Result<BatchLifecycleMetadata, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT b.lifecycle_revision, b.archived_at, b.deleted_at,
                            b.deletion_operation_id,
                            (SELECT COUNT(*) FROM batch_images bi
                             JOIN runs child ON child.id = bi.child_run_id
                             WHERE bi.batch_id = b.id AND child.deleted_at IS NOT NULL)
                     FROM dataset_batches b WHERE b.id = ?1",
                    [batch_id],
                    |row| {
                        Ok(BatchLifecycleMetadata {
                            lifecycle_revision: to_u64(row.get(0)?),
                            archived_at: row.get(1)?,
                            deleted_at: row.get(2)?,
                            deletion_operation_id: row.get(3)?,
                            deleted_child_runs: usize::try_from(row.get::<_, i64>(4)?.max(0))
                                .unwrap_or(usize::MAX),
                        })
                    },
                )
                .optional()?
                .ok_or_else(|| management_error("entity_purged", "Dataset Run was not found"))
        })
    }

    pub fn project_management_usage(
        &self,
        scope: &ManagementScope,
    ) -> Result<ManagementUsageSummary, StorageError> {
        self.with_connection(|connection| {
            let stable_project_id = scope.stable_project_id.to_string();
            let visible = read_usage_totals(
                connection,
                "SELECT COALESCE(SUM(u.input_tokens), 0), COALESCE(SUM(u.output_tokens), 0),
                        COALESCE(SUM(u.total_tokens), 0), COALESCE(SUM(CAST(u.cost AS REAL)), 0.0)
                 FROM usage_records u JOIN runs r ON r.id = u.run_id
                 WHERE r.project_id = ?1 AND r.deleted_at IS NULL",
                &stable_project_id,
            )?;
            let current_history = read_usage_totals(
                connection,
                "SELECT COALESCE(SUM(u.input_tokens), 0), COALESCE(SUM(u.output_tokens), 0),
                        COALESCE(SUM(u.total_tokens), 0), COALESCE(SUM(CAST(u.cost AS REAL)), 0.0)
                 FROM usage_records u JOIN runs r ON r.id = u.run_id
                 WHERE r.project_id = ?1",
                &stable_project_id,
            )?;
            let cleaned = read_usage_totals(
                connection,
                "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(total_tokens), 0), COALESCE(SUM(CAST(cost AS REAL)), 0.0)
                 FROM historical_usage_ledger WHERE project_id = ?1",
                &stable_project_id,
            )?;
            Ok(ManagementUsageSummary {
                visible_runs: lifecycle_usage_totals(visible),
                cleaned_up_runs: lifecycle_usage_totals(cleaned),
                historical_total: lifecycle_usage_totals((
                    current_history.0.saturating_add(cleaned.0),
                    current_history.1.saturating_add(cleaned.1),
                    current_history.2.saturating_add(cleaned.2),
                    current_history.3 + cleaned.3,
                )),
            })
        })
    }
}

fn read_usage_totals(
    connection: &Connection,
    sql: &str,
    project_id: &str,
) -> Result<(i64, i64, i64, f64), StorageError> {
    connection
        .query_row(sql, [project_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(StorageError::from)
}

fn lifecycle_usage_totals(values: (i64, i64, i64, f64)) -> LifecycleUsageTotals {
    LifecycleUsageTotals {
        input_tokens: u64::try_from(values.0.max(0)).unwrap_or(u64::MAX),
        output_tokens: u64::try_from(values.1.max(0)).unwrap_or(u64::MAX),
        total_tokens: u64::try_from(values.2.max(0)).unwrap_or(u64::MAX),
        cost: format!("{:.6}", values.3.max(0.0)),
    }
}

fn preview(
    connection: &Connection,
    scope: &ManagementScope,
    request: &ManagementRequest,
) -> Result<ManagementPreview, StorageError> {
    let objects = normalize_objects(connection, scope, &request.objects)?;
    let mut blockers = Vec::new();
    let mut impact = ManagementImpact {
        top_level_objects: objects.len(),
        estimated_reclaimable_bytes: None,
        estimate_note: match request.action {
            ManagementAction::MoveToTrash | ManagementAction::Restore => {
                "Moving items to or from Trash does not free disk space.".to_owned()
            }
            _ => "Database and file reclamation is measured during explicit cleanup.".to_owned(),
        },
        ..ManagementImpact::default()
    };
    let mut impacted_runs = BTreeSet::new();
    for object in &objects {
        match object.kind {
            ManagementObjectKind::Run => {
                let run = read_run(connection, &object.id)?;
                check_run(
                    connection,
                    scope,
                    request.action,
                    object,
                    &run,
                    &mut blockers,
                )?;
                impacted_runs.insert(run.id);
            }
            ManagementObjectKind::Batch => {
                let batch = read_batch(connection, &object.id)?;
                check_batch(
                    connection,
                    scope,
                    request.action,
                    object,
                    &batch,
                    &mut blockers,
                )?;
                let child_runs = batch_impacted_runs(connection, &batch, request.action)?;
                impact.child_runs += child_runs.len();
                impacted_runs.extend(child_runs);
            }
            ManagementObjectKind::WorkflowDraft => {
                let draft = read_draft(connection, &object.id)?;
                check_draft(
                    connection,
                    scope,
                    request.action,
                    object,
                    &draft,
                    &mut blockers,
                )?;
            }
            ManagementObjectKind::WorkflowVersion => {
                let version = read_version(
                    connection,
                    &object.id,
                    object.version.expect("validated version"),
                )?;
                check_version(connection, scope, request, object, &version, &mut blockers)?;
                impact.historical_run_references += historical_run_references(
                    connection,
                    scope,
                    &version.workflow_id,
                    Some(version.version),
                )?;
            }
            ManagementObjectKind::Pipeline => {
                let pipeline = read_pipeline(connection, &object.id)?;
                check_pipeline(connection, scope, request, object, &pipeline, &mut blockers)?;
                impact.historical_run_references +=
                    historical_run_references(connection, scope, &pipeline.workflow_id, None)?;
            }
        }
    }
    for run_id in impacted_runs {
        impact.unresolved_reviews_hidden += count(
            connection,
            "SELECT COUNT(*) FROM review_queue WHERE run_id = ?1 AND status = 'pending'",
            &run_id,
        )?;
        impact.confirmed_annotations_retained += count(
            connection,
            "SELECT COUNT(*) FROM annotations
             WHERE run_id = ?1 AND review_status IN ('auto_accepted', 'human_accepted')",
            &run_id,
        )?;
        impact.calibration_references += count(
            connection,
            "SELECT COUNT(*) FROM geometry_correction_evidence WHERE run_id = ?1",
            &run_id,
        )?;
        for sql in [
            "SELECT COUNT(*) FROM run_steps WHERE run_id = ?1",
            "SELECT COUNT(*) FROM run_events WHERE run_id = ?1",
            "SELECT COUNT(*) FROM model_messages WHERE run_id = ?1",
            "SELECT COUNT(*) FROM tool_calls WHERE run_id = ?1",
            "SELECT COUNT(*) FROM vision_artifacts WHERE run_id = ?1",
            "SELECT COUNT(*) FROM validation_issues WHERE run_id = ?1",
        ] {
            impact.debug_rows += count(connection, sql, &run_id)?;
        }
    }
    let can_execute = blockers.is_empty();
    let recoverable = request.action != ManagementAction::Purge;
    let summary = summary(request.action, &objects, &impact, can_execute);
    let navigation_target = if objects.iter().all(|object| {
        matches!(
            object.kind,
            ManagementObjectKind::Run | ManagementObjectKind::Batch
        )
    }) {
        format!("/projects/{}/runs", scope.project_id)
    } else {
        format!("/projects/{}/build/pipeline", scope.project_id)
    };
    let material = serde_json::to_vec(&(
        &scope.project_id,
        scope.stable_project_id,
        request.action,
        &objects,
        &impact,
        &blockers,
        &request.replacement_default_version,
        request.clear_default,
        &request.display_name,
    ))?;
    let confirmation_token = annotagent_image_tools::sha256(&material);
    Ok(ManagementPreview {
        project_id: scope.project_id.clone(),
        action: request.action,
        objects,
        impact,
        blockers,
        confirmation_token,
        can_execute,
        recoverable,
        navigation_target,
        summary,
    })
}

fn normalize_objects(
    connection: &Connection,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let selected_batches = objects
        .iter()
        .filter(|object| object.kind == ManagementObjectKind::Batch)
        .map(|object| object.id.clone())
        .collect::<Vec<_>>();
    let mut child_runs = BTreeSet::new();
    for batch_id in selected_batches {
        if let Ok(batch) = read_batch(connection, &batch_id)
            && batch.project_id == scope.project_id
        {
            child_runs.extend(batch_child_runs(connection, &batch_id)?);
        }
    }
    let selected_pipelines = objects
        .iter()
        .filter(|object| object.kind == ManagementObjectKind::Pipeline)
        .map(|object| object.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut normalized = objects
        .iter()
        .filter(|object| {
            (object.kind != ManagementObjectKind::Run || !child_runs.contains(&object.id))
                && !matches!(
                    object.kind,
                    ManagementObjectKind::WorkflowDraft | ManagementObjectKind::WorkflowVersion
                )
                .then(|| selected_pipelines.contains(object.id.as_str()))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn check_run(
    connection: &Connection,
    scope: &ManagementScope,
    action: ManagementAction,
    object: &ManagementObjectRef,
    run: &RunLifecycle,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    if run.project_id.as_deref() != Some(&scope.stable_project_id.to_string()) {
        blockers.push(blocker(
            "foreign_project_object",
            object,
            "Run does not belong to this Project.",
        ));
        return Ok(());
    }
    if run.revision != object.expected_revision {
        blockers.push(blocker(
            "revision_conflict",
            object,
            &format!(
                "Run changed after it was selected (expected revision {}, current revision {}).",
                object.expected_revision, run.revision
            ),
        ));
    }
    match action {
        ManagementAction::MoveToTrash => {
            if run.deleted_at.is_some() {
                blockers.push(blocker(
                    "entity_in_trash",
                    object,
                    "Run is already in Trash.",
                ));
            }
            let active_row = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM active_project_runs WHERE run_id = ?1)",
                [&run.id],
                |row| row.get::<_, bool>(0),
            )?;
            if active_row || is_active_run_status(run.status) {
                blockers.push(blocker(
                    "run_active",
                    object,
                    "Run is still executable. Cancel it and wait for terminal worker state before deleting.",
                ));
            }
        }
        ManagementAction::Restore => {
            if run.deleted_at.is_none() {
                blockers.push(blocker(
                    "entity_not_in_trash",
                    object,
                    "Run is not in Trash.",
                ));
            }
        }
        ManagementAction::Purge => {
            if run.deleted_at.is_none() {
                blockers.push(blocker(
                    "entity_not_in_trash",
                    object,
                    "Run must be in Trash before permanent cleanup.",
                ));
            }
        }
        ManagementAction::CancelAndDelete => {
            if run.deleted_at.is_some() {
                blockers.push(blocker(
                    "entity_in_trash",
                    object,
                    "Run is already in Trash.",
                ));
            }
        }
        _ => blockers.push(blocker(
            "unsupported_management_action",
            object,
            "This action is not supported for Runs.",
        )),
    }
    Ok(())
}

fn check_batch(
    connection: &Connection,
    scope: &ManagementScope,
    action: ManagementAction,
    object: &ManagementObjectRef,
    batch: &BatchLifecycle,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    if batch.project_id != scope.project_id {
        blockers.push(blocker(
            "foreign_project_object",
            object,
            "Dataset Run does not belong to this Project.",
        ));
        return Ok(());
    }
    if batch.revision != object.expected_revision {
        blockers.push(blocker(
            "revision_conflict",
            object,
            &format!(
                "Dataset Run changed after it was selected (expected revision {}, current revision {}).",
                object.expected_revision, batch.revision
            ),
        ));
    }
    match action {
        ManagementAction::MoveToTrash => {
            if batch.deleted_at.is_some() {
                blockers.push(blocker(
                    "entity_in_trash",
                    object,
                    "Dataset Run is already in Trash.",
                ));
            }
            let has_live_lease = batch.lease_owner.is_some()
                && batch
                    .lease_expires_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .is_some_and(|expires_at| expires_at > Utc::now());
            if !batch.status.is_terminal() || has_live_lease {
                blockers.push(blocker(
                    "run_active",
                    object,
                    "Dataset Run or its worker lease is still active. Cancel the Dataset Run and wait for termination.",
                ));
            }
            for child in batch_child_runs(connection, &batch.id)? {
                let run = read_run(connection, &child)?;
                if is_active_run_status(run.status)
                    || connection.query_row(
                        "SELECT EXISTS(SELECT 1 FROM active_project_runs WHERE run_id = ?1)",
                        [&run.id],
                        |row| row.get::<_, bool>(0),
                    )?
                {
                    blockers.push(ManagementBlocker {
                        code: "run_active".to_owned(),
                        object: object.clone(),
                        message: "Dataset Run has an active child Run.".to_owned(),
                        related_ids: vec![run.id],
                    });
                }
            }
        }
        ManagementAction::Restore => {
            if batch.deleted_at.is_none() {
                blockers.push(blocker(
                    "entity_not_in_trash",
                    object,
                    "Dataset Run is not in Trash.",
                ));
            }
        }
        ManagementAction::Purge => {
            if batch.deleted_at.is_none() {
                blockers.push(blocker(
                    "entity_not_in_trash",
                    object,
                    "Dataset Run must be in Trash before permanent cleanup.",
                ));
            }
        }
        ManagementAction::CancelAndDelete => {
            if batch.deleted_at.is_some() {
                blockers.push(blocker(
                    "entity_in_trash",
                    object,
                    "Dataset Run is already in Trash.",
                ));
            }
        }
        _ => blockers.push(blocker(
            "unsupported_management_action",
            object,
            "This action is not supported for Dataset Runs.",
        )),
    }
    Ok(())
}

fn check_draft(
    connection: &Connection,
    scope: &ManagementScope,
    action: ManagementAction,
    object: &ManagementObjectRef,
    draft: &DraftLifecycle,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    if draft.project_id != scope.project_id {
        blockers.push(blocker(
            "foreign_project_object",
            object,
            "Pipeline Draft does not belong to this Project.",
        ));
        return Ok(());
    }
    lifecycle_revision_blocker(object, draft.lifecycle_revision, "Pipeline Draft", blockers);
    lifecycle_state_blockers(
        action,
        object,
        draft.archived_at.as_deref(),
        draft.deleted_at.as_deref(),
        blockers,
    );
    if matches!(
        action,
        ManagementAction::MoveToTrash | ManagementAction::Archive
    ) && draft_has_active_writer(connection, scope, &draft.id)?
    {
        blockers.push(blocker(
            "active_builder_session",
            object,
            "An active Builder or Sample Test is using this Draft.",
        ));
    }
    if matches!(
        action,
        ManagementAction::Rename | ManagementAction::SetDefault | ManagementAction::ClearDefault
    ) {
        blockers.push(blocker(
            "unsupported_management_action",
            object,
            "This action is not supported for Pipeline Drafts.",
        ));
    }
    Ok(())
}

fn check_version(
    connection: &Connection,
    scope: &ManagementScope,
    request: &ManagementRequest,
    object: &ManagementObjectRef,
    version: &VersionLifecycle,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    if version.project_id != scope.project_id {
        blockers.push(blocker(
            "foreign_project_object",
            object,
            "Published Version does not belong to this Project.",
        ));
        return Ok(());
    }
    lifecycle_revision_blocker(
        object,
        version.lifecycle_revision,
        "Published Version",
        blockers,
    );
    lifecycle_state_blockers(
        request.action,
        object,
        version.archived_at.as_deref(),
        version.deleted_at.as_deref(),
        blockers,
    );
    let parent = read_pipeline(connection, &version.workflow_id)?;
    if matches!(
        request.action,
        ManagementAction::Restore | ManagementAction::SetDefault
    ) && (parent.deleted_at.is_some() || parent.archived_at.is_some())
    {
        blockers.push(blocker(
            "parent_pipeline_unavailable",
            object,
            "Restore or unarchive the parent Pipeline before using this Version.",
        ));
    }
    if matches!(
        request.action,
        ManagementAction::MoveToTrash | ManagementAction::Archive
    ) && version_has_active_execution(connection, scope, &version.workflow_id, version.version)?
    {
        blockers.push(blocker(
            "run_active",
            object,
            "An active Run or Dataset Run is using this immutable Version.",
        ));
    }
    if matches!(
        request.action,
        ManagementAction::MoveToTrash | ManagementAction::Archive
    ) {
        check_default_transition(
            connection,
            scope,
            request,
            object,
            &version.workflow_id,
            Some(version.version),
            blockers,
        )?;
    }
    match request.action {
        ManagementAction::Rename => blockers.push(blocker(
            "unsupported_management_action",
            object,
            "Rename the Pipeline display alias instead of immutable Version content.",
        )),
        ManagementAction::SetDefault => {
            if version.deleted_at.is_some() || version.archived_at.is_some() {
                blockers.push(blocker(
                    "entity_unavailable",
                    object,
                    "A deleted or archived Version cannot be the Project default.",
                ));
            }
        }
        ManagementAction::ClearDefault => {
            let expected = WorkflowVersionRef {
                workflow_id: version.workflow_id.clone(),
                version: version.version,
            };
            match read_default(connection, &scope.project_id)? {
                Some(current) if current == expected => {}
                _ => blockers.push(blocker(
                    "default_not_selected",
                    object,
                    "This Version is not the current Project default.",
                )),
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_pipeline(
    connection: &Connection,
    scope: &ManagementScope,
    request: &ManagementRequest,
    object: &ManagementObjectRef,
    pipeline: &PipelineLifecycle,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    if pipeline.project_id != scope.project_id {
        blockers.push(blocker(
            "foreign_project_object",
            object,
            "Pipeline does not belong to this Project.",
        ));
        return Ok(());
    }
    lifecycle_revision_blocker(object, pipeline.lifecycle_revision, "Pipeline", blockers);
    lifecycle_state_blockers(
        request.action,
        object,
        pipeline.archived_at.as_deref(),
        pipeline.deleted_at.as_deref(),
        blockers,
    );
    if matches!(
        request.action,
        ManagementAction::MoveToTrash | ManagementAction::Archive
    ) {
        if draft_has_active_writer(connection, scope, &pipeline.workflow_id)? {
            blockers.push(blocker(
                "active_builder_session",
                object,
                "An active Builder or Sample Test is using this Pipeline.",
            ));
        }
        if pipeline_has_active_execution(connection, scope, &pipeline.workflow_id)? {
            blockers.push(blocker(
                "run_active",
                object,
                "An active Run or Dataset Run is using a Version of this Pipeline.",
            ));
        }
        check_default_transition(
            connection,
            scope,
            request,
            object,
            &pipeline.workflow_id,
            None,
            blockers,
        )?;
    }
    if matches!(
        request.action,
        ManagementAction::SetDefault | ManagementAction::ClearDefault
    ) {
        blockers.push(blocker(
            "unsupported_management_action",
            object,
            "Set or clear the Project default on a specific Published Version.",
        ));
    }
    if request.action == ManagementAction::Purge {
        let deletion_operation = pipeline.deletion_operation_id.as_deref();
        let protected_children =
            pipeline_children(connection, scope, &pipeline.workflow_id, true, None)?
                .into_iter()
                .filter(|child| match child.kind {
                    ManagementObjectKind::WorkflowDraft => read_draft(connection, &child.id)
                        .is_ok_and(|draft| {
                            draft.deletion_operation_id.as_deref() != deletion_operation
                        }),
                    ManagementObjectKind::WorkflowVersion => {
                        read_version(connection, &child.id, child.version.expect("Version child"))
                            .is_ok_and(|version| {
                                version.deletion_operation_id.as_deref() != deletion_operation
                            })
                    }
                    _ => false,
                })
                .map(|child| format!("{}@{}", child.id, child.version.unwrap_or(0)))
                .collect::<Vec<_>>();
        if !protected_children.is_empty() {
            blockers.push(ManagementBlocker {
                code: "referenced_data_protected".to_owned(),
                object: object.clone(),
                message: "This Pipeline has children removed by a different operation. Clean up or restore those items explicitly before cleaning up the parent."
                    .to_owned(),
                related_ids: protected_children,
            });
        }
    }
    Ok(())
}

fn lifecycle_revision_blocker(
    object: &ManagementObjectRef,
    current: u64,
    label: &str,
    blockers: &mut Vec<ManagementBlocker>,
) {
    if object.expected_revision != current {
        blockers.push(blocker(
            "revision_conflict",
            object,
            &format!(
                "{label} changed after selection (expected revision {}, current revision {current}).",
                object.expected_revision
            ),
        ));
    }
}

fn lifecycle_state_blockers(
    action: ManagementAction,
    object: &ManagementObjectRef,
    archived_at: Option<&str>,
    deleted_at: Option<&str>,
    blockers: &mut Vec<ManagementBlocker>,
) {
    match action {
        ManagementAction::MoveToTrash if deleted_at.is_some() => blockers.push(blocker(
            "entity_in_trash",
            object,
            "The selected item is already in Trash.",
        )),
        ManagementAction::Restore | ManagementAction::Purge if deleted_at.is_none() => blockers
            .push(blocker(
                "entity_not_in_trash",
                object,
                "The selected item is not in Trash.",
            )),
        ManagementAction::Archive if archived_at.is_some() => blockers.push(blocker(
            "entity_archived",
            object,
            "The selected item is already archived.",
        )),
        ManagementAction::Unarchive if archived_at.is_none() => blockers.push(blocker(
            "entity_not_archived",
            object,
            "The selected item is not archived.",
        )),
        _ => {}
    }
    if !matches!(action, ManagementAction::Restore | ManagementAction::Purge)
        && deleted_at.is_some()
    {
        blockers.push(blocker(
            "entity_in_trash",
            object,
            "Restore this item before applying another lifecycle action.",
        ));
    }
}

fn check_default_transition(
    connection: &Connection,
    scope: &ManagementScope,
    request: &ManagementRequest,
    object: &ManagementObjectRef,
    workflow_id: &str,
    version: Option<u32>,
    blockers: &mut Vec<ManagementBlocker>,
) -> Result<(), StorageError> {
    let Some(current) = read_default(connection, &scope.project_id)? else {
        return Ok(());
    };
    let targets_default = current.workflow_id == workflow_id
        && version.is_none_or(|version| current.version == version);
    if !targets_default {
        return Ok(());
    }
    if !request.clear_default && request.replacement_default_version.is_none() {
        blockers.push(blocker(
            "default_replacement_required",
            object,
            "Choose another available Published Version or explicitly clear the Project default.",
        ));
        return Ok(());
    }
    if let Some(replacement) = &request.replacement_default_version {
        match read_version(connection, &replacement.workflow_id, replacement.version) {
            Ok(candidate)
                if candidate.project_id == scope.project_id
                    && candidate.deleted_at.is_none()
                    && candidate.archived_at.is_none()
                    && !(candidate.workflow_id == workflow_id
                        && version.is_none_or(|version| candidate.version == version)) => {}
            _ => blockers.push(blocker(
                "default_replacement_invalid",
                object,
                "The replacement must be another available Published Version in this Project.",
            )),
        }
    }
    Ok(())
}

fn move_to_trash(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
    operation_id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let mut affected = Vec::new();
    let now = now.to_rfc3339();
    for object in objects {
        match object.kind {
            ManagementObjectKind::Run => {
                let run = read_run(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &run_state(&run))?;
                let changed = transaction.execute(
                    "UPDATE runs
                     SET deleted_at = ?3, deletion_operation_id = ?4,
                         lifecycle_revision = lifecycle_revision + 1
                     WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?5
                       AND deleted_at IS NULL",
                    params![
                        object.id,
                        scope.stable_project_id.to_string(),
                        now,
                        operation_id,
                        to_i64(object.expected_revision),
                    ],
                )?;
                ensure_one(changed, "revision_conflict", "Run changed during deletion")?;
                affected.push(ManagementObjectRef {
                    expected_revision: object.expected_revision.saturating_add(1),
                    ..object.clone()
                });
            }
            ManagementObjectKind::Batch => {
                let batch = read_batch(transaction, &object.id)?;
                let child_runs = batch_child_runs(transaction, &batch.id)?;
                for child_id in child_runs {
                    let child = read_run(transaction, &child_id)?;
                    if child.deleted_at.is_some() {
                        continue;
                    }
                    let child_ref = ManagementObjectRef {
                        kind: ManagementObjectKind::Run,
                        id: child.id.clone(),
                        version: None,
                        expected_revision: child.revision,
                    };
                    record_item(transaction, operation_id, &child_ref, &run_state(&child))?;
                    transaction.execute(
                        "UPDATE runs
                         SET deleted_at = ?2, deletion_operation_id = ?3,
                             lifecycle_revision = lifecycle_revision + 1
                         WHERE id = ?1 AND deleted_at IS NULL",
                        params![child.id, now, operation_id],
                    )?;
                    affected.push(ManagementObjectRef {
                        expected_revision: child.revision.saturating_add(1),
                        ..child_ref
                    });
                }
                record_item(transaction, operation_id, object, &batch_state(&batch))?;
                let changed = transaction.execute(
                    "UPDATE dataset_batches
                     SET deleted_at = ?3, deletion_operation_id = ?4,
                         lifecycle_revision = lifecycle_revision + 1
                     WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?5
                       AND deleted_at IS NULL",
                    params![
                        object.id,
                        scope.project_id,
                        now,
                        operation_id,
                        to_i64(object.expected_revision),
                    ],
                )?;
                ensure_one(
                    changed,
                    "revision_conflict",
                    "Dataset Run changed during deletion",
                )?;
                affected.push(ManagementObjectRef {
                    expected_revision: object.expected_revision.saturating_add(1),
                    ..object.clone()
                });
            }
            ManagementObjectKind::WorkflowDraft => {
                let draft = read_draft(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &draft_state(&draft))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_drafts SET deleted_at = ?2, deletion_operation_id = ?3,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE id = ?1 AND project_id = ?4 AND lifecycle_revision = ?5
                           AND deleted_at IS NULL",
                        params![
                            object.id,
                            now,
                            operation_id,
                            scope.project_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline Draft changed during deletion",
                )?;
                affected.push(incremented(object));
            }
            ManagementObjectKind::WorkflowVersion => {
                let version_number = object.version.expect("validated version");
                let version = read_version(transaction, &object.id, version_number)?;
                record_item(transaction, operation_id, object, &version_state(&version))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_versions SET deleted_at = ?3, deletion_operation_id = ?4,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE workflow_id = ?1 AND version = ?2 AND project_id = ?5
                           AND lifecycle_revision = ?6 AND deleted_at IS NULL",
                        params![
                            object.id,
                            version_number,
                            now,
                            operation_id,
                            scope.project_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Published Version changed during deletion",
                )?;
                affected.push(incremented(object));
            }
            ManagementObjectKind::Pipeline => {
                let pipeline = read_pipeline(transaction, &object.id)?;
                for child in pipeline_children(transaction, scope, &object.id, false, None)? {
                    record_and_set_child_deleted(
                        transaction,
                        operation_id,
                        &child,
                        &now,
                        Some(operation_id),
                    )?;
                    affected.push(incremented(&child));
                }
                record_item(
                    transaction,
                    operation_id,
                    object,
                    &pipeline_state(&pipeline),
                )?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_pipelines SET deleted_at = ?3, deletion_operation_id = ?4,
                           lifecycle_revision = lifecycle_revision + 1, updated_at = ?3
                         WHERE workflow_id = ?1 AND project_id = ?2 AND lifecycle_revision = ?5
                           AND deleted_at IS NULL",
                        params![
                            object.id,
                            scope.project_id,
                            now,
                            operation_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline changed during deletion",
                )?;
                affected.push(incremented(object));
            }
        }
    }
    affected.sort();
    Ok(affected)
}

fn restore(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
    operation_id: &str,
    _now: DateTime<Utc>,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let mut affected = Vec::new();
    for object in objects {
        match object.kind {
            ManagementObjectKind::Run => {
                let run = read_run(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &run_state(&run))?;
                let changed = transaction.execute(
                    "UPDATE runs
                     SET deleted_at = NULL, deletion_operation_id = NULL,
                         lifecycle_revision = lifecycle_revision + 1
                     WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                       AND deleted_at IS NOT NULL",
                    params![
                        object.id,
                        scope.stable_project_id.to_string(),
                        to_i64(object.expected_revision),
                    ],
                )?;
                ensure_one(changed, "revision_conflict", "Run changed during restore")?;
                affected.push(ManagementObjectRef {
                    expected_revision: object.expected_revision.saturating_add(1),
                    ..object.clone()
                });
            }
            ManagementObjectKind::Batch => {
                let batch = read_batch(transaction, &object.id)?;
                let deletion_operation = batch.deletion_operation_id.clone().ok_or_else(|| {
                    management_error("entity_not_in_trash", "Dataset Run is not in Trash")
                })?;
                for child_id in batch_child_runs(transaction, &batch.id)? {
                    let child = read_run(transaction, &child_id)?;
                    if child.deletion_operation_id.as_deref() != Some(&deletion_operation) {
                        continue;
                    }
                    let child_ref = ManagementObjectRef {
                        kind: ManagementObjectKind::Run,
                        id: child.id.clone(),
                        version: None,
                        expected_revision: child.revision,
                    };
                    record_item(transaction, operation_id, &child_ref, &run_state(&child))?;
                    transaction.execute(
                        "UPDATE runs
                         SET deleted_at = NULL, deletion_operation_id = NULL,
                             lifecycle_revision = lifecycle_revision + 1
                         WHERE id = ?1 AND deletion_operation_id = ?2",
                        params![child.id, deletion_operation],
                    )?;
                    affected.push(ManagementObjectRef {
                        expected_revision: child.revision.saturating_add(1),
                        ..child_ref
                    });
                }
                record_item(transaction, operation_id, object, &batch_state(&batch))?;
                let changed = transaction.execute(
                    "UPDATE dataset_batches
                     SET deleted_at = NULL, deletion_operation_id = NULL,
                         lifecycle_revision = lifecycle_revision + 1
                     WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                       AND deleted_at IS NOT NULL",
                    params![
                        object.id,
                        scope.project_id,
                        to_i64(object.expected_revision),
                    ],
                )?;
                ensure_one(
                    changed,
                    "revision_conflict",
                    "Dataset Run changed during restore",
                )?;
                affected.push(ManagementObjectRef {
                    expected_revision: object.expected_revision.saturating_add(1),
                    ..object.clone()
                });
            }
            ManagementObjectKind::WorkflowDraft => {
                let draft = read_draft(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &draft_state(&draft))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_drafts SET deleted_at = NULL, deletion_operation_id = NULL,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                           AND deleted_at IS NOT NULL",
                        params![object.id, scope.project_id, to_i64(object.expected_revision)],
                    )?,
                    "revision_conflict",
                    "Pipeline Draft changed during restore",
                )?;
                affected.push(incremented(object));
            }
            ManagementObjectKind::WorkflowVersion => {
                let version_number = object.version.expect("validated version");
                let version = read_version(transaction, &object.id, version_number)?;
                record_item(transaction, operation_id, object, &version_state(&version))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_versions SET deleted_at = NULL, deletion_operation_id = NULL,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE workflow_id = ?1 AND version = ?2 AND project_id = ?3
                           AND lifecycle_revision = ?4 AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            version_number,
                            scope.project_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Published Version changed during restore",
                )?;
                affected.push(incremented(object));
            }
            ManagementObjectKind::Pipeline => {
                let pipeline = read_pipeline(transaction, &object.id)?;
                let deletion_operation =
                    pipeline.deletion_operation_id.clone().ok_or_else(|| {
                        management_error("entity_not_in_trash", "Pipeline is not in Trash")
                    })?;
                for child in pipeline_children(
                    transaction,
                    scope,
                    &object.id,
                    true,
                    Some(&deletion_operation),
                )? {
                    record_and_set_child_deleted(transaction, operation_id, &child, "", None)?;
                    affected.push(incremented(&child));
                }
                record_item(
                    transaction,
                    operation_id,
                    object,
                    &pipeline_state(&pipeline),
                )?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_pipelines SET deleted_at = NULL,
                           deletion_operation_id = NULL, lifecycle_revision = lifecycle_revision + 1,
                           updated_at = ?4
                         WHERE workflow_id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                           AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            scope.project_id,
                            to_i64(object.expected_revision),
                            Utc::now().to_rfc3339(),
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline changed during restore",
                )?;
                affected.push(incremented(object));
            }
        }
    }
    affected.sort();
    Ok(affected)
}

fn purge_objects(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
    operation_id: &str,
    now: DateTime<Utc>,
) -> Result<(Vec<ManagementObjectRef>, Option<PurgeReport>), StorageError> {
    let mut affected = Vec::new();
    let mut report = PurgeReport::default();
    for object in objects {
        match object.kind {
            ManagementObjectKind::Run => {
                purge_run(transaction, scope, object, operation_id, &now, &mut report)?;
            }
            ManagementObjectKind::Batch => {
                let batch = read_batch(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &batch_state(&batch))?;
                let deletion_operation =
                    batch.deletion_operation_id.as_deref().ok_or_else(|| {
                        management_error("entity_not_in_trash", "Dataset Run is not in Trash")
                    })?;
                for child_id in batch_child_runs(transaction, &batch.id)? {
                    let child = read_run(transaction, &child_id)?;
                    if child.deletion_operation_id.as_deref() != Some(deletion_operation) {
                        continue;
                    }
                    let child_ref = ManagementObjectRef {
                        kind: ManagementObjectKind::Run,
                        id: child.id,
                        version: None,
                        expected_revision: child.revision,
                    };
                    purge_run(
                        transaction,
                        scope,
                        &child_ref,
                        operation_id,
                        &now,
                        &mut report,
                    )?;
                    affected.push(child_ref);
                }
                report.database_rows_removed += count(
                    transaction,
                    "SELECT COUNT(*) FROM batch_events WHERE batch_id = ?1",
                    &batch.id,
                )?;
                report.database_rows_removed += count(
                    transaction,
                    "SELECT COUNT(*) FROM batch_images WHERE batch_id = ?1",
                    &batch.id,
                )?;
                ensure_one(
                    transaction.execute(
                        "DELETE FROM dataset_batches
                         WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                           AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            scope.project_id,
                            to_i64(object.expected_revision)
                        ],
                    )?,
                    "revision_conflict",
                    "Dataset Run changed during permanent cleanup",
                )?;
                report.database_rows_removed += 1;
            }
            ManagementObjectKind::WorkflowDraft => {
                let draft = read_draft(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &draft_state(&draft))?;
                report.database_rows_removed += delete_count(
                    transaction,
                    "DELETE FROM workflow_sample_tests WHERE draft_id = ?1",
                    &object.id,
                )?;
                report.database_rows_removed += transaction.execute(
                    "DELETE FROM agent_sessions WHERE project_id = ?1 AND status != 'running'
                     AND (json_extract(session_json, '$.draft_id') = ?2
                       OR json_extract(session_json, '$.working_draft.draft_id') = ?2)",
                    params![scope.project_id, object.id],
                )?;
                ensure_one(
                    transaction.execute(
                        "DELETE FROM workflow_drafts
                         WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                           AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            scope.project_id,
                            to_i64(object.expected_revision)
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline Draft changed during permanent cleanup",
                )?;
                report.database_rows_removed += 1;
            }
            ManagementObjectKind::WorkflowVersion => {
                let version_number = object.version.expect("validated version");
                let version = read_version(transaction, &object.id, version_number)?;
                record_item(transaction, operation_id, object, &version_state(&version))?;
                ensure_one(
                    transaction.execute(
                        "DELETE FROM workflow_versions
                         WHERE workflow_id = ?1 AND version = ?2 AND project_id = ?3
                           AND lifecycle_revision = ?4 AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            version_number,
                            scope.project_id,
                            to_i64(object.expected_revision)
                        ],
                    )?,
                    "revision_conflict",
                    "Published Version changed during permanent cleanup",
                )?;
                report.database_rows_removed += 1;
                if historical_run_references(transaction, scope, &object.id, Some(version_number))?
                    > 0
                {
                    report.retained_provenance_records += 1;
                    push_reason(
                        &mut report,
                        "Historical Runs retain their immutable Workflow execution snapshot and content hash.",
                    );
                }
            }
            ManagementObjectKind::Pipeline => {
                let pipeline = read_pipeline(transaction, &object.id)?;
                let deletion_operation =
                    pipeline.deletion_operation_id.as_deref().ok_or_else(|| {
                        management_error("entity_not_in_trash", "Pipeline is not in Trash")
                    })?;
                for child in pipeline_children(
                    transaction,
                    scope,
                    &object.id,
                    true,
                    Some(deletion_operation),
                )? {
                    match child.kind {
                        ManagementObjectKind::WorkflowDraft => {
                            let draft = read_draft(transaction, &child.id)?;
                            record_item(transaction, operation_id, &child, &draft_state(&draft))?;
                            report.database_rows_removed += delete_count(
                                transaction,
                                "DELETE FROM workflow_sample_tests WHERE draft_id = ?1",
                                &child.id,
                            )?;
                            report.database_rows_removed += transaction.execute(
                                "DELETE FROM agent_sessions WHERE project_id = ?1 AND status != 'running'
                                 AND (json_extract(session_json, '$.draft_id') = ?2
                                   OR json_extract(session_json, '$.working_draft.draft_id') = ?2)",
                                params![scope.project_id, child.id],
                            )?;
                            report.database_rows_removed += transaction.execute(
                                "DELETE FROM workflow_drafts WHERE id = ?1 AND deleted_at IS NOT NULL",
                                [&child.id],
                            )?;
                        }
                        ManagementObjectKind::WorkflowVersion => {
                            let number = child.version.expect("Pipeline Version child");
                            let version = read_version(transaction, &child.id, number)?;
                            record_item(
                                transaction,
                                operation_id,
                                &child,
                                &version_state(&version),
                            )?;
                            report.database_rows_removed += transaction.execute(
                                "DELETE FROM workflow_versions
                                 WHERE workflow_id = ?1 AND version = ?2 AND deleted_at IS NOT NULL",
                                params![child.id, number],
                            )?;
                        }
                        _ => unreachable!("Pipeline child kind"),
                    }
                    affected.push(child);
                }
                record_item(
                    transaction,
                    operation_id,
                    object,
                    &pipeline_state(&pipeline),
                )?;
                ensure_one(
                    transaction.execute(
                        "DELETE FROM workflow_pipelines
                         WHERE workflow_id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
                           AND deleted_at IS NOT NULL",
                        params![
                            object.id,
                            scope.project_id,
                            to_i64(object.expected_revision)
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline changed during permanent cleanup",
                )?;
                report.database_rows_removed += 1;
                if historical_run_references(transaction, scope, &object.id, None)? > 0 {
                    report.retained_provenance_records += 1;
                    push_reason(
                        &mut report,
                        "Historical Runs retain independent immutable Workflow snapshots after Pipeline cleanup.",
                    );
                }
            }
        }
        affected.push(object.clone());
    }
    affected.sort();
    affected.dedup();
    if report.files_removed == 0 {
        push_reason(
            &mut report,
            "Original images, exports, model assets, credentials, and shared workspace files were not removed.",
        );
    }
    Ok((affected, Some(report)))
}

fn purge_run(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    object: &ManagementObjectRef,
    operation_id: &str,
    now: &DateTime<Utc>,
    report: &mut PurgeReport,
) -> Result<(), StorageError> {
    let run = read_run(transaction, &object.id)?;
    record_item(transaction, operation_id, object, &run_state(&run))?;
    let (provider, model, project_name, snapshot, created_at, updated_at) = transaction.query_row(
        "SELECT provider, model, project_name, workflow_snapshot_json, created_at, updated_at
         FROM runs WHERE id = ?1",
        [&object.id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )?;
    let snapshot_value = snapshot
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
    let selected = snapshot_value
        .as_ref()
        .and_then(|value| value.get("selected_workflow"));
    let workflow_id = selected
        .and_then(|value| value.get("workflow_id"))
        .and_then(serde_json::Value::as_str);
    let workflow_version = selected
        .and_then(|value| value.get("version"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let workflow_content_hash = selected
        .and_then(|value| value.get("content_hash"))
        .and_then(serde_json::Value::as_str);
    let retained_annotations = count(
        transaction,
        "SELECT COUNT(*) FROM annotations a WHERE a.run_id = ?1 AND (
           a.review_status IN ('auto_accepted', 'human_accepted')
           OR EXISTS(SELECT 1 FROM annotation_revisions ar WHERE ar.annotation_id = a.id)
           OR EXISTS(SELECT 1 FROM geometry_correction_evidence ge WHERE ge.annotation_id = a.id)
         )",
        &object.id,
    )?;
    let summary = serde_json::json!({
        "project_name": project_name,
        "run_status": run.status,
        "created_at": created_at,
        "updated_at": updated_at,
        "retained_annotations": retained_annotations,
        "workflow_name_snapshot": selected
            .and_then(|value| value.get("name"))
            .or_else(|| selected.and_then(|value| value.get("draft")).and_then(|value| value.get("name"))),
    });
    transaction.execute(
        "INSERT OR IGNORE INTO run_provenance_tombstones
         (run_id, project_id, workflow_id, workflow_version, workflow_content_hash,
          provider, model, summary_json, purged_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            object.id,
            scope.stable_project_id.to_string(),
            workflow_id,
            workflow_version,
            workflow_content_hash,
            provider,
            model,
            serde_json::to_string(&summary)?,
            now.to_rfc3339(),
        ],
    )?;
    report.retained_provenance_records += 1;

    let (input_tokens, output_tokens, total_tokens, cost, usage_rows) = transaction.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(total_tokens), 0), COALESCE(SUM(CAST(cost AS REAL)), 0.0), COUNT(*)
         FROM usage_records WHERE run_id = ?1",
        [&object.id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )?;
    if usage_rows > 0 {
        transaction.execute(
            "INSERT OR IGNORE INTO historical_usage_ledger
             (run_id, project_id, input_tokens, output_tokens, total_tokens, cost, currency, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'USD', ?7)",
            params![
                object.id,
                scope.stable_project_id.to_string(),
                input_tokens,
                output_tokens,
                total_tokens,
                format!("{cost:.6}"),
                now.to_rfc3339(),
            ],
        )?;
        report.retained_usage_records += 1;
    }
    report.retained_annotation_records += retained_annotations;
    if retained_annotations > 0 {
        push_reason(
            report,
            "Accepted annotations, human revisions, and correction evidence remain available and exportable.",
        );
    }

    report.database_rows_removed += delete_count(
        transaction,
        "DELETE FROM review_queue WHERE run_id = ?1",
        &object.id,
    )?;
    report.database_rows_removed += transaction.execute(
        "DELETE FROM annotations WHERE run_id = ?1 AND id NOT IN (
           SELECT a.id FROM annotations a WHERE a.run_id = ?1 AND (
             a.review_status IN ('auto_accepted', 'human_accepted')
             OR EXISTS(SELECT 1 FROM annotation_revisions ar WHERE ar.annotation_id = a.id)
             OR EXISTS(SELECT 1 FROM geometry_correction_evidence ge WHERE ge.annotation_id = a.id)
           )
         )",
        [&object.id],
    )?;
    report.database_rows_removed += transaction.execute(
        "DELETE FROM vision_artifacts WHERE run_id = ?1 AND artifact_id NOT IN (
           SELECT candidate_artifact_id FROM geometry_quality_reports
           UNION SELECT reference_artifact_id FROM geometry_quality_reports
             WHERE reference_artifact_id IS NOT NULL
           UNION SELECT value FROM annotations a, json_each(a.annotation_json, '$.provenance.artifact_ids')
             WHERE a.run_id = ?1
         )",
        [&object.id],
    )?;
    for sql in [
        "DELETE FROM active_project_runs WHERE run_id = ?1",
        "DELETE FROM run_start_requests WHERE run_id = ?1",
        "DELETE FROM run_images WHERE run_id = ?1",
        "DELETE FROM task_runs WHERE run_id = ?1",
        "DELETE FROM run_steps WHERE run_id = ?1",
        "DELETE FROM run_events WHERE run_id = ?1",
        "DELETE FROM model_calls WHERE run_id = ?1",
        "DELETE FROM model_messages WHERE run_id = ?1",
        "DELETE FROM tool_calls WHERE run_id = ?1",
        "DELETE FROM validation_issues WHERE run_id = ?1",
        "DELETE FROM usage_records WHERE run_id = ?1",
    ] {
        report.database_rows_removed += delete_count(transaction, sql, &object.id)?;
    }
    transaction.execute(
        "UPDATE batch_images SET child_run_id = NULL WHERE child_run_id = ?1",
        [&object.id],
    )?;
    ensure_one(
        transaction.execute(
            "DELETE FROM runs WHERE id = ?1 AND project_id = ?2 AND lifecycle_revision = ?3
               AND deleted_at IS NOT NULL",
            params![
                object.id,
                scope.stable_project_id.to_string(),
                to_i64(object.expected_revision)
            ],
        )?,
        "revision_conflict",
        "Run changed during permanent cleanup",
    )?;
    report.database_rows_removed += 1;
    Ok(())
}

fn delete_count(transaction: &Transaction<'_>, sql: &str, id: &str) -> Result<usize, StorageError> {
    transaction.execute(sql, [id]).map_err(StorageError::from)
}

fn push_reason(report: &mut PurgeReport, reason: &str) {
    if !report.retained_reasons.iter().any(|item| item == reason) {
        report.retained_reasons.push(reason.to_owned());
    }
}

fn change_archive_state(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
    operation_id: &str,
    now: DateTime<Utc>,
    archived: bool,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let now = now.to_rfc3339();
    let mut affected = Vec::new();
    for object in objects {
        let archived_value = archived.then_some(now.as_str());
        match object.kind {
            ManagementObjectKind::WorkflowDraft => {
                let draft = read_draft(transaction, &object.id)?;
                record_item(transaction, operation_id, object, &draft_state(&draft))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_drafts SET archived_at = ?2,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE id = ?1 AND project_id = ?3 AND lifecycle_revision = ?4",
                        params![
                            object.id,
                            archived_value,
                            scope.project_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline Draft changed during archive action",
                )?;
            }
            ManagementObjectKind::WorkflowVersion => {
                let number = object.version.expect("validated version");
                let version = read_version(transaction, &object.id, number)?;
                record_item(transaction, operation_id, object, &version_state(&version))?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_versions SET archived_at = ?3,
                           lifecycle_revision = lifecycle_revision + 1
                         WHERE workflow_id = ?1 AND version = ?2 AND project_id = ?4
                           AND lifecycle_revision = ?5",
                        params![
                            object.id,
                            number,
                            archived_value,
                            scope.project_id,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Published Version changed during archive action",
                )?;
            }
            ManagementObjectKind::Pipeline => {
                let pipeline = read_pipeline(transaction, &object.id)?;
                record_item(
                    transaction,
                    operation_id,
                    object,
                    &pipeline_state(&pipeline),
                )?;
                ensure_one(
                    transaction.execute(
                        "UPDATE workflow_pipelines SET archived_at = ?3,
                           lifecycle_revision = lifecycle_revision + 1, updated_at = ?4
                         WHERE workflow_id = ?1 AND project_id = ?2 AND lifecycle_revision = ?5",
                        params![
                            object.id,
                            scope.project_id,
                            archived_value,
                            now,
                            to_i64(object.expected_revision),
                        ],
                    )?,
                    "revision_conflict",
                    "Pipeline changed during archive action",
                )?;
            }
            ManagementObjectKind::Run | ManagementObjectKind::Batch => {
                unreachable!("preview blocks Run archive")
            }
        }
        affected.push(incremented(object));
    }
    affected.sort();
    Ok(affected)
}

fn rename_pipeline(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    object: &ManagementObjectRef,
    operation_id: &str,
    now: DateTime<Utc>,
    display_name: &str,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let pipeline = read_pipeline(transaction, &object.id)?;
    record_item(
        transaction,
        operation_id,
        object,
        &pipeline_state(&pipeline),
    )?;
    ensure_one(
        transaction.execute(
            "UPDATE workflow_pipelines SET display_name = ?3,
               lifecycle_revision = lifecycle_revision + 1, updated_at = ?4
             WHERE workflow_id = ?1 AND project_id = ?2 AND lifecycle_revision = ?5
               AND deleted_at IS NULL",
            params![
                object.id,
                scope.project_id,
                display_name.trim(),
                now.to_rfc3339(),
                to_i64(object.expected_revision),
            ],
        )?,
        "revision_conflict",
        "Pipeline changed during rename",
    )?;
    Ok(vec![incremented(object)])
}

fn record_default_action(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    objects: &[ManagementObjectRef],
    operation_id: &str,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    for object in objects {
        record_item(
            transaction,
            operation_id,
            object,
            &serde_json::json!({"default": read_default(transaction, &scope.project_id)?}),
        )?;
    }
    Ok(objects.to_vec())
}

fn apply_default_transition(
    transaction: &Transaction<'_>,
    scope: &ManagementScope,
    request: &ManagementRequest,
    objects: &[ManagementObjectRef],
    now: &DateTime<Utc>,
) -> Result<(), StorageError> {
    if request.action == ManagementAction::SetDefault {
        let object = objects.first().expect("validated objects");
        let version = object.version.expect("Set Default requires a Version");
        transaction.execute(
            "INSERT INTO project_workflow_defaults (project_id, workflow_id, version, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET workflow_id = excluded.workflow_id,
               version = excluded.version, updated_at = excluded.updated_at",
            params![scope.project_id, object.id, version, now.to_rfc3339()],
        )?;
        return Ok(());
    }
    if request.action == ManagementAction::ClearDefault {
        transaction.execute(
            "DELETE FROM project_workflow_defaults WHERE project_id = ?1",
            [&scope.project_id],
        )?;
        return Ok(());
    }
    let Some(current) = read_default(transaction, &scope.project_id)? else {
        return Ok(());
    };
    let targets_default = objects.iter().any(|object| match object.kind {
        ManagementObjectKind::WorkflowVersion => {
            object.id == current.workflow_id && object.version == Some(current.version)
        }
        ManagementObjectKind::Pipeline => object.id == current.workflow_id,
        _ => false,
    });
    if !targets_default {
        return Ok(());
    }
    if request.clear_default {
        transaction.execute(
            "DELETE FROM project_workflow_defaults WHERE project_id = ?1",
            [&scope.project_id],
        )?;
    } else if let Some(replacement) = &request.replacement_default_version {
        transaction.execute(
            "UPDATE project_workflow_defaults SET workflow_id = ?2, version = ?3, updated_at = ?4
             WHERE project_id = ?1",
            params![
                scope.project_id,
                replacement.workflow_id,
                replacement.version,
                now.to_rfc3339(),
            ],
        )?;
    }
    Ok(())
}

fn pipeline_children(
    connection: &Connection,
    scope: &ManagementScope,
    workflow_id: &str,
    deleted: bool,
    operation_id: Option<&str>,
) -> Result<Vec<ManagementObjectRef>, StorageError> {
    let mut children = Vec::new();
    if let Ok(draft) = read_draft(connection, workflow_id)
        && draft.project_id == scope.project_id
        && draft.deleted_at.is_some() == deleted
        && operation_id
            .is_none_or(|operation| draft.deletion_operation_id.as_deref() == Some(operation))
    {
        children.push(ManagementObjectRef {
            kind: ManagementObjectKind::WorkflowDraft,
            id: draft.id,
            version: None,
            expected_revision: draft.lifecycle_revision,
        });
    }
    let mut statement = connection.prepare(
        "SELECT version FROM workflow_versions WHERE workflow_id = ?1 AND project_id = ?2",
    )?;
    let versions = statement
        .query_map(params![workflow_id, scope.project_id], |row| {
            row.get::<_, u32>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for number in versions {
        let version = read_version(connection, workflow_id, number)?;
        if version.deleted_at.is_some() == deleted
            && operation_id
                .is_none_or(|operation| version.deletion_operation_id.as_deref() == Some(operation))
        {
            children.push(ManagementObjectRef {
                kind: ManagementObjectKind::WorkflowVersion,
                id: workflow_id.to_owned(),
                version: Some(number),
                expected_revision: version.lifecycle_revision,
            });
        }
    }
    Ok(children)
}

fn record_and_set_child_deleted(
    transaction: &Transaction<'_>,
    operation_id: &str,
    object: &ManagementObjectRef,
    deleted_at: &str,
    deletion_operation_id: Option<&str>,
) -> Result<(), StorageError> {
    match object.kind {
        ManagementObjectKind::WorkflowDraft => {
            let draft = read_draft(transaction, &object.id)?;
            record_item(transaction, operation_id, object, &draft_state(&draft))?;
            transaction.execute(
                "UPDATE workflow_drafts SET deleted_at = ?2, deletion_operation_id = ?3,
                   lifecycle_revision = lifecycle_revision + 1 WHERE id = ?1",
                params![
                    object.id,
                    (!deleted_at.is_empty()).then_some(deleted_at),
                    deletion_operation_id,
                ],
            )?;
        }
        ManagementObjectKind::WorkflowVersion => {
            let number = object.version.expect("version child");
            let version = read_version(transaction, &object.id, number)?;
            record_item(transaction, operation_id, object, &version_state(&version))?;
            transaction.execute(
                "UPDATE workflow_versions SET deleted_at = ?3, deletion_operation_id = ?4,
                   lifecycle_revision = lifecycle_revision + 1
                 WHERE workflow_id = ?1 AND version = ?2",
                params![
                    object.id,
                    number,
                    (!deleted_at.is_empty()).then_some(deleted_at),
                    deletion_operation_id,
                ],
            )?;
        }
        _ => unreachable!("Pipeline child kind"),
    }
    Ok(())
}

fn incremented(object: &ManagementObjectRef) -> ManagementObjectRef {
    ManagementObjectRef {
        expected_revision: object.expected_revision.saturating_add(1),
        ..object.clone()
    }
}

fn read_run(connection: &Connection, id: &str) -> Result<RunLifecycle, StorageError> {
    connection
        .query_row(
            "SELECT id, project_id, status, lifecycle_revision,
                    deleted_at, deletion_operation_id
             FROM runs WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| management_error("entity_purged", format!("Run {id} was not found")))
        .and_then(
            |(id, project_id, status, revision, deleted_at, operation)| {
                Ok(RunLifecycle {
                    id,
                    project_id,
                    status: serde_json::from_value(serde_json::Value::String(status))?,
                    revision: to_u64(revision),
                    deleted_at,
                    deletion_operation_id: operation,
                })
            },
        )
}

fn read_batch(connection: &Connection, id: &str) -> Result<BatchLifecycle, StorageError> {
    connection
        .query_row(
            "SELECT id, project_id, status, lifecycle_revision, lease_owner,
                    lease_expires_at, deleted_at, deletion_operation_id
             FROM dataset_batches WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| management_error("entity_purged", format!("Dataset Run {id} was not found")))
        .and_then(
            |(
                id,
                project_id,
                status,
                revision,
                lease_owner,
                lease_expires_at,
                deleted_at,
                operation,
            )| {
                Ok(BatchLifecycle {
                    id,
                    project_id,
                    status: serde_json::from_value(serde_json::Value::String(status))?,
                    revision: to_u64(revision),
                    lease_owner,
                    lease_expires_at,
                    deleted_at,
                    deletion_operation_id: operation,
                })
            },
        )
}

fn read_draft(connection: &Connection, id: &str) -> Result<DraftLifecycle, StorageError> {
    connection
        .query_row(
            "SELECT id, project_id, COALESCE(json_extract(draft_json, '$.name'), id),
                    revision, lifecycle_revision, content_hash, archived_at, deleted_at,
                    deletion_operation_id
             FROM workflow_drafts WHERE id = ?1",
            [id],
            |row| {
                Ok(DraftLifecycle {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    name: row.get(2)?,
                    authoring_revision: to_u64(row.get(3)?),
                    lifecycle_revision: to_u64(row.get(4)?),
                    content_hash: row.get(5)?,
                    archived_at: row.get(6)?,
                    deleted_at: row.get(7)?,
                    deletion_operation_id: row.get(8)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            management_error(
                "entity_purged",
                format!("Pipeline Draft {id} was not found"),
            )
        })
}

fn read_version(
    connection: &Connection,
    workflow_id: &str,
    version: u32,
) -> Result<VersionLifecycle, StorageError> {
    connection
        .query_row(
            "SELECT workflow_id, version, project_id,
                    COALESCE(display_name, json_extract(version_json, '$.draft.name'), workflow_id),
                    lifecycle_revision, json_extract(version_json, '$.content_hash'), archived_at,
                    deleted_at, deletion_operation_id
             FROM workflow_versions WHERE workflow_id = ?1 AND version = ?2",
            params![workflow_id, version],
            |row| {
                Ok(VersionLifecycle {
                    workflow_id: row.get(0)?,
                    version: u32::try_from(row.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                    project_id: row.get(2)?,
                    name: row.get(3)?,
                    lifecycle_revision: to_u64(row.get(4)?),
                    content_hash: row.get(5)?,
                    archived_at: row.get(6)?,
                    deleted_at: row.get(7)?,
                    deletion_operation_id: row.get(8)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            management_error(
                "entity_purged",
                format!("Published Version {workflow_id}@{version} was not found"),
            )
        })
}

fn read_pipeline(
    connection: &Connection,
    workflow_id: &str,
) -> Result<PipelineLifecycle, StorageError> {
    connection
        .query_row(
            "SELECT workflow_id, project_id, display_name, lifecycle_revision, archived_at,
                    deleted_at, deletion_operation_id
             FROM workflow_pipelines WHERE workflow_id = ?1",
            [workflow_id],
            |row| {
                Ok(PipelineLifecycle {
                    workflow_id: row.get(0)?,
                    project_id: row.get(1)?,
                    name: row.get(2)?,
                    lifecycle_revision: to_u64(row.get(3)?),
                    archived_at: row.get(4)?,
                    deleted_at: row.get(5)?,
                    deletion_operation_id: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            management_error(
                "entity_purged",
                format!("Pipeline {workflow_id} was not found"),
            )
        })
}

fn read_default(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<WorkflowVersionRef>, StorageError> {
    connection
        .query_row(
            "SELECT workflow_id, version FROM project_workflow_defaults WHERE project_id = ?1",
            [project_id],
            |row| {
                Ok(WorkflowVersionRef {
                    workflow_id: row.get(0)?,
                    version: u32::try_from(row.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

fn draft_has_active_writer(
    connection: &Connection,
    scope: &ManagementScope,
    draft_id: &str,
) -> Result<bool, StorageError> {
    let session = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM agent_sessions
           WHERE project_id = ?1 AND status = 'running'
             AND (
               json_extract(session_json, '$.draft_id') = ?2
               OR json_extract(session_json, '$.working_draft.draft_id') = ?2
             )
         )",
        params![scope.project_id, draft_id],
        |row| row.get::<_, bool>(0),
    )?;
    let lease = has_management_lease(
        connection,
        scope,
        ManagementObjectKind::WorkflowDraft,
        draft_id,
        0,
    )?;
    Ok(session || lease)
}

fn has_management_lease(
    connection: &Connection,
    scope: &ManagementScope,
    kind: ManagementObjectKind,
    id: &str,
    version: u32,
) -> Result<bool, StorageError> {
    let now = Utc::now().to_rfc3339();
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM management_entity_leases
               WHERE project_id = ?1 AND object_kind = ?2 AND object_id = ?3
                 AND object_version = ?4 AND expires_at > ?5
             )",
            params![scope.project_id, enum_json(kind)?, id, version, now],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StorageError::from)
}

fn version_has_active_execution(
    connection: &Connection,
    scope: &ManagementScope,
    workflow_id: &str,
    version: u32,
) -> Result<bool, StorageError> {
    let version_i64 = i64::from(version);
    let active_run = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM runs r
           LEFT JOIN active_project_runs a ON a.run_id = r.id
           WHERE r.project_id = ?1
             AND (a.run_id IS NOT NULL OR r.status IN ('pending', 'running', 'paused', 'awaiting_review'))
             AND json_extract(r.workflow_snapshot_json, '$.selected_workflow.workflow_id') = ?2
             AND json_extract(r.workflow_snapshot_json, '$.selected_workflow.version') = ?3
         )",
        params![scope.stable_project_id.to_string(), workflow_id, version_i64],
        |row| row.get::<_, bool>(0),
    )?;
    let batch_identity = format!("{workflow_id}@{version}");
    let active_batch = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM dataset_batches
           WHERE project_id = ?1 AND workflow_version = ?2
             AND (status IN ('pending', 'running', 'paused', 'awaiting_review')
                  OR (lease_owner IS NOT NULL AND lease_expires_at > ?3))
         )",
        params![scope.project_id, batch_identity, Utc::now().to_rfc3339()],
        |row| row.get::<_, bool>(0),
    )?;
    Ok(active_run
        || active_batch
        || has_management_lease(
            connection,
            scope,
            ManagementObjectKind::WorkflowVersion,
            workflow_id,
            version,
        )?)
}

fn pipeline_has_active_execution(
    connection: &Connection,
    scope: &ManagementScope,
    workflow_id: &str,
) -> Result<bool, StorageError> {
    let mut statement =
        connection.prepare("SELECT version FROM workflow_versions WHERE workflow_id = ?1")?;
    let versions = statement
        .query_map([workflow_id], |row| row.get::<_, u32>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for version in versions {
        if version_has_active_execution(connection, scope, workflow_id, version)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn historical_run_references(
    connection: &Connection,
    scope: &ManagementScope,
    workflow_id: &str,
    version: Option<u32>,
) -> Result<usize, StorageError> {
    let value = connection.query_row(
        "SELECT COUNT(*) FROM runs
         WHERE project_id = ?1
           AND json_extract(workflow_snapshot_json, '$.selected_workflow.workflow_id') = ?2
           AND (?3 IS NULL OR json_extract(workflow_snapshot_json, '$.selected_workflow.version') = ?3)",
        params![scope.stable_project_id.to_string(), workflow_id, version.map(i64::from)],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(usize::try_from(value.max(0)).unwrap_or(usize::MAX))
}

fn pipeline_summary(
    connection: &Connection,
    scope: &ManagementScope,
    workflow_id: &str,
    include_archived: bool,
    include_deleted: bool,
) -> Result<PipelineLifecycleSummary, StorageError> {
    let pipeline = read_pipeline(connection, workflow_id)?;
    let default = read_default(connection, &scope.project_id)?;
    let mut draft_statement =
        connection.prepare("SELECT id FROM workflow_drafts WHERE id = ?1 AND project_id = ?2")?;
    let draft_ids = draft_statement
        .query_map(params![workflow_id, scope.project_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let drafts = draft_ids
        .into_iter()
        .map(|id| {
            let draft = read_draft(connection, &id)?;
            if (!include_archived && draft.archived_at.is_some())
                || (!include_deleted && draft.deleted_at.is_some())
            {
                return Ok(None);
            }
            Ok(Some(WorkflowLifecycleItem {
                object: ManagementObjectRef {
                    kind: ManagementObjectKind::WorkflowDraft,
                    id: draft.id.clone(),
                    version: None,
                    expected_revision: draft.lifecycle_revision,
                },
                display_name: draft.name,
                content_hash: draft.content_hash,
                archived_at: optional_datetime(draft.archived_at.as_deref())?,
                deleted_at: optional_datetime(draft.deleted_at.as_deref())?,
                deletion_operation_id: draft.deletion_operation_id,
                is_default: false,
                historical_run_references: 0,
            }))
        })
        .collect::<Result<Vec<_>, StorageError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let mut version_statement = connection.prepare(
        "SELECT version FROM workflow_versions WHERE workflow_id = ?1 AND project_id = ?2
         ORDER BY version DESC",
    )?;
    let version_numbers = version_statement
        .query_map(params![workflow_id, scope.project_id], |row| {
            row.get::<_, u32>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let versions = version_numbers
        .into_iter()
        .map(|number| {
            let version = read_version(connection, workflow_id, number)?;
            if (!include_archived && version.archived_at.is_some())
                || (!include_deleted && version.deleted_at.is_some())
            {
                return Ok(None);
            }
            Ok(Some(WorkflowLifecycleItem {
                object: ManagementObjectRef {
                    kind: ManagementObjectKind::WorkflowVersion,
                    id: version.workflow_id.clone(),
                    version: Some(version.version),
                    expected_revision: version.lifecycle_revision,
                },
                display_name: version.name,
                content_hash: version.content_hash,
                archived_at: optional_datetime(version.archived_at.as_deref())?,
                deleted_at: optional_datetime(version.deleted_at.as_deref())?,
                deletion_operation_id: version.deletion_operation_id,
                is_default: default.as_ref()
                    == Some(&WorkflowVersionRef {
                        workflow_id: version.workflow_id.clone(),
                        version: version.version,
                    }),
                historical_run_references: historical_run_references(
                    connection,
                    scope,
                    workflow_id,
                    Some(number),
                )?,
            }))
        })
        .collect::<Result<Vec<_>, StorageError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(PipelineLifecycleSummary {
        project_id: pipeline.project_id,
        workflow_id: pipeline.workflow_id,
        display_name: pipeline.name,
        lifecycle_revision: pipeline.lifecycle_revision,
        archived_at: optional_datetime(pipeline.archived_at.as_deref())?,
        deleted_at: optional_datetime(pipeline.deleted_at.as_deref())?,
        deletion_operation_id: pipeline.deletion_operation_id,
        default_version: default
            .filter(|candidate| candidate.workflow_id == workflow_id)
            .map(|candidate| candidate.version),
        drafts,
        versions,
    })
}

fn optional_datetime(value: Option<&str>) -> Result<Option<DateTime<Utc>>, StorageError> {
    value.map(parse_datetime).transpose()
}

fn batch_child_runs(connection: &Connection, batch_id: &str) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT child_run_id FROM batch_images
         WHERE batch_id = ?1 AND child_run_id IS NOT NULL ORDER BY position",
    )?;
    statement
        .query_map([batch_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn batch_impacted_runs(
    connection: &Connection,
    batch: &BatchLifecycle,
    action: ManagementAction,
) -> Result<Vec<String>, StorageError> {
    let mut runs = Vec::new();
    for child_id in batch_child_runs(connection, &batch.id)? {
        let child = read_run(connection, &child_id)?;
        let included = match action {
            ManagementAction::MoveToTrash => child.deleted_at.is_none(),
            ManagementAction::Restore | ManagementAction::Purge => {
                batch.deletion_operation_id.is_some()
                    && child.deletion_operation_id == batch.deletion_operation_id
            }
            _ => false,
        };
        if included {
            runs.push(child_id);
        }
    }
    Ok(runs)
}

fn record_item(
    transaction: &Transaction<'_>,
    operation_id: &str,
    object: &ManagementObjectRef,
    previous_state: &serde_json::Value,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO management_operation_items
         (operation_id, object_kind, object_id, object_version, previous_state_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            operation_id,
            enum_json(object.kind)?,
            object.id,
            i64::from(object.version.unwrap_or(0)),
            serde_json::to_string(previous_state)?,
        ],
    )?;
    Ok(())
}

fn run_state(run: &RunLifecycle) -> serde_json::Value {
    serde_json::json!({
        "status": run.status,
        "revision": run.revision,
        "deleted_at": run.deleted_at,
        "deletion_operation_id": run.deletion_operation_id,
    })
}

fn batch_state(batch: &BatchLifecycle) -> serde_json::Value {
    serde_json::json!({
        "status": batch.status,
        "revision": batch.revision,
        "lease_owner": batch.lease_owner,
        "deleted_at": batch.deleted_at,
        "deletion_operation_id": batch.deletion_operation_id,
    })
}

fn draft_state(draft: &DraftLifecycle) -> serde_json::Value {
    serde_json::json!({
        "authoring_revision": draft.authoring_revision,
        "lifecycle_revision": draft.lifecycle_revision,
        "content_hash": draft.content_hash,
        "archived_at": draft.archived_at,
        "deleted_at": draft.deleted_at,
        "deletion_operation_id": draft.deletion_operation_id,
    })
}

fn version_state(version: &VersionLifecycle) -> serde_json::Value {
    serde_json::json!({
        "lifecycle_revision": version.lifecycle_revision,
        "content_hash": version.content_hash,
        "archived_at": version.archived_at,
        "deleted_at": version.deleted_at,
        "deletion_operation_id": version.deletion_operation_id,
    })
}

fn pipeline_state(pipeline: &PipelineLifecycle) -> serde_json::Value {
    serde_json::json!({
        "display_name": pipeline.name,
        "lifecycle_revision": pipeline.lifecycle_revision,
        "archived_at": pipeline.archived_at,
        "deleted_at": pipeline.deleted_at,
        "deletion_operation_id": pipeline.deletion_operation_id,
    })
}

fn is_active_run_status(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Pending | RunStatus::Running | RunStatus::Paused | RunStatus::AwaitingReview
    )
}

fn blocker(code: &str, object: &ManagementObjectRef, message: &str) -> ManagementBlocker {
    ManagementBlocker {
        code: code.to_owned(),
        object: object.clone(),
        message: message.to_owned(),
        related_ids: Vec::new(),
    }
}

fn summary(
    action: ManagementAction,
    objects: &[ManagementObjectRef],
    impact: &ManagementImpact,
    can_execute: bool,
) -> String {
    if !can_execute {
        return format!(
            "{} selected item(s) cannot be changed until blockers are resolved.",
            objects.len()
        );
    }
    match action {
        ManagementAction::MoveToTrash => format!(
            "Move {} item(s) and {} owned child Run(s) to Trash. {} confirmed annotation(s) remain. No disk space is freed.",
            objects.len(),
            impact.child_runs,
            impact.confirmed_annotations_retained
        ),
        ManagementAction::Restore => format!(
            "Restore {} item(s) with their original identities. No Run will restart.",
            objects.len()
        ),
        _ => format!("Apply lifecycle action to {} item(s).", objects.len()),
    }
}

fn count(connection: &Connection, sql: &str, id: &str) -> Result<usize, StorageError> {
    let value = connection.query_row(sql, [id], |row| row.get::<_, i64>(0))?;
    Ok(usize::try_from(value.max(0)).unwrap_or(usize::MAX))
}

fn canonical_request_json(request: &ManagementRequest) -> Result<String, StorageError> {
    let mut canonical = request.clone();
    canonical.confirmation_token = None;
    serde_json::to_string(&canonical).map_err(StorageError::from)
}

fn enum_json(value: impl serde::Serialize) -> Result<String, StorageError> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .unwrap_or("unknown")
        .to_owned())
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| StorageError::InvalidEnum(error.to_string()))
}

fn to_u64(value: i64) -> u64 {
    u64::try_from(value.max(1)).unwrap_or(u64::MAX)
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

fn trash_entry(
    scope: &ManagementScope,
    object: ManagementObjectRef,
    display_name: String,
    deleted_at: &str,
    deletion_operation_id: String,
) -> Result<TrashEntry, StorageError> {
    Ok(TrashEntry {
        project_id: scope.project_id.clone(),
        object,
        display_name,
        deleted_at: parse_datetime(deleted_at)?,
        deletion_operation_id,
        source_project: scope.project_id.clone(),
        recoverable: true,
    })
}

fn ensure_one(changed: usize, code: &str, message: &str) -> Result<(), StorageError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(management_error(code, message))
    }
}

fn management_error(code: &str, message: impl Into<String>) -> StorageError {
    StorageError::Management {
        code: code.to_owned(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{
        AgentBudget, AgentKind, AgentSession, Annotation, AnnotationId, AnnotationProvenance,
        AnnotationSource, AnnotationValue, BatchBudgetLedger, BatchBudgetLimits, BatchId,
        BatchRecord, ImageId, LabelId, ReviewStatus, RunId, TaskId, WorkflowDraft,
        WorkflowDraftNode, WorkflowDraftStatus, WorkflowNodeKind, WorkflowSnapshot,
    };
    use annotagent_runtime::{RunRecord, RuntimeStore};
    use std::collections::BTreeMap;

    fn test_scope() -> ManagementScope {
        ManagementScope {
            project_id: "project".to_owned(),
            stable_project_id: ProjectId::new(),
        }
    }

    fn request(
        project_id: &str,
        object: ManagementObjectRef,
        action: ManagementAction,
        key: &str,
    ) -> ManagementRequest {
        ManagementRequest {
            project_id: project_id.to_owned(),
            objects: vec![object],
            action,
            replacement_default_version: None,
            clear_default: false,
            display_name: None,
            idempotency_key: key.to_owned(),
            confirmation_token: None,
        }
    }

    async fn insert_run(store: &SqliteStore, scope: &ManagementScope, status: RunStatus) -> RunId {
        let id = RunId::new();
        RuntimeStore::create_run(
            store,
            &RunRecord {
                id,
                project_id: scope.stable_project_id,
                project_name: "Project".to_owned(),
                skill_id: "test".to_owned(),
                provider: "test".to_owned(),
                model: "test".to_owned(),
                status,
                project_schema_json: "{}".to_owned(),
                workflow_snapshot_json: None,
            },
        )
        .await
        .expect("insert Run");
        if status != RunStatus::Pending {
            RuntimeStore::set_run_status(store, id, status, None)
                .await
                .expect("set status");
        }
        id
    }

    #[tokio::test]
    async fn terminal_run_moves_to_trash_restores_and_is_idempotent() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let id = insert_run(&store, &scope, RunStatus::Completed).await;
        let object = ManagementObjectRef {
            kind: ManagementObjectKind::Run,
            id: id.to_string(),
            version: None,
            expected_revision: 1,
        };
        let mut delete = request(
            &scope.project_id,
            object,
            ManagementAction::MoveToTrash,
            "delete",
        );
        let preview = store
            .preview_management(&scope, &delete)
            .expect("preview delete");
        assert!(preview.can_execute);
        delete.confirmation_token = Some(preview.confirmation_token);
        let first = store
            .execute_management(&scope, &delete)
            .expect("delete Run");
        let repeated = store
            .execute_management(&scope, &delete)
            .expect("idempotent delete");
        assert_eq!(first.operation_id, repeated.operation_id);
        assert!(store.list_runs().expect("visible Runs").is_empty());

        let trash = store
            .list_project_trash(&scope, Some(ManagementObjectKind::Run))
            .expect("Trash");
        assert_eq!(trash.len(), 1);
        let mut restore = request(
            &scope.project_id,
            trash[0].object.clone(),
            ManagementAction::Restore,
            "restore",
        );
        let preview = store
            .preview_management(&scope, &restore)
            .expect("preview restore");
        restore.confirmation_token = Some(preview.confirmation_token);
        store
            .execute_management(&scope, &restore)
            .expect("restore Run");
        assert_eq!(store.list_runs().expect("visible Runs").len(), 1);
    }

    #[tokio::test]
    async fn permanent_run_cleanup_retains_annotations_provenance_and_usage_ledger() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let id = insert_run(&store, &scope, RunStatus::Completed).await;
        let accepted = Annotation {
            id: AnnotationId::new(),
            image_id: ImageId::new(),
            task_id: TaskId::from("objects"),
            label: Some(LabelId::from("ball")),
            value: AnnotationValue::Classification {
                labels: vec![LabelId::from("ball")],
            },
            attributes: BTreeMap::new(),
            confidence: Some(0.9),
            source: AnnotationSource::Model,
            review_status: ReviewStatus::HumanAccepted,
            provenance: AnnotationProvenance::default(),
            created_at: Utc::now(),
        };
        let unresolved = Annotation {
            id: AnnotationId::new(),
            review_status: ReviewStatus::NeedsReview,
            ..accepted.clone()
        };
        RuntimeStore::commit_annotation(&store, id, &accepted)
            .await
            .expect("accepted annotation");
        RuntimeStore::commit_annotation(&store, id, &unresolved)
            .await
            .expect("unresolved annotation");
        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO usage_records
                     (run_id, usage_json, input_tokens, output_tokens, total_tokens, cost, created_at)
                     VALUES (?1, '{}', 10, 4, 14, '0.125', ?2)",
                    params![id.to_string(), Utc::now().to_rfc3339()],
                )?;
                connection.execute(
                    "INSERT INTO run_steps (id, run_id, step_index, summary, created_at)
                     VALUES ('debug-step', ?1, 0, 'debug', ?2)",
                    params![id.to_string(), Utc::now().to_rfc3339()],
                )?;
                Ok(())
            })
            .expect("fixture detail");
        let object = ManagementObjectRef {
            kind: ManagementObjectKind::Run,
            id: id.to_string(),
            version: None,
            expected_revision: 1,
        };
        let mut delete = request(
            &scope.project_id,
            object,
            ManagementAction::MoveToTrash,
            "delete-before-purge",
        );
        let preview = store
            .preview_management(&scope, &delete)
            .expect("delete preview");
        delete.confirmation_token = Some(preview.confirmation_token);
        let deleted = store
            .execute_management(&scope, &delete)
            .expect("soft delete");
        let trashed = deleted
            .affected_objects
            .into_iter()
            .find(|object| object.kind == ManagementObjectKind::Run)
            .expect("trashed Run");
        let mut purge = request(
            &scope.project_id,
            trashed,
            ManagementAction::Purge,
            "purge-run",
        );
        let preview = store
            .preview_management(&scope, &purge)
            .expect("purge preview");
        assert!(preview.can_execute);
        purge.confirmation_token = Some(preview.confirmation_token);
        let receipt = store.execute_management(&scope, &purge).expect("purge Run");
        let report = receipt.purge.expect("cleanup report");
        assert_eq!(report.retained_annotation_records, 1);
        assert_eq!(report.retained_usage_records, 1);
        assert_eq!(
            store.list_annotations(id).expect("retained annotations"),
            vec![accepted.clone()]
        );
        assert_eq!(
            store
                .list_project_annotations_for_run(scope.stable_project_id, id)
                .expect("retained Project annotations"),
            vec![accepted]
        );
        let provenance = store
            .run_provenance_summary(&id.to_string())
            .expect("provenance")
            .expect("provenance tombstone");
        assert!(provenance.source_deleted);
        store
            .with_connection(|connection| {
                assert_eq!(
                    connection.query_row(
                        "SELECT COUNT(*) FROM runs WHERE id = ?1",
                        [id.to_string()],
                        |row| row.get::<_, i64>(0),
                    )?,
                    0
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT total_tokens FROM historical_usage_ledger WHERE run_id = ?1",
                        [id.to_string()],
                        |row| row.get::<_, i64>(0),
                    )?,
                    14
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT COUNT(*) FROM run_steps WHERE run_id = ?1",
                        [id.to_string()],
                        |row| row.get::<_, i64>(0),
                    )?,
                    0
                );
                Ok(())
            })
            .expect("cleanup assertions");
    }

    #[tokio::test]
    async fn active_and_foreign_runs_are_blocked_atomically() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let active = insert_run(&store, &scope, RunStatus::Running).await;
        let foreign_scope = test_scope();
        let foreign = insert_run(&store, &foreign_scope, RunStatus::Completed).await;
        for (id, expected_code) in [(active, "run_active"), (foreign, "foreign_project_object")] {
            let request = request(
                &scope.project_id,
                ManagementObjectRef {
                    kind: ManagementObjectKind::Run,
                    id: id.to_string(),
                    version: None,
                    expected_revision: 1,
                },
                ManagementAction::MoveToTrash,
                &format!("delete-{id}"),
            );
            let preview = store.preview_management(&scope, &request).expect("preview");
            assert!(!preview.can_execute);
            assert!(
                preview
                    .blockers
                    .iter()
                    .any(|blocker| blocker.code == expected_code)
            );
        }
    }

    #[tokio::test]
    async fn cancel_and_delete_waits_for_terminal_state_before_soft_deletion() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let run_id = insert_run(&store, &scope, RunStatus::Running).await;
        let mut cancel = request(
            &scope.project_id,
            ManagementObjectRef {
                kind: ManagementObjectKind::Run,
                id: run_id.to_string(),
                version: None,
                expected_revision: 1,
            },
            ManagementAction::CancelAndDelete,
            "cancel-delete",
        );
        let preview = store.preview_management(&scope, &cancel).expect("preview");
        assert!(preview.can_execute);
        cancel.confirmation_token = Some(preview.confirmation_token);
        let waiting = store
            .prepare_cancel_and_delete(&scope, &cancel)
            .expect("persist cancellation intent");
        assert_eq!(
            waiting.status,
            ManagementOperationStatus::WaitingForCancellation
        );
        assert!(matches!(
            store.complete_cancel_and_delete(&scope, &cancel),
            Err(StorageError::Management { ref code, .. }) if code == "cancellation_not_completed"
        ));
        assert_eq!(store.list_runs().expect("Run remains").len(), 1);
        RuntimeStore::set_run_status(&store, run_id, RunStatus::Cancelled, None)
            .await
            .expect("worker terminated");
        let completed = store
            .complete_cancel_and_delete(&scope, &cancel)
            .expect("delete after terminal state");
        assert_eq!(completed.status, ManagementOperationStatus::Completed);
        assert!(store.list_runs().expect("hidden Run").is_empty());
        assert_eq!(
            store
                .get_management_operation(&scope, &waiting.operation_id)
                .expect("durable operation")
                .status,
            ManagementOperationStatus::Completed
        );
    }

    #[tokio::test]
    async fn batch_restore_only_revives_children_deleted_by_the_batch_operation() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let first = insert_run(&store, &scope, RunStatus::Completed).await;
        let second = insert_run(&store, &scope, RunStatus::Completed).await;
        let now = Utc::now();
        let batch_id = BatchId::new();
        store
            .create_batch(
                BatchRecord {
                    id: batch_id,
                    project_id: scope.project_id.clone(),
                    project_path: "projects/project/project.yaml".to_owned(),
                    provider: "test".to_owned(),
                    status: BatchStatus::Pending,
                    max_concurrency: 1,
                    workflow_version: "workflow@1".to_owned(),
                    workflow_snapshot: serde_json::json!({}),
                    project_snapshot: serde_json::json!({}),
                    budget_limits: BatchBudgetLimits::default(),
                    budget_ledger: BatchBudgetLedger::default(),
                    lease_owner: None,
                    lease_expires_at: None,
                    event_sequence: 0,
                    created_at: now,
                    updated_at: now,
                },
                &[
                    (ImageId::new(), "first.png".to_owned()),
                    (ImageId::new(), "second.png".to_owned()),
                ],
            )
            .expect("Batch");
        store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE batch_images SET child_run_id = CASE position
                       WHEN 0 THEN ?2 ELSE ?3 END WHERE batch_id = ?1",
                    params![batch_id.to_string(), first.to_string(), second.to_string()],
                )?;
                Ok(())
            })
            .expect("attach child Runs");
        store
            .set_batch_status(batch_id, BatchStatus::Completed, now)
            .expect("complete Batch");

        let mut delete_first = request(
            &scope.project_id,
            ManagementObjectRef {
                kind: ManagementObjectKind::Run,
                id: first.to_string(),
                version: None,
                expected_revision: 1,
            },
            ManagementAction::MoveToTrash,
            "delete-first",
        );
        let preview = store
            .preview_management(&scope, &delete_first)
            .expect("preview first");
        delete_first.confirmation_token = Some(preview.confirmation_token);
        store
            .execute_management(&scope, &delete_first)
            .expect("delete first");

        let mut delete_batch = request(
            &scope.project_id,
            ManagementObjectRef {
                kind: ManagementObjectKind::Batch,
                id: batch_id.to_string(),
                version: None,
                expected_revision: 1,
            },
            ManagementAction::MoveToTrash,
            "delete-batch",
        );
        let preview = store
            .preview_management(&scope, &delete_batch)
            .expect("preview Batch");
        assert_eq!(preview.impact.child_runs, 1);
        delete_batch.confirmation_token = Some(preview.confirmation_token);
        store
            .execute_management(&scope, &delete_batch)
            .expect("delete Batch");

        let trashed_batch = store
            .list_project_trash(&scope, Some(ManagementObjectKind::Batch))
            .expect("Trash")
            .pop()
            .expect("trashed Batch");
        let mut restore_batch = request(
            &scope.project_id,
            trashed_batch.object,
            ManagementAction::Restore,
            "restore-batch",
        );
        let preview = store
            .preview_management(&scope, &restore_batch)
            .expect("preview restore");
        assert_eq!(preview.impact.child_runs, 1);
        restore_batch.confirmation_token = Some(preview.confirmation_token);
        store
            .execute_management(&scope, &restore_batch)
            .expect("restore Batch");

        let trash = store
            .list_project_trash(&scope, Some(ManagementObjectKind::Run))
            .expect("Run Trash");
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].object.id, first.to_string());
        assert_eq!(store.list_runs().expect("visible Runs").len(), 1);
    }

    fn workflow_draft(id: &str, project_id: &str) -> WorkflowDraft {
        let now = Utc::now();
        WorkflowDraft {
            schema_version: 2,
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            name: format!("Pipeline {id}"),
            status: WorkflowDraftStatus::Editing,
            revision: 1,
            content_hash: String::new(),
            nodes: vec![WorkflowDraftNode {
                id: "commit".to_owned(),
                node_type: "commit".to_owned(),
                kind: WorkflowNodeKind::Commit,
                ..WorkflowDraftNode::default()
            }],
            edges: Vec::new(),
            enabled_skills: std::collections::BTreeMap::new(),
            resource_versions: std::collections::BTreeMap::new(),
            runtime_policies: std::collections::BTreeMap::new(),
            allow_unvalidated_commit: true,
            geometry_risk_acceptance: None,
            annotation_schema: None,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn confirmed(
        store: &SqliteStore,
        scope: &ManagementScope,
        mut request: ManagementRequest,
    ) -> ManagementReceipt {
        let preview = store
            .preview_management(scope, &request)
            .expect("management preview");
        assert!(preview.can_execute, "{:?}", preview.blockers);
        request.confirmation_token = Some(preview.confirmation_token);
        store
            .execute_management(scope, &request)
            .expect("management action")
    }

    #[test]
    fn pipeline_lifecycle_preserves_hash_defaults_and_exact_restore_membership() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        for id in ["pipeline-a", "pipeline-b"] {
            let draft = workflow_draft(id, &scope.project_id);
            store.save_workflow_draft(&draft).expect("Draft");
            store
                .publish_workflow_draft(
                    &draft,
                    format!("published-hash-{id}"),
                    WorkflowSnapshot {
                        schema_version: 2,
                        draft: Some(draft.clone()),
                        ..WorkflowSnapshot::default()
                    },
                )
                .expect("Version");
        }
        assert_eq!(
            store
                .project_workflow_default(&scope.project_id)
                .expect("default"),
            Some(WorkflowVersionRef {
                workflow_id: "pipeline-b".to_owned(),
                version: 1,
            })
        );

        let default_object = ManagementObjectRef {
            kind: ManagementObjectKind::WorkflowVersion,
            id: "pipeline-b".to_owned(),
            version: Some(1),
            expected_revision: 1,
        };
        let blocked = request(
            &scope.project_id,
            default_object.clone(),
            ManagementAction::MoveToTrash,
            "blocked-default",
        );
        let preview = store
            .preview_management(&scope, &blocked)
            .expect("preview default");
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| { blocker.code == "default_replacement_required" })
        );

        let mut delete_default = request(
            &scope.project_id,
            default_object,
            ManagementAction::MoveToTrash,
            "delete-default",
        );
        delete_default.clear_default = true;
        confirmed(&store, &scope, delete_default);
        assert_eq!(
            store
                .project_workflow_default(&scope.project_id)
                .expect("cleared default"),
            None
        );
        assert_eq!(
            store
                .get_published_workflow_version("pipeline-b", 1)
                .expect("historical Version")
                .content_hash,
            "published-hash-pipeline-b"
        );
        let trashed = store
            .list_project_trash(&scope, Some(ManagementObjectKind::WorkflowVersion))
            .expect("Version Trash");
        confirmed(
            &store,
            &scope,
            request(
                &scope.project_id,
                trashed
                    .iter()
                    .find(|item| item.object.id == "pipeline-b")
                    .expect("trashed B")
                    .object
                    .clone(),
                ManagementAction::Restore,
                "restore-version-b",
            ),
        );
        assert_eq!(
            store
                .project_workflow_default(&scope.project_id)
                .expect("default remains cleared"),
            None
        );

        confirmed(
            &store,
            &scope,
            request(
                &scope.project_id,
                ManagementObjectRef {
                    kind: ManagementObjectKind::WorkflowVersion,
                    id: "pipeline-a".to_owned(),
                    version: Some(1),
                    expected_revision: 1,
                },
                ManagementAction::MoveToTrash,
                "delete-version-a",
            ),
        );
        confirmed(
            &store,
            &scope,
            request(
                &scope.project_id,
                ManagementObjectRef {
                    kind: ManagementObjectKind::Pipeline,
                    id: "pipeline-a".to_owned(),
                    version: None,
                    expected_revision: 1,
                },
                ManagementAction::MoveToTrash,
                "delete-pipeline-a",
            ),
        );
        let pipeline_a = store
            .list_project_trash(&scope, Some(ManagementObjectKind::Pipeline))
            .expect("Pipeline Trash")
            .into_iter()
            .find(|item| item.object.id == "pipeline-a")
            .expect("trashed Pipeline A");
        confirmed(
            &store,
            &scope,
            request(
                &scope.project_id,
                pipeline_a.object,
                ManagementAction::Restore,
                "restore-pipeline-a",
            ),
        );
        let version_trash = store
            .list_project_trash(&scope, Some(ManagementObjectKind::WorkflowVersion))
            .expect("Version Trash");
        assert!(
            version_trash
                .iter()
                .any(|item| item.object.id == "pipeline-a")
        );

        let catalog = store
            .list_project_pipeline_lifecycle(&scope, true, false)
            .expect("catalog");
        let pipeline = catalog
            .iter()
            .find(|pipeline| pipeline.workflow_id == "pipeline-a")
            .expect("Pipeline A");
        let hash = store
            .get_published_workflow_version("pipeline-a", 1)
            .expect("historical Version")
            .content_hash;
        let mut rename = request(
            &scope.project_id,
            ManagementObjectRef {
                kind: ManagementObjectKind::Pipeline,
                id: "pipeline-a".to_owned(),
                version: None,
                expected_revision: pipeline.lifecycle_revision,
            },
            ManagementAction::Rename,
            "rename-pipeline-a",
        );
        rename.display_name = Some("Renamed Pipeline".to_owned());
        confirmed(&store, &scope, rename);
        let renamed = store
            .list_project_pipeline_lifecycle(&scope, true, false)
            .expect("renamed catalog")
            .into_iter()
            .find(|pipeline| pipeline.workflow_id == "pipeline-a")
            .expect("renamed A");
        assert_eq!(renamed.display_name, "Renamed Pipeline");
        assert_eq!(
            store
                .get_published_workflow_version("pipeline-a", 1)
                .expect("historical Version after rename")
                .content_hash,
            hash
        );
    }

    #[test]
    fn active_builder_and_sample_test_lease_block_draft_deletion() {
        let store = SqliteStore::open_in_memory().expect("store");
        let scope = test_scope();
        let draft = workflow_draft("busy-pipeline", &scope.project_id);
        store.save_workflow_draft(&draft).expect("Draft");
        let object = ManagementObjectRef {
            kind: ManagementObjectKind::WorkflowDraft,
            id: draft.id.clone(),
            version: None,
            expected_revision: 1,
        };
        let mut session = AgentSession::start(AgentKind::PipelineBuilder, AgentBudget::default())
            .with_project(&scope.project_id);
        session.set_builder_draft(&draft.id);
        store.save_agent_session(&session).expect("Builder session");
        let preview = store
            .preview_management(
                &scope,
                &request(
                    &scope.project_id,
                    object.clone(),
                    ManagementAction::MoveToTrash,
                    "builder-blocked",
                ),
            )
            .expect("preview");
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.code == "active_builder_session")
        );

        session.status = annotagent_core::AgentSessionStatus::Succeeded;
        session.updated_at = Utc::now();
        store.save_agent_session(&session).expect("finish Builder");
        store
            .acquire_management_lease(
                &scope,
                &object,
                "sample_test",
                "test-owner",
                chrono::Duration::minutes(5),
            )
            .expect("Sample Test lease");
        let preview = store
            .preview_management(
                &scope,
                &request(
                    &scope.project_id,
                    object.clone(),
                    ManagementAction::MoveToTrash,
                    "sample-blocked",
                ),
            )
            .expect("preview");
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.code == "active_builder_session")
        );
        store
            .release_management_lease(&scope, &object, "sample_test", "test-owner")
            .expect("release lease");
    }
}
