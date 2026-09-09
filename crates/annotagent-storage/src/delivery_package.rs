//! Frozen extension to the existing conversation Export receipt and event stream.
use crate::conversation_exports::export_event;
use crate::delivery_image_review::{decode, intent, receipt, snapshot};
use crate::{DeliveryImageReview, SqliteStore, StorageError, TaskDeliveryRevision};
use annotagent_core::{ImageId, dataset_delivery::DETECTION_PROFILE};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryPackageInput {
    pub command_id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    /// Exact current whole-image receipt revision for every selected image, including exclusions.
    pub image_reviews: BTreeMap<ImageId, u32>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenDeliveryImage {
    pub review: DeliveryImageReview,
    pub annotation_revision_ids: Vec<String>,
    /// Internal source evidence fingerprint; raw provider configuration is never packaged.
    pub source_evidence_sha256: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryPackageSnapshot {
    pub delivery: TaskDeliveryRevision,
    pub images: Vec<FrozenDeliveryImage>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryPackagePhase {
    Preparing,
    Exporting,
    Validating,
    Ready,
    Failed,
    Cancelled,
}
impl DeliveryPackagePhase {
    fn name(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Exporting => "exporting",
            Self::Validating => "validating",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryPackageJob {
    pub id: Uuid,
    pub phase: DeliveryPackagePhase,
    pub input: DeliveryPackageInput,
    pub snapshot: DeliveryPackageSnapshot,
    pub snapshot_sha256: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
fn invalid(s: &str) -> StorageError {
    StorageError::InvalidConversation(s.into())
}

// Preserve the existing one-start/one-terminal stream contract. Intermediate
// phases are available through the package read projection, not invented events.
fn phase_event(db: &Connection, id: Uuid, phase: DeliveryPackagePhase) -> Result<(), StorageError> {
    export_event(
        db,
        id,
        match phase {
            DeliveryPackagePhase::Ready => "completed",
            DeliveryPackagePhase::Failed | DeliveryPackagePhase::Cancelled => "failed",
            DeliveryPackagePhase::Preparing => "requested",
            DeliveryPackagePhase::Exporting | DeliveryPackagePhase::Validating => return Ok(()),
        },
    )
}
fn job(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    id: Uuid,
) -> Result<DeliveryPackageJob, StorageError> {
    let row = db.query_row("SELECT p.phase,p.input_json,p.snapshot_json,p.snapshot_sha256,e.result_json,e.error FROM delivery_export_snapshots p JOIN conversation_exports e ON e.id=p.export_id JOIN conversation_tasks t ON t.id=e.task_id JOIN project_conversations c ON c.id=t.conversation_id WHERE e.id=?1 AND e.project_id=?2 AND e.conversation_id=?3 AND e.task_id=?4 AND c.id=e.conversation_id AND c.project_id=e.project_id",params![id.to_string(),project,conversation.to_string(),task.to_string()],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?))).optional()?.ok_or_else(||invalid("owned delivery package not found"))?;
    Ok(DeliveryPackageJob {
        id,
        phase: serde_json::from_value(serde_json::Value::String(row.0))?,
        input: serde_json::from_str(&row.1)?,
        snapshot: serde_json::from_str(&row.2)?,
        snapshot_sha256: row.3,
        result: row.4.map(|s| serde_json::from_str(&s)).transpose()?,
        error: row.5,
    })
}

impl SqliteStore {
    /// Cheap owned polling for writer checkpoints; do not deserialize all frozen annotations per chunk.
    pub fn delivery_package_phase(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<DeliveryPackagePhase, StorageError> {
        self.with_connection(|db| {
            let phase:Option<String>=db.query_row("SELECT p.phase FROM delivery_export_snapshots p JOIN conversation_exports e ON e.id=p.export_id WHERE e.id=?1 AND e.project_id=?2 AND e.conversation_id=?3 AND e.task_id=?4",params![id.to_string(),project,conversation.to_string(),task.to_string()],|r|r.get(0)).optional()?;
            Ok(serde_json::from_value(serde_json::Value::String(phase.ok_or_else(||invalid("owned package not found"))?))?)
        })
    }
    pub fn delivery_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<DeliveryPackageJob, StorageError> {
        self.with_connection(|db| job(db, project, conversation, task, id))
    }

    /// Atomically freezes current owned intent, image bytes, complete review receipts and annotations.
    /// No client paths, annotation bodies or package-ready assertion are accepted.
    pub fn begin_delivery_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryPackageInput,
    ) -> Result<(DeliveryPackageJob, bool), StorageError> {
        if !input.confirmed || input.command_id.is_nil() {
            return Err(invalid("explicit package scope confirmation is required"));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            let saved=intent(&tx,project,conversation,task)?;
            let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_exports WHERE id=?1)",[input.command_id.to_string()],|r|r.get(0))?;
            if exists {
                let old=job(&tx,project,conversation,task,input.command_id)?;
                if old.input!=*input { return Err(invalid("package retry cannot change its frozen request")); }
                return Ok((old,false));
            }
            if saved.revision!=input.intent_revision || saved.content_sha256!=input.intent_sha256 || !saved.intent.missing_slots().is_empty() || !saved.intent.training_target.as_ref().is_some_and(annotagent_core::dataset_delivery::TrainingTarget::is_detection_preset) { return Err(invalid("package intent is stale, incomplete or unsupported")); }
            let scope=saved.intent.dataset_scope.as_ref().ok_or_else(||invalid("missing package scope"))?;
            if scope.len()!=input.image_reviews.len() || scope.iter().any(|i|!input.image_reviews.contains_key(&i.image_id)) { return Err(invalid("package confirmation must include every scoped image and exclusion")); }
            let mut images=Vec::new();
            for image in scope {
                let row=tx.query_row("SELECT revision,input_json,snapshot_json,created_at FROM delivery_image_reviews WHERE task_id=?1 AND intent_revision=?2 AND image_id=?3 ORDER BY revision DESC LIMIT 1",params![task.to_string(),saved.revision,image.image_id.to_string()],receipt).optional()?.ok_or_else(||invalid("package image needs whole-image confirmation"))?;
                let review=decode(row)?;
                if input.image_reviews.get(&image.image_id)!=Some(&review.revision) { return Err(invalid("whole-image confirmation changed before packaging")); }
                let current=snapshot(&tx,&saved,image.image_id,review.input.source_run_id)?;
                if current.sha256!=review.snapshot.sha256 { return Err(invalid("annotations changed after whole-image confirmation")); }
                let mut annotation_revision_ids=Vec::new();
                for annotation in &current.annotations {
                    let mut stmt=tx.prepare("SELECT revision_id FROM annotation_revisions WHERE annotation_id=?1 ORDER BY created_at,revision_id")?;
                    annotation_revision_ids.extend(stmt.query_map([annotation.id.to_string()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?);
                }
                let source_evidence_sha256=review.input.source_run_id.map(|run| {
                    let evidence:String=tx.query_row("SELECT json_array(project_schema_json,workflow_snapshot_json,provider,model) FROM runs WHERE id=?1 AND project_id=?2",params![run.to_string(),project],|r|r.get(0))?;
                    Ok::<_,StorageError>(format!("{:x}",Sha256::digest(evidence.as_bytes())))
                }).transpose()?;
                images.push(FrozenDeliveryImage {review,annotation_revision_ids,source_evidence_sha256});
            }
            images.sort_by_key(|i|i.review.input.image_id);
            let frozen=DeliveryPackageSnapshot {delivery:saved,images};
            let json=serde_json::to_string(&frozen)?;
            let hash=format!("{:x}",Sha256::digest(json.as_bytes()));
            let now=chrono::Utc::now().to_rfc3339();
            tx.execute("INSERT INTO conversation_exports(id,project_id,conversation_id,task_id,format,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![input.command_id.to_string(),project,conversation.to_string(),task.to_string(),DETECTION_PROFILE,now])?;
            tx.execute("INSERT INTO delivery_export_snapshots(export_id,input_json,snapshot_json,snapshot_sha256,phase,updated_at) VALUES(?1,?2,?3,?4,'preparing',?5)",params![input.command_id.to_string(),serde_json::to_string(input)?,json,hash,now])?;
            phase_event(&tx,input.command_id,DeliveryPackagePhase::Preparing)?;
            let result=job(&tx,project,conversation,task,input.command_id)?;
            tx.commit()?;
            Ok((result,true))
        })
    }

    /// CAS worker claim/stage transition. Cancellation or another worker wins atomically.
    pub fn advance_delivery_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        expected: DeliveryPackagePhase,
        next: DeliveryPackagePhase,
    ) -> Result<bool, StorageError> {
        if !matches!(
            (expected, next),
            (
                DeliveryPackagePhase::Preparing,
                DeliveryPackagePhase::Exporting
            ) | (
                DeliveryPackagePhase::Exporting,
                DeliveryPackagePhase::Validating
            )
        ) {
            return Err(invalid("invalid package stage transition"));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            job(&tx,project,conversation,task,id)?;
            let changed=tx.execute("UPDATE delivery_export_snapshots SET phase=?2,updated_at=?3 WHERE export_id=?1 AND phase=?4",params![id.to_string(),next.name(),chrono::Utc::now().to_rfc3339(),expected.name()])?==1;
            if changed { phase_event(&tx,id,next)?; }
            tx.commit()?;Ok(changed)
        })
    }

    /// Ready is only recorded after the caller has validated and atomically published the file.
    /// Existing terminal receipts cannot be overwritten, including cancelled jobs.
    pub fn finish_delivery_package(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        outcome: Result<&serde_json::Value, &str>,
        cancel: bool,
    ) -> Result<bool, StorageError> {
        if cancel && outcome.is_ok() {
            return Err(invalid("cancelled package cannot have a success result"));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let current = job(&tx, project, conversation, task, id)?;
            if matches!(
                current.phase,
                DeliveryPackagePhase::Ready
                    | DeliveryPackagePhase::Failed
                    | DeliveryPackagePhase::Cancelled
            ) {
                return Ok(false);
            }
            if outcome.is_ok() && current.phase != DeliveryPackagePhase::Validating {
                return Err(invalid("package must be validated before ready"));
            }
            let phase = if cancel {
                DeliveryPackagePhase::Cancelled
            } else if outcome.is_ok() {
                DeliveryPackagePhase::Ready
            } else {
                DeliveryPackagePhase::Failed
            };
            tx.execute(
                "UPDATE delivery_export_snapshots SET phase=?2,updated_at=?3 WHERE export_id=?1",
                params![
                    id.to_string(),
                    phase.name(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )?;
            tx.execute(
                "UPDATE conversation_exports SET result_json=?2,error=?3 WHERE id=?1",
                params![
                    id.to_string(),
                    outcome
                        .as_ref()
                        .ok()
                        .map(serde_json::to_string)
                        .transpose()?,
                    outcome.err()
                ],
            )?;
            phase_event(&tx, id, phase)?;
            tx.commit()?;
            Ok(true)
        })
    }
}
