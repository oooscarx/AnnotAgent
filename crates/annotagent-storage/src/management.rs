use std::collections::BTreeSet;

use annotagent_core::{
    BatchStatus, ManagementAction, ManagementBlocker, ManagementImpact, ManagementObjectKind,
    ManagementObjectRef, ManagementOperationStatus, ManagementPreview, ManagementReceipt,
    ManagementRequest, ProjectId, RunStatus, TrashEntry,
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
    deleted_at: Option<String>,
    deletion_operation_id: Option<String>,
}

impl SqliteStore {
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

            let affected = match request.action {
                ManagementAction::MoveToTrash => move_to_trash(
                    &transaction,
                    scope,
                    &current_preview.objects,
                    &operation_id,
                    now,
                )?,
                ManagementAction::Restore => restore(
                    &transaction,
                    scope,
                    &current_preview.objects,
                    &operation_id,
                    now,
                )?,
                _ => {
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
                purge: None,
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
            entries.sort_by(|left, right| right.deleted_at.cmp(&left.deleted_at));
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
            _ => blockers.push(ManagementBlocker {
                code: "unsupported_management_action".to_owned(),
                object: object.clone(),
                message:
                    "Pipeline lifecycle management is delivered by the next management milestone."
                        .to_owned(),
                related_ids: Vec::new(),
            }),
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
    let mut normalized = objects
        .iter()
        .filter(|object| {
            object.kind != ManagementObjectKind::Run || !child_runs.contains(&object.id)
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
            if !batch.status.is_terminal() || batch.lease_owner.is_some() {
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
        _ => blockers.push(blocker(
            "unsupported_management_action",
            object,
            "This action is not supported for Dataset Runs.",
        )),
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
            _ => unreachable!("preview blocks unsupported objects"),
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
            _ => unreachable!("preview blocks unsupported objects"),
        }
    }
    affected.sort();
    Ok(affected)
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
                    deleted_at, deletion_operation_id
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
                ))
            },
        )
        .optional()?
        .ok_or_else(|| management_error("entity_purged", format!("Dataset Run {id} was not found")))
        .and_then(
            |(id, project_id, status, revision, lease_owner, deleted_at, operation)| {
                Ok(BatchLifecycle {
                    id,
                    project_id,
                    status: serde_json::from_value(serde_json::Value::String(status))?,
                    revision: to_u64(revision),
                    lease_owner,
                    deleted_at,
                    deletion_operation_id: operation,
                })
            },
        )
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
        BatchBudgetLedger, BatchBudgetLimits, BatchId, BatchRecord, ImageId, RunId,
    };
    use annotagent_runtime::{RunRecord, RuntimeStore};

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
}
