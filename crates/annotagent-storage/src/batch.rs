use std::time::Duration;

use annotagent_core::{
    BatchBudgetLedger, BatchCheckpoint, BatchEvent, BatchId, BatchImageCheckpoint,
    BatchImageRecord, BatchImageStatus, BatchProgress, BatchRecord, BatchStatus, BatchUsage,
    ImageId, RunId,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params, types::Type};

use crate::{SqliteStore, StorageError};

#[derive(Debug, Clone, PartialEq)]
pub enum BatchClaimResult {
    Claimed(Box<BatchImageRecord>),
    Empty,
    BudgetExceeded(String),
}

impl SqliteStore {
    pub fn recover_orphaned_batch_leases(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<BatchId>, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch_ids = {
                let mut statement = transaction.prepare(
                    "SELECT id FROM dataset_batches
                     WHERE lease_owner IS NOT NULL
                       AND status IN ('pending', 'running', 'paused', 'awaiting_review')",
                )?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .map(|row| parse_id::<BatchId>(&row?, "batch id"))
                    .collect::<Result<Vec<_>, _>>()?
            };
            for batch_id in &batch_ids {
                let mut batch = read_batch(&transaction, *batch_id)?;
                reclaim_stale_images(&transaction, &mut batch)?;
                if batch.status == BatchStatus::Running {
                    batch.status = BatchStatus::Pending;
                }
                batch.lease_owner = None;
                batch.lease_expires_at = None;
                batch.updated_at = now;
                update_batch_runtime(&transaction, &batch)?;
                append_event(
                    &transaction,
                    *batch_id,
                    "orphaned_lease_recovered",
                    None,
                    &serde_json::json!({"status": batch.status}),
                    now,
                )?;
            }
            transaction.commit()?;
            Ok(batch_ids)
        })
    }

    pub fn create_batch(
        &self,
        mut batch: BatchRecord,
        images: &[(ImageId, String)],
    ) -> Result<BatchRecord, StorageError> {
        if images.is_empty() {
            return Err(StorageError::InvalidEnum(
                "dataset batch requires at least one image".to_owned(),
            ));
        }
        batch.status = BatchStatus::Pending;
        batch.budget_ledger = BatchBudgetLedger::default();
        batch.lease_owner = None;
        batch.lease_expires_at = None;
        batch.event_sequence = 0;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            insert_batch(&transaction, &batch)?;
            for (position, (image_id, image_path)) in images.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO batch_images
                     (batch_id, image_id, image_path, position, status, attempt_count,
                      reservation_json, actual_usage_json, checkpoint_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5, ?5, ?6, ?7)",
                    params![
                        batch.id.to_string(),
                        image_id.to_string(),
                        image_path,
                        to_i64(position)?,
                        serde_json::to_string(&BatchUsage::default())?,
                        serde_json::to_string(&BatchImageCheckpoint::default())?,
                        batch.created_at.to_rfc3339(),
                    ],
                )?;
            }
            append_event(
                &transaction,
                batch.id,
                "batch_created",
                None,
                &serde_json::json!({"image_count": images.len()}),
                batch.created_at,
            )?;
            transaction.commit()?;
            Ok(())
        })?;
        self.get_batch(batch.id)
    }

    pub fn get_batch(&self, batch_id: BatchId) -> Result<BatchRecord, StorageError> {
        self.with_connection(|connection| read_batch(connection, batch_id))
    }

    pub fn list_batches(&self, active_only: bool) -> Result<Vec<BatchRecord>, StorageError> {
        self.with_connection(|connection| {
            let sql = if active_only {
                "SELECT * FROM dataset_batches
                 WHERE status IN ('pending', 'running', 'paused', 'awaiting_review')
                 ORDER BY created_at"
            } else {
                "SELECT * FROM dataset_batches ORDER BY created_at DESC"
            };
            let mut statement = connection.prepare(sql)?;
            statement
                .query_map([], batch_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn list_batch_images(
        &self,
        batch_id: BatchId,
    ) -> Result<Vec<BatchImageRecord>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT batch_id, image_id, image_path, position, status, child_run_id,
                        attempt_count, reservation_json, actual_usage_json, checkpoint_json,
                        error, lease_owner, updated_at
                 FROM batch_images WHERE batch_id = ?1 ORDER BY position",
            )?;
            statement
                .query_map([batch_id.to_string()], batch_image_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn list_batch_events(&self, batch_id: BatchId) -> Result<Vec<BatchEvent>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT batch_id, sequence, kind, image_id, detail_json, occurred_at
                 FROM batch_events WHERE batch_id = ?1 ORDER BY sequence",
            )?;
            statement
                .query_map([batch_id.to_string()], batch_event_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn acquire_batch_lease(
        &self,
        batch_id: BatchId,
        owner: &str,
        lease_duration: Duration,
        now: DateTime<Utc>,
    ) -> Result<BatchRecord, StorageError> {
        if owner.trim().is_empty() {
            return Err(StorageError::BatchLeaseConflict(
                "lease owner cannot be empty".to_owned(),
            ));
        }
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut batch = read_batch(&transaction, batch_id)?;
            if batch.status.is_terminal() || batch.status == BatchStatus::Paused {
                return Err(StorageError::BatchLeaseConflict(format!(
                    "batch is {:?}",
                    batch.status
                )));
            }
            let owner_matches = batch.lease_owner.as_deref() == Some(owner);
            let expired = batch.lease_expires_at.is_none_or(|expires| expires <= now);
            if batch.lease_owner.is_some() && !owner_matches && !expired {
                return Err(StorageError::BatchLeaseConflict(format!(
                    "owned by {:?} until {:?}",
                    batch.lease_owner, batch.lease_expires_at
                )));
            }
            if (expired && batch.lease_owner.is_some() && !owner_matches)
                || batch.lease_owner.is_none()
            {
                reclaim_stale_images(&transaction, &mut batch)?;
            }
            let chrono_duration = chrono::Duration::from_std(lease_duration).map_err(|error| {
                StorageError::InvalidEnum(format!("invalid lease duration: {error}"))
            })?;
            batch.status = BatchStatus::Running;
            batch.lease_owner = Some(owner.to_owned());
            batch.lease_expires_at = Some(now + chrono_duration);
            batch.updated_at = now;
            update_batch_runtime(&transaction, &batch)?;
            append_event(
                &transaction,
                batch_id,
                "lease_acquired",
                None,
                &serde_json::json!({"owner": owner, "expires_at": batch.lease_expires_at}),
                now,
            )?;
            transaction.commit()?;
            read_batch(connection, batch_id)
        })
    }

    pub fn renew_batch_lease(
        &self,
        batch_id: BatchId,
        owner: &str,
        lease_duration: Duration,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let chrono_duration = chrono::Duration::from_std(lease_duration).map_err(|error| {
                StorageError::InvalidEnum(format!("invalid lease duration: {error}"))
            })?;
            let changed = connection.execute(
                "UPDATE dataset_batches SET lease_expires_at = ?3, updated_at = ?4
                 WHERE id = ?1 AND lease_owner = ?2 AND status = 'running'",
                params![
                    batch_id.to_string(),
                    owner,
                    (now + chrono_duration).to_rfc3339(),
                    now.to_rfc3339()
                ],
            )?;
            if changed == 0 {
                return Err(StorageError::BatchLeaseConflict(
                    "cannot renew a lease not owned by this worker".to_owned(),
                ));
            }
            Ok(())
        })
    }

    pub fn claim_batch_image(
        &self,
        batch_id: BatchId,
        owner: &str,
        reservation: &BatchUsage,
        now: DateTime<Utc>,
    ) -> Result<BatchClaimResult, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut batch = read_batch(&transaction, batch_id)?;
            verify_live_lease(&batch, owner, now)?;
            let image_id = transaction
                .query_row(
                    "SELECT image_id FROM batch_images
                     WHERE batch_id = ?1 AND status = 'pending'
                     ORDER BY position LIMIT 1",
                    [batch_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(image_id) = image_id else {
                transaction.commit()?;
                return Ok(BatchClaimResult::Empty);
            };
            let Some(current_total) = batch.budget_ledger.committed_and_reserved() else {
                return Err(StorageError::InvalidEnum(
                    "batch budget ledger overflow".to_owned(),
                ));
            };
            let Some(projected) = current_total.checked_add(reservation) else {
                return Err(StorageError::InvalidEnum(
                    "batch budget reservation overflow".to_owned(),
                ));
            };
            if let Some(reason) = batch.budget_limits.exceeded_by(&projected, now) {
                batch.status = BatchStatus::BudgetExceeded;
                batch.lease_owner = None;
                batch.lease_expires_at = None;
                batch.updated_at = now;
                update_batch_runtime(&transaction, &batch)?;
                append_event(
                    &transaction,
                    batch_id,
                    "budget_exceeded",
                    None,
                    &serde_json::json!({"reason": reason}),
                    now,
                )?;
                transaction.commit()?;
                return Ok(BatchClaimResult::BudgetExceeded(reason));
            }
            batch.budget_ledger.reserved = batch
                .budget_ledger
                .reserved
                .checked_add(reservation)
                .ok_or_else(|| StorageError::InvalidEnum("reserved budget overflow".to_owned()))?;
            batch.updated_at = now;
            update_batch_runtime(&transaction, &batch)?;
            transaction.execute(
                "UPDATE batch_images
                 SET status = 'leased', attempt_count = attempt_count + 1,
                     reservation_json = ?3, lease_owner = ?4, updated_at = ?5
                 WHERE batch_id = ?1 AND image_id = ?2 AND status = 'pending'",
                params![
                    batch_id.to_string(),
                    image_id,
                    serde_json::to_string(reservation)?,
                    owner,
                    now.to_rfc3339(),
                ],
            )?;
            let parsed_image_id = parse_id::<ImageId>(&image_id, "image id")?;
            append_event(
                &transaction,
                batch_id,
                "image_claimed",
                Some(parsed_image_id),
                &serde_json::json!({"owner": owner, "reservation": reservation}),
                now,
            )?;
            let image = read_batch_image(&transaction, batch_id, parsed_image_id)?;
            transaction.commit()?;
            Ok(BatchClaimResult::Claimed(Box::new(image)))
        })
    }

    pub fn mark_batch_image_running(
        &self,
        batch_id: BatchId,
        image_id: ImageId,
        owner: &str,
        child_run_id: RunId,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch = read_batch(&transaction, batch_id)?;
            verify_live_lease(&batch, owner, now)?;
            let changed = transaction.execute(
                "UPDATE batch_images SET status = 'running', child_run_id = ?4, updated_at = ?5
                 WHERE batch_id = ?1 AND image_id = ?2 AND lease_owner = ?3 AND status = 'leased'",
                params![
                    batch_id.to_string(),
                    image_id.to_string(),
                    owner,
                    child_run_id.to_string(),
                    now.to_rfc3339()
                ],
            )?;
            if changed == 0 {
                return Err(StorageError::BatchLeaseConflict(
                    "image is not leased by this worker".to_owned(),
                ));
            }
            append_event(
                &transaction,
                batch_id,
                "image_started",
                Some(image_id),
                &serde_json::json!({"run_id": child_run_id}),
                now,
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finish_batch_image(
        &self,
        batch_id: BatchId,
        image_id: ImageId,
        owner: &str,
        status: BatchImageStatus,
        actual: &BatchUsage,
        checkpoint: &BatchImageCheckpoint,
        error: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<bool, StorageError> {
        if matches!(
            status,
            BatchImageStatus::Pending | BatchImageStatus::Leased | BatchImageStatus::Running
        ) {
            return Err(StorageError::InvalidEnum(
                "finish status must be terminal or awaiting_review".to_owned(),
            ));
        }
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut batch = read_batch(&transaction, batch_id)?;
            let image = read_batch_image(&transaction, batch_id, image_id)?;
            if matches!(
                image.status,
                BatchImageStatus::Completed | BatchImageStatus::AwaitingReview
            ) || (image.status == BatchImageStatus::Failed && status == BatchImageStatus::Failed)
            {
                transaction.commit()?;
                return Ok(false);
            }
            if image.lease_owner.as_deref() != Some(owner) && !batch.status.is_terminal() {
                return Err(StorageError::BatchLeaseConflict(
                    "cannot finish image without owning the batch lease".to_owned(),
                ));
            }
            batch.budget_ledger.reserved = batch
                .budget_ledger
                .reserved
                .saturating_sub(&image.reservation);
            batch.budget_ledger.consumed = batch
                .budget_ledger
                .consumed
                .checked_add(actual)
                .ok_or_else(|| StorageError::InvalidEnum("consumed budget overflow".to_owned()))?;
            batch.updated_at = now;
            update_batch_runtime(&transaction, &batch)?;
            let cumulative_actual = image
                .actual_usage
                .checked_add(actual)
                .ok_or_else(|| StorageError::InvalidEnum("image usage overflow".to_owned()))?;
            transaction.execute(
                "UPDATE batch_images SET status = ?3, reservation_json = ?4,
                     actual_usage_json = ?5, checkpoint_json = ?6, error = ?7,
                     lease_owner = NULL, updated_at = ?8
                 WHERE batch_id = ?1 AND image_id = ?2",
                params![
                    batch_id.to_string(),
                    image_id.to_string(),
                    enum_text(status)?,
                    serde_json::to_string(&BatchUsage::default())?,
                    serde_json::to_string(&cumulative_actual)?,
                    serde_json::to_string(checkpoint)?,
                    error,
                    now.to_rfc3339(),
                ],
            )?;
            append_event(
                &transaction,
                batch_id,
                "image_finished",
                Some(image_id),
                &serde_json::json!({"status": status, "actual_usage": actual, "error": error}),
                now,
            )?;
            transaction.commit()?;
            Ok(true)
        })
    }

    pub fn set_batch_status(
        &self,
        batch_id: BatchId,
        status: BatchStatus,
        now: DateTime<Utc>,
    ) -> Result<BatchRecord, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut batch = read_batch(&transaction, batch_id)?;
            if batch.status.is_terminal() && batch.status != status {
                return Err(StorageError::InvalidEnum(format!(
                    "terminal batch {:?} cannot transition to {status:?}",
                    batch.status
                )));
            }
            batch.status = status;
            batch.updated_at = now;
            if status != BatchStatus::Running {
                batch.lease_owner = None;
                batch.lease_expires_at = None;
            }
            if status == BatchStatus::Cancelled {
                release_unfinished_reservations(&transaction, &mut batch)?;
                transaction.execute(
                    "UPDATE batch_images SET status = 'cancelled', lease_owner = NULL,
                         reservation_json = ?2, updated_at = ?3
                     WHERE batch_id = ?1 AND status IN ('pending', 'leased', 'running')",
                    params![
                        batch_id.to_string(),
                        serde_json::to_string(&BatchUsage::default())?,
                        now.to_rfc3339()
                    ],
                )?;
            } else if status == BatchStatus::Paused {
                release_leased_reservations(&transaction, &mut batch)?;
            }
            update_batch_runtime(&transaction, &batch)?;
            append_event(
                &transaction,
                batch_id,
                "batch_status_changed",
                None,
                &serde_json::json!({"status": status}),
                now,
            )?;
            transaction.commit()?;
            read_batch(connection, batch_id)
        })
    }

    pub fn retry_failed_batch_images(
        &self,
        batch_id: BatchId,
        now: DateTime<Utc>,
    ) -> Result<usize, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch = read_batch(&transaction, batch_id)?;
            if !matches!(
                batch.status,
                BatchStatus::Failed | BatchStatus::Partial | BatchStatus::Paused
            ) {
                return Err(StorageError::InvalidEnum(
                    "failed images can only be retried from failed, partial, or paused batches"
                        .to_owned(),
                ));
            }
            let changed = transaction.execute(
                "UPDATE batch_images SET status = 'pending', error = NULL, updated_at = ?2
                 WHERE batch_id = ?1 AND status = 'failed'",
                params![batch_id.to_string(), now.to_rfc3339()],
            )?;
            transaction.execute(
                "UPDATE dataset_batches SET status = 'pending', updated_at = ?2
                 WHERE id = ?1",
                params![batch_id.to_string(), now.to_rfc3339()],
            )?;
            append_event(
                &transaction,
                batch_id,
                "failed_images_retried",
                None,
                &serde_json::json!({"count": changed}),
                now,
            )?;
            transaction.commit()?;
            Ok(changed)
        })
    }

    pub fn finalize_batch(
        &self,
        batch_id: BatchId,
        owner: &str,
        now: DateTime<Utc>,
    ) -> Result<BatchRecord, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut batch = read_batch(&transaction, batch_id)?;
            if batch.status.is_terminal() {
                transaction.commit()?;
                return read_batch(connection, batch_id);
            }
            if batch.lease_owner.as_deref() != Some(owner) {
                return Err(StorageError::BatchLeaseConflict(
                    "cannot finalize without the batch lease".to_owned(),
                ));
            }
            let statuses = batch_status_counts(&transaction, batch_id)?;
            if statuses.iter().any(|(status, count)| {
                *count > 0
                    && matches!(
                        status,
                        BatchImageStatus::Pending
                            | BatchImageStatus::Leased
                            | BatchImageStatus::Running
                    )
            }) {
                return Err(StorageError::InvalidEnum(
                    "batch still has unfinished images".to_owned(),
                ));
            }
            let completed = count_status(&statuses, BatchImageStatus::Completed);
            let failed = count_status(&statuses, BatchImageStatus::Failed);
            let review = count_status(&statuses, BatchImageStatus::AwaitingReview);
            let cancelled = count_status(&statuses, BatchImageStatus::Cancelled);
            batch.status = if review > 0 {
                BatchStatus::AwaitingReview
            } else if failed > 0 && completed > 0 {
                BatchStatus::Partial
            } else if failed > 0 {
                BatchStatus::Failed
            } else if cancelled > 0 && completed == 0 {
                BatchStatus::Cancelled
            } else {
                BatchStatus::Completed
            };
            batch.lease_owner = None;
            batch.lease_expires_at = None;
            batch.updated_at = now;
            update_batch_runtime(&transaction, &batch)?;
            append_event(
                &transaction,
                batch_id,
                "batch_finished",
                None,
                &serde_json::json!({"status": batch.status}),
                now,
            )?;
            transaction.commit()?;
            read_batch(connection, batch_id)
        })
    }

    pub fn batch_checkpoint(&self, batch_id: BatchId) -> Result<BatchCheckpoint, StorageError> {
        let batch = self.get_batch(batch_id)?;
        let images = self.list_batch_images(batch_id)?;
        let mut remaining_images = Vec::new();
        let mut completed_images = Vec::new();
        let mut current_node_states = std::collections::BTreeMap::new();
        let mut artifact_references = std::collections::BTreeMap::new();
        let mut retry_counters = std::collections::BTreeMap::new();
        let mut review_suspensions = std::collections::BTreeMap::new();
        for image in images {
            if image.status == BatchImageStatus::Completed {
                completed_images.push(image.image_id);
            } else if image.status != BatchImageStatus::Cancelled {
                remaining_images.push(image.image_id);
            }
            current_node_states.insert(image.image_id, image.checkpoint.node_states.clone());
            artifact_references
                .insert(image.image_id, image.checkpoint.artifact_references.clone());
            retry_counters.insert(image.image_id, image.checkpoint.retry_counters.clone());
            review_suspensions.insert(image.image_id, image.checkpoint.review_suspensions.clone());
        }
        let event_sequence = batch.event_sequence;
        Ok(BatchCheckpoint {
            batch,
            remaining_images,
            completed_images,
            current_node_states,
            artifact_references,
            retry_counters,
            review_suspensions,
            event_sequence,
        })
    }

    pub fn batch_progress(&self, batch_id: BatchId) -> Result<BatchProgress, StorageError> {
        self.with_connection(|connection| {
            read_batch(connection, batch_id)?;
            let statuses = batch_status_counts(connection, batch_id)?;
            let pending = count_status(&statuses, BatchImageStatus::Pending);
            let leased = count_status(&statuses, BatchImageStatus::Leased);
            let running = count_status(&statuses, BatchImageStatus::Running);
            let completed = count_status(&statuses, BatchImageStatus::Completed);
            let failed = count_status(&statuses, BatchImageStatus::Failed);
            let review = count_status(&statuses, BatchImageStatus::AwaitingReview);
            let cancelled = count_status(&statuses, BatchImageStatus::Cancelled);
            Ok(BatchProgress {
                total_images: pending + leased + running + completed + failed + review + cancelled,
                pending_images: pending + leased,
                running_images: running,
                completed_images: completed,
                failed_images: failed,
                review_images: review,
                cancelled_images: cancelled,
            })
        })
    }
}

fn insert_batch(transaction: &Transaction<'_>, batch: &BatchRecord) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO dataset_batches
         (id, project_id, project_path, provider, status, max_concurrency, workflow_version,
          workflow_snapshot_json, project_snapshot_json, budget_limits_json, budget_ledger_json,
          lease_owner, lease_expires_at, event_sequence, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            batch.id.to_string(),
            batch.project_id,
            batch.project_path,
            batch.provider,
            enum_text(batch.status)?,
            i64::from(batch.max_concurrency),
            batch.workflow_version,
            serde_json::to_string(&batch.workflow_snapshot)?,
            serde_json::to_string(&batch.project_snapshot)?,
            serde_json::to_string(&batch.budget_limits)?,
            serde_json::to_string(&batch.budget_ledger)?,
            batch.lease_owner,
            batch.lease_expires_at.map(|value| value.to_rfc3339()),
            to_i64(batch.event_sequence)?,
            batch.created_at.to_rfc3339(),
            batch.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn update_batch_runtime(
    transaction: &Transaction<'_>,
    batch: &BatchRecord,
) -> Result<(), StorageError> {
    transaction.execute(
        "UPDATE dataset_batches SET status = ?2, budget_ledger_json = ?3,
             lease_owner = ?4, lease_expires_at = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            batch.id.to_string(),
            enum_text(batch.status)?,
            serde_json::to_string(&batch.budget_ledger)?,
            batch.lease_owner,
            batch.lease_expires_at.map(|value| value.to_rfc3339()),
            batch.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn read_batch(connection: &Connection, batch_id: BatchId) -> Result<BatchRecord, StorageError> {
    connection
        .query_row(
            "SELECT * FROM dataset_batches WHERE id = ?1",
            [batch_id.to_string()],
            batch_from_row,
        )
        .optional()?
        .ok_or(StorageError::BatchNotFound(batch_id))
}

fn batch_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BatchRecord> {
    Ok(BatchRecord {
        id: parse_column_id(row, 0, "batch id")?,
        project_id: row.get(1)?,
        project_path: row.get(2)?,
        provider: row.get(3)?,
        status: parse_enum_column(row, 4)?,
        max_concurrency: row.get::<_, u32>(5)?,
        workflow_version: row.get(6)?,
        workflow_snapshot: parse_json_column(row, 7)?,
        project_snapshot: parse_json_column(row, 8)?,
        budget_limits: parse_json_column(row, 9)?,
        budget_ledger: parse_json_column(row, 10)?,
        lease_owner: row.get(11)?,
        lease_expires_at: parse_optional_date_column(row, 12)?,
        event_sequence: u64_column(row, 13)?,
        created_at: parse_date_column(row, 14)?,
        updated_at: parse_date_column(row, 15)?,
    })
}

fn read_batch_image(
    connection: &Connection,
    batch_id: BatchId,
    image_id: ImageId,
) -> Result<BatchImageRecord, StorageError> {
    connection
        .query_row(
            "SELECT batch_id, image_id, image_path, position, status, child_run_id,
                    attempt_count, reservation_json, actual_usage_json, checkpoint_json,
                    error, lease_owner, updated_at
             FROM batch_images WHERE batch_id = ?1 AND image_id = ?2",
            params![batch_id.to_string(), image_id.to_string()],
            batch_image_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::InvalidEnum(format!("batch image {image_id} was not found")))
}

fn batch_image_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BatchImageRecord> {
    Ok(BatchImageRecord {
        batch_id: parse_column_id(row, 0, "batch id")?,
        image_id: parse_column_id(row, 1, "image id")?,
        image_path: row.get(2)?,
        position: u64_column(row, 3)?,
        status: parse_enum_column(row, 4)?,
        child_run_id: parse_optional_id_column(row, 5, "run id")?,
        attempt_count: row.get(6)?,
        reservation: parse_json_column(row, 7)?,
        actual_usage: parse_json_column(row, 8)?,
        checkpoint: parse_json_column(row, 9)?,
        error: row.get(10)?,
        lease_owner: row.get(11)?,
        updated_at: parse_date_column(row, 12)?,
    })
}

fn batch_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BatchEvent> {
    Ok(BatchEvent {
        batch_id: parse_column_id(row, 0, "batch id")?,
        sequence: u64_column(row, 1)?,
        kind: row.get(2)?,
        image_id: parse_optional_id_column(row, 3, "image id")?,
        detail: parse_json_column(row, 4)?,
        occurred_at: parse_date_column(row, 5)?,
    })
}

fn append_event(
    transaction: &Transaction<'_>,
    batch_id: BatchId,
    kind: &str,
    image_id: Option<ImageId>,
    detail: &serde_json::Value,
    now: DateTime<Utc>,
) -> Result<u64, StorageError> {
    let current = transaction.query_row(
        "SELECT event_sequence FROM dataset_batches WHERE id = ?1",
        [batch_id.to_string()],
        |row| u64_column(row, 0),
    )?;
    let sequence = current
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidEnum("batch event sequence overflow".to_owned()))?;
    transaction.execute(
        "UPDATE dataset_batches SET event_sequence = ?2, updated_at = ?3 WHERE id = ?1",
        params![batch_id.to_string(), to_i64(sequence)?, now.to_rfc3339()],
    )?;
    transaction.execute(
        "INSERT INTO batch_events
         (batch_id, sequence, kind, image_id, detail_json, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            batch_id.to_string(),
            to_i64(sequence)?,
            kind,
            image_id.map(|value| value.to_string()),
            serde_json::to_string(detail)?,
            now.to_rfc3339(),
        ],
    )?;
    Ok(sequence)
}

fn verify_live_lease(
    batch: &BatchRecord,
    owner: &str,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    if batch.status != BatchStatus::Running
        || batch.lease_owner.as_deref() != Some(owner)
        || batch.lease_expires_at.is_none_or(|expires| expires <= now)
    {
        return Err(StorageError::BatchLeaseConflict(
            "worker does not own a live running lease".to_owned(),
        ));
    }
    Ok(())
}

fn reclaim_stale_images(
    transaction: &Transaction<'_>,
    batch: &mut BatchRecord,
) -> Result<(), StorageError> {
    let reservations = {
        let mut statement = transaction.prepare(
            "SELECT reservation_json FROM batch_images
             WHERE batch_id = ?1 AND status IN ('leased', 'running')",
        )?;
        statement
            .query_map([batch.id.to_string()], |row| row.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str::<BatchUsage>(&row?)?))
            .collect::<Result<Vec<_>, StorageError>>()?
    };
    for reservation in reservations {
        batch.budget_ledger.reserved = batch.budget_ledger.reserved.saturating_sub(&reservation);
    }
    transaction.execute(
        "UPDATE batch_images SET status = 'pending', lease_owner = NULL,
             reservation_json = ?2, child_run_id = NULL
         WHERE batch_id = ?1 AND status IN ('leased', 'running')",
        params![
            batch.id.to_string(),
            serde_json::to_string(&BatchUsage::default())?
        ],
    )?;
    Ok(())
}

fn release_unfinished_reservations(
    transaction: &Transaction<'_>,
    batch: &mut BatchRecord,
) -> Result<(), StorageError> {
    reclaim_stale_images(transaction, batch)
}

fn release_leased_reservations(
    transaction: &Transaction<'_>,
    batch: &mut BatchRecord,
) -> Result<(), StorageError> {
    let reservations = {
        let mut statement = transaction.prepare(
            "SELECT reservation_json FROM batch_images
             WHERE batch_id = ?1 AND status = 'leased'",
        )?;
        statement
            .query_map([batch.id.to_string()], |row| row.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str::<BatchUsage>(&row?)?))
            .collect::<Result<Vec<_>, StorageError>>()?
    };
    for reservation in reservations {
        batch.budget_ledger.reserved = batch.budget_ledger.reserved.saturating_sub(&reservation);
    }
    transaction.execute(
        "UPDATE batch_images SET status = 'pending', lease_owner = NULL,
             reservation_json = ?2, child_run_id = NULL
         WHERE batch_id = ?1 AND status = 'leased'",
        params![
            batch.id.to_string(),
            serde_json::to_string(&BatchUsage::default())?
        ],
    )?;
    Ok(())
}

fn batch_status_counts(
    connection: &Connection,
    batch_id: BatchId,
) -> Result<Vec<(BatchImageStatus, u64)>, StorageError> {
    let mut statement = connection
        .prepare("SELECT status, COUNT(*) FROM batch_images WHERE batch_id = ?1 GROUP BY status")?;
    statement
        .query_map([batch_id.to_string()], |row| {
            let raw = row.get::<_, String>(0)?;
            let status =
                serde_json::from_value(serde_json::Value::String(raw)).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
                })?;
            Ok((status, u64_column(row, 1)?))
        })?
        .map(|row| row.map_err(StorageError::from))
        .collect()
}

fn count_status(statuses: &[(BatchImageStatus, u64)], wanted: BatchImageStatus) -> u64 {
    statuses
        .iter()
        .find_map(|(status, count)| (*status == wanted).then_some(*count))
        .unwrap_or(0)
}

fn enum_text<T: serde::Serialize>(value: T) -> Result<String, StorageError> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StorageError::InvalidEnum("enum did not serialize as string".to_owned()))
}

fn parse_enum_column<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    let raw = row.get::<_, String>(index)?;
    serde_json::from_value(serde_json::Value::String(raw)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn parse_json_column<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    let raw = row.get::<_, String>(index)?;
    serde_json::from_str(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn parse_date_column(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let raw = row.get::<_, String>(index)?;
    DateTime::parse_from_rfc3339(&raw)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
}

fn parse_optional_date_column(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<DateTime<Utc>>> {
    row.get::<_, Option<String>>(index)?
        .map(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
                })
        })
        .transpose()
}

fn parse_column_id<T: std::str::FromStr>(
    row: &rusqlite::Row<'_>,
    index: usize,
    label: &str,
) -> rusqlite::Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw = row.get::<_, String>(index)?;
    raw.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid {label}: {error}"),
            )),
        )
    })
}

fn parse_optional_id_column<T: std::str::FromStr>(
    row: &rusqlite::Row<'_>,
    index: usize,
    label: &str,
) -> rusqlite::Result<Option<T>>
where
    T::Err: std::fmt::Display,
{
    row.get::<_, Option<String>>(index)?
        .map(|raw| {
            raw.parse().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid {label}: {error}"),
                    )),
                )
            })
        })
        .transpose()
}

fn parse_id<T: std::str::FromStr>(raw: &str, label: &str) -> Result<T, StorageError>
where
    T::Err: std::fmt::Display,
{
    raw.parse()
        .map_err(|error| StorageError::InvalidEnum(format!("invalid {label}: {error}")))
}

fn to_i64(value: impl TryInto<i64>) -> Result<i64, StorageError> {
    value
        .try_into()
        .map_err(|_| StorageError::InvalidEnum("integer exceeds SQLite range".to_owned()))
}

fn u64_column(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Integer, Box::new(error))
    })
}
