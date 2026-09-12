//! Whole-image confirmation is explicit and bound to a server-derived snapshot.
use crate::{SqliteStore, StorageError, TaskDeliveryRevision};
use annotagent_core::{Annotation, ImageId, ReviewStatus, RunId};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryImageDecision {
    PositiveComplete,
    NegativeConfirmed,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryImageReviewInput {
    pub command_id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub image_id: ImageId,
    pub source_run_id: Option<RunId>,
    pub expected_snapshot_sha256: String,
    pub expected_review_revision: u32,
    pub decision: DeliveryImageDecision,
    pub reason: Option<String>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryImageSnapshot {
    pub image_id: ImageId,
    pub content_sha256: String,
    pub source_run_id: Option<RunId>,
    pub annotations: Vec<Annotation>,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryImageReview {
    pub revision: u32,
    pub input: DeliveryImageReviewInput,
    pub snapshot: DeliveryImageSnapshot,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryRunSource {
    pub processing_operation_id: String,
    pub batch_id: String,
    pub run_id: String,
    pub model: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryObjectEdit {
    pub command_id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub source_run_id: RunId,
    pub annotation_id: annotagent_core::AnnotationId,
    pub expected_snapshot_sha256: String,
    pub label: String,
    pub value: annotagent_core::AnnotationValue,
    pub review_status: ReviewStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryObjectCreate {
    pub command_id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub source_run_id: RunId,
    pub expected_snapshot_sha256: String,
    pub label: String,
    pub value: annotagent_core::AnnotationValue,
    pub reason: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

pub(super) fn intent(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> Result<TaskDeliveryRevision, StorageError> {
    let row: Option<(u32, String, String)> = db.query_row(
        "SELECT d.revision,d.content_sha256,d.intent_json FROM task_delivery_intents d JOIN conversation_tasks t ON t.id=d.task_id JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.id=?2 AND c.project_id=?3 ORDER BY d.revision DESC LIMIT 1",
        params![task.to_string(), conversation.to_string(), project], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional()?;
    let (revision, content_sha256, json) =
        row.ok_or_else(|| invalid("owned delivery intent not found"))?;
    Ok(TaskDeliveryRevision {
        revision,
        content_sha256,
        intent: serde_json::from_str(&json)?,
    })
}

pub(crate) fn snapshot(
    db: &Connection,
    saved: &TaskDeliveryRevision,
    image: ImageId,
    run: Option<RunId>,
) -> Result<DeliveryImageSnapshot, StorageError> {
    let selected = saved
        .intent
        .dataset_scope
        .as_ref()
        .and_then(|images| images.iter().find(|i| i.image_id == image))
        .ok_or_else(|| invalid("image is outside the frozen delivery scope"))?;
    let hash: Option<String> = db
        .query_row(
            "SELECT sha256 FROM images WHERE id=?1 AND project_id=?2",
            params![image.to_string(), saved.intent.project_id],
            |r| r.get(0),
        )
        .optional()?;
    if hash.as_deref() != Some(selected.content_sha256.as_str()) {
        return Err(invalid("delivery image changed or is no longer owned"));
    }
    if let Some(run) = run {
        let eligible: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM runs r JOIN run_images i ON i.run_id=r.id WHERE r.id=?1 AND r.project_id=?2 AND i.image_id=?3 AND r.status IN ('completed','completed_with_review','partial'))", params![run.to_string(), saved.intent.project_id, image.to_string()], |r| r.get(0))?;
        if !eligible {
            return Err(invalid(
                "source Run is not an owned terminal Run for this image",
            ));
        }
    }
    let mut stmt = db.prepare("SELECT a.annotation_json FROM annotations a LEFT JOIN runs r ON r.id=a.run_id LEFT JOIN run_provenance_tombstones p ON p.run_id=a.run_id WHERE COALESCE(r.project_id,p.project_id)=?1 AND a.image_id=?2 AND (?3 IS NULL OR a.run_id=?3) ORDER BY a.id")?;
    let rows = stmt.query_map(
        params![
            saved.intent.project_id,
            image.to_string(),
            run.map(|r| r.to_string())
        ],
        |r| r.get::<_, String>(0),
    )?;
    let annotations: Vec<Annotation> = rows
        .map(|r| Ok(serde_json::from_str(&r?)?))
        .collect::<Result<_, StorageError>>()?;
    if annotations.iter().any(|a| a.image_id != image) {
        return Err(invalid("annotation image identity is inconsistent"));
    }
    let bytes = serde_json::to_vec(&(image, &selected.content_sha256, run, &annotations))?;
    Ok(DeliveryImageSnapshot {
        image_id: image,
        content_sha256: selected.content_sha256.clone(),
        source_run_id: run,
        annotations,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

pub(super) fn receipt(row: &rusqlite::Row<'_>) -> rusqlite::Result<(u32, String, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}
pub(super) fn decode(
    row: (u32, String, String, String),
) -> Result<DeliveryImageReview, StorageError> {
    Ok(DeliveryImageReview {
        revision: row.0,
        input: serde_json::from_str(&row.1)?,
        snapshot: serde_json::from_str(&row.2)?,
        created_at: row.3,
    })
}

impl SqliteStore {
    /// A manual missing-object correction, not a model result or image approval.
    /// Reuses formal annotations/revision history with the same snapshot CAS.
    pub fn create_delivery_object(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        input: &DeliveryObjectCreate,
    ) -> Result<annotagent_core::AnnotationRevision, StorageError> {
        use annotagent_core::{
            AnnotationId, AnnotationRevision, AnnotationRevisionId, AnnotationSource,
            RevisionActor, TaskKind,
        };
        if input.command_id.is_nil()
            || input.reason.trim().is_empty()
            || input.reason.len() > 2000
            || !matches!(
                input.value,
                annotagent_core::AnnotationValue::BoundingBox { .. }
            )
        {
            return Err(invalid(
                "Missing-object correction requires a bbox, command ID and bounded reason",
            ));
        }
        input
            .value
            .validate()
            .map_err(|_| invalid("Invalid missing-object geometry"))?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                project,
                conversation,
                task,
                image,
                input
            ))?)
        );
        let reason = format!("{} [delivery create {fingerprint}]", input.reason);
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            let saved=intent(&tx,project,conversation,task)?;
            let previous:Option<String>=tx.query_row("SELECT revision_json FROM annotation_revisions WHERE revision_id=?1",[input.command_id.to_string()],|r|r.get(0)).optional()?;
            if let Some(json)=previous {let old:AnnotationRevision=serde_json::from_str(&json)?;if old.reason.as_deref()!=Some(&reason){return Err(invalid("Missing-object command changed its immutable scope"));}return Ok(old);}
            if saved.revision!=input.intent_revision || saved.content_sha256!=input.intent_sha256 || !saved.intent.label_spec.as_ref().is_some_and(|labels|labels.iter().any(|l|l.stable_id==input.label)){return Err(invalid("Delivery version or label changed before adding an object"));}
            let current=snapshot(&tx,&saved,image,Some(input.source_run_id))?;
            if current.sha256!=input.expected_snapshot_sha256{return Err(invalid("Image annotations changed before adding an object"));}
            let schema:String=tx.query_row("SELECT project_schema_json FROM runs WHERE id=?1",[input.source_run_id.to_string()],|r|r.get(0))?;
            let schema:annotagent_core::ProjectSchema=serde_json::from_str(&schema)?;
            let tasks=schema.tasks.iter().filter(|t|t.kind==TaskKind::BoundingBox&&t.labels.iter().any(|l|l.as_str()==input.label)).collect::<Vec<_>>();
            if tasks.len()!=1{return Err(invalid("The source Run must have one unambiguous bbox task for this label"));}
            let annotation=Annotation{id:AnnotationId(Uuid::new_v5(&input.command_id,b"delivery-missing-object")),image_id:image,task_id:tasks[0].id.clone(),label:Some(input.label.as_str().into()),value:input.value.clone(),attributes:std::collections::BTreeMap::default(),confidence:None,source:AnnotationSource::Human,review_status:ReviewStatus::NeedsReview,provenance:annotagent_core::AnnotationProvenance::default(),created_at:chrono::Utc::now()};
            annotation.validate().map_err(|_|invalid("Invalid manual annotation"))?;
            let revision=AnnotationRevision{revision_id:AnnotationRevisionId(input.command_id),annotation_id:annotation.id,parent_revision_id:None,before:None,after:Some(annotation.snapshot()),actor:RevisionActor::Human,reason:Some(reason),created_at:annotation.created_at};
            revision.validate().map_err(|_|invalid("Invalid creation revision"))?;
            tx.execute("INSERT INTO annotations(id,run_id,image_id,task_id,label,review_status,annotation_json,created_at) VALUES(?1,?2,?3,?4,?5,'needs_review',?6,?7)",params![annotation.id.to_string(),input.source_run_id.to_string(),image.to_string(),annotation.task_id.as_str(),input.label,serde_json::to_string(&annotation)?,annotation.created_at.to_rfc3339()])?;
            tx.execute("INSERT INTO annotation_revisions(revision_id,annotation_id,parent_revision_id,revision_json,created_at) VALUES(?1,?2,NULL,?3,?4)",params![revision.revision_id.to_string(),annotation.id.to_string(),serde_json::to_string(&revision)?,revision.created_at.to_rfc3339()])?;
            tx.execute("INSERT INTO review_queue(run_id,annotation_id,status,reasons_json,created_at) VALUES(?1,?2,'pending','[\"manual_missing_object\"]',?3)",params![input.source_run_id.to_string(),annotation.id.to_string(),annotation.created_at.to_rfc3339()])?;
            tx.commit()?;Ok(revision)
        })
    }
    /// Existing revision writer plus same-transaction delivery snapshot checks.
    pub fn edit_delivery_object(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        input: &DeliveryObjectEdit,
    ) -> Result<annotagent_core::AnnotationRevision, StorageError> {
        if !matches!(
            input.review_status,
            ReviewStatus::NeedsReview | ReviewStatus::HumanAccepted | ReviewStatus::Rejected
        ) || !matches!(
            input.value,
            annotagent_core::AnnotationValue::BoundingBox { .. }
        ) || input.reason.trim().is_empty()
            || input.reason.len() > 2000
        {
            return Err(invalid(
                "Object edit requires a bbox, explicit human review state and bounded reason",
            ));
        }
        let initial = self.delivery_image_snapshot(
            project,
            conversation,
            task,
            image,
            Some(input.source_run_id),
        )?;
        let mut annotation = initial
            .annotations
            .into_iter()
            .find(|a| a.id == input.annotation_id)
            .ok_or_else(|| invalid("Object is not in this owned image and source Run"))?;
        annotation.label = Some(input.label.as_str().into());
        annotation.value = input.value.clone();
        annotation.review_status = input.review_status;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                project,
                conversation,
                task,
                image,
                input
            ))?)
        );
        let reason = format!("{} [delivery request {fingerprint}]", input.reason);
        self.update_annotation_guarded(
            &annotation,
            Some(&reason),
            Some(annotagent_core::AnnotationRevisionId(input.command_id)),
            |db| {
                let saved = intent(db, project, conversation, task)?;
                let existing: Option<String> = db
                    .query_row(
                        "SELECT revision_json FROM annotation_revisions WHERE revision_id=?1",
                        [input.command_id.to_string()],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(json) = existing {
                    let revision: annotagent_core::AnnotationRevision =
                        serde_json::from_str(&json)?;
                    if revision.annotation_id != input.annotation_id
                        || revision.reason.as_deref() != Some(reason.as_str())
                    {
                        return Err(invalid(
                            "Object edit command conflicts with its immutable saved request",
                        ));
                    }
                    return Ok(Some(revision));
                }
                if saved.revision != input.intent_revision
                    || saved.content_sha256 != input.intent_sha256
                {
                    return Err(invalid("Delivery intent changed before object save"));
                }
                if !saved
                    .intent
                    .label_spec
                    .as_ref()
                    .is_some_and(|labels| labels.iter().any(|l| l.stable_id == input.label))
                {
                    return Err(invalid(
                        "Edited object label is outside the delivery Schema",
                    ));
                }
                let current = snapshot(db, &saved, image, Some(input.source_run_id))?;
                if current.sha256 != input.expected_snapshot_sha256 {
                    return Err(invalid(
                        "Image annotations changed; local object edit was not applied",
                    ));
                }
                Ok(None)
            },
        )
    }
    /// Explicit selectable formal sources, never an implicit latest-Run projection.
    pub fn delivery_run_sources(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
    ) -> Result<Vec<DeliveryRunSource>, StorageError> {
        self.with_connection(|db| {
            let saved = intent(db, project, conversation, task)?;
            // Reuse ownership, scope and current image-version checks.
            snapshot(db, &saved, image, None)?;
            let mut stmt = db.prepare("SELECT p.id,b.batch_id,r.id,r.model,r.status,r.created_at FROM runs r JOIN batch_images b ON b.child_run_id=r.id JOIN processing_operations p ON p.id=b.batch_id WHERE r.project_id=?1 AND b.image_id=?2 AND json_extract(p.state_json,'$.authorization.conversation.conversation_id')=?3 AND json_extract(p.state_json,'$.authorization.conversation.task_id')=?4 AND json_extract(p.state_json,'$.authorization.delivery_scope.intent_revision')=?5 AND json_extract(p.state_json,'$.authorization.delivery_scope.intent_sha256')=?6 AND r.status IN ('completed','completed_with_review','partial') ORDER BY r.created_at DESC,r.id")?;
            stmt.query_map(params![project, image.to_string(), conversation.to_string(), task.to_string(), saved.revision, saved.content_sha256], |r| Ok(DeliveryRunSource {
                processing_operation_id:r.get(0)?,batch_id:r.get(1)?,run_id: r.get(2)?, model: r.get(3)?, status: r.get(4)?, created_at: r.get(5)?,
            }))?.collect::<Result<Vec<_>,_>>().map_err(Into::into)
        })
    }
    pub fn delivery_image_snapshot(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        image: ImageId,
        run: Option<RunId>,
    ) -> Result<DeliveryImageSnapshot, StorageError> {
        self.with_connection(|db| {
            snapshot(db, &intent(db, project, conversation, task)?, image, run)
        })
    }

    /// Current-intent receipts only. Consumers must recheck snapshot hashes for readiness.
    pub fn delivery_image_reviews(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<DeliveryImageReview>, StorageError> {
        self.with_connection(|db| {
            let saved = intent(db,project,conversation,task)?;
            let mut stmt = db.prepare("SELECT r.revision,r.input_json,r.snapshot_json,r.created_at FROM delivery_image_reviews r WHERE r.task_id=?1 AND r.intent_revision=?2 AND r.revision=(SELECT MAX(x.revision) FROM delivery_image_reviews x WHERE x.task_id=r.task_id AND x.intent_revision=r.intent_revision AND x.image_id=r.image_id) ORDER BY r.image_id")?;
            stmt.query_map(params![task.to_string(),saved.revision],receipt)?.map(|r| decode(r?)).collect()
        })
    }

    /// This command accepts no annotation bodies, paths or caller-supplied confirmation IDs.
    pub fn confirm_delivery_image(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &DeliveryImageReviewInput,
    ) -> Result<DeliveryImageReview, StorageError> {
        if !input.confirmed || input.command_id.is_nil() {
            return Err(invalid(
                "explicit whole-image confirmation and command ID are required",
            ));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let saved = intent(&tx,project,conversation,task)?;
            if let Some(row) = tx.query_row("SELECT revision,input_json,snapshot_json,created_at FROM delivery_image_reviews WHERE task_id=?1 AND command_id=?2",params![task.to_string(),input.command_id.to_string()],receipt).optional()? {
                let old = decode(row)?;
                if old.input != *input { return Err(invalid("whole-image confirmation retry changed its scope")); }
                return Ok(old);
            }
            if !saved.intent.missing_slots().is_empty() || saved.revision != input.intent_revision || saved.content_sha256 != input.intent_sha256 { return Err(invalid("delivery intent changed or is incomplete; reload before confirming")); }
            let current: u32 = tx.query_row("SELECT COALESCE(MAX(revision),0) FROM delivery_image_reviews WHERE task_id=?1 AND intent_revision=?2 AND image_id=?3",params![task.to_string(),saved.revision,input.image_id.to_string()],|r| r.get(0))?;
            if current != input.expected_review_revision { return Err(invalid("whole-image review changed; reload before confirming")); }
            let snapshot = snapshot(&tx,&saved,input.image_id,input.source_run_id)?;
            if snapshot.sha256 != input.expected_snapshot_sha256 { return Err(invalid("image annotations changed; inspect the latest image before confirming")); }
            let accepted = snapshot.annotations.iter().filter(|a| a.review_status == ReviewStatus::HumanAccepted).count();
            let unresolved = snapshot.annotations.iter().any(|a| !matches!(a.review_status,ReviewStatus::HumanAccepted | ReviewStatus::Rejected));
            match input.decision {
                DeliveryImageDecision::PositiveComplete if input.source_run_id.is_none() || accepted == 0 || unresolved => return Err(invalid("positive image requires an explicit source Run and resolved accepted objects")),
                DeliveryImageDecision::NegativeConfirmed if accepted != 0 || unresolved => return Err(invalid("negative image still contains accepted or unresolved objects")),
                DeliveryImageDecision::Excluded if input.reason.as_ref().is_none_or(|s| s.trim().is_empty()) => return Err(invalid("excluding an image requires a reason")),
                _ => {}
            }
            // Some historical rows predate revision tracking. Record a truthful present-day
            // import baseline, not a fabricated earlier human acceptance or geometry edit.
            for annotation in snapshot.annotations.iter().filter(|a|a.review_status==ReviewStatus::HumanAccepted) {
                let has_revision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM annotation_revisions WHERE annotation_id=?1)",[annotation.id.to_string()],|r|r.get(0))?;
                if !has_revision {
                    let baseline=annotagent_core::AnnotationRevision {
                        revision_id:annotagent_core::AnnotationRevisionId::new(),annotation_id:annotation.id,parent_revision_id:None,
                        before:None,after:Some(annotation.snapshot()),actor:annotagent_core::RevisionActor::Import,
                        reason:Some("Existing annotation snapshot registered at explicit whole-image delivery confirmation; earlier revision history unavailable.".into()),created_at:chrono::Utc::now(),
                    };
                    baseline.validate().map_err(|_|invalid("Cannot record annotation revision baseline"))?;
                    tx.execute("INSERT INTO annotation_revisions(revision_id,annotation_id,parent_revision_id,revision_json,created_at) VALUES(?1,?2,NULL,?3,?4)",params![baseline.revision_id.to_string(),annotation.id.to_string(),serde_json::to_string(&baseline)?,baseline.created_at.to_rfc3339()])?;
                }
            }
            let result = DeliveryImageReview { revision:current.checked_add(1).ok_or_else(|| invalid("review revision overflow"))?, input:input.clone(), snapshot, created_at:chrono::Utc::now().to_rfc3339() };
            tx.execute("INSERT INTO delivery_image_reviews(task_id,intent_revision,image_id,revision,command_id,input_json,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![task.to_string(),saved.revision,input.image_id.to_string(),result.revision,input.command_id.to_string(),serde_json::to_string(input)?,serde_json::to_string(&result.snapshot)?,result.created_at])?;
            tx.commit()?;
            Ok(result)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BeginConversationTask, ConversationMessageInput, ConversationSelectionRef,
        ConversationSendDisposition, ConversationSendInput, ConversationSendMode,
    };
    use annotagent_core::{
        AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, NormalizedRect,
        TaskKind, dataset_delivery::*,
    };

    struct TestData {
        store: SqliteStore,
        path: std::path::PathBuf,
        saved: TaskDeliveryRevision,
        image: ImageId,
        run: RunId,
        annotation: Annotation,
    }
    #[test]
    fn missing_object_is_a_scoped_idempotent_human_revision_not_whole_image_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let f = TestData::new(root.path());
        f.put(&f.annotation);
        let before = f.input(DeliveryImageDecision::PositiveComplete);
        f.confirm(&before).unwrap();
        let input = DeliveryObjectCreate {
            command_id: Uuid::new_v4(),
            intent_revision: f.saved.revision,
            intent_sha256: f.saved.content_sha256.clone(),
            source_run_id: f.run,
            expected_snapshot_sha256: before.expected_snapshot_sha256,
            label: "target".into(),
            value: f.annotation.value.clone(),
            reason: "TEST manually checked missing object".into(),
        };
        let i = &f.saved.intent;
        let create = |input: &DeliveryObjectCreate| {
            f.store.create_delivery_object(
                &i.project_id,
                i.conversation_id,
                i.task_id,
                f.image,
                input,
            )
        };
        let revision = create(&input).unwrap();
        assert!(revision.before.is_none());
        assert_eq!(revision, create(&input).unwrap());
        let snapshot = f
            .store
            .delivery_image_snapshot(
                &i.project_id,
                i.conversation_id,
                i.task_id,
                f.image,
                Some(f.run),
            )
            .unwrap();
        assert_eq!(snapshot.annotations.len(), 2);
        assert_ne!(snapshot.sha256, input.expected_snapshot_sha256);
        let added = snapshot
            .annotations
            .iter()
            .find(|a| a.id == revision.annotation_id)
            .unwrap();
        assert_eq!(added.source, AnnotationSource::Human);
        assert_eq!(added.confidence, None);
        assert_eq!(added.review_status, ReviewStatus::NeedsReview);
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
                .is_err()
        );
        let mut changed = input.clone();
        changed.reason = "changed".into();
        assert!(create(&changed).is_err());
        changed.command_id = Uuid::new_v4();
        assert!(create(&changed).is_err());
        changed.expected_snapshot_sha256 = snapshot.sha256;
        changed.label = "not-a-label".into();
        assert!(create(&changed).is_err());
        assert!(
            f.store
                .create_delivery_object(
                    "foreign-project",
                    i.conversation_id,
                    i.task_id,
                    f.image,
                    &input
                )
                .is_err()
        );
    }
    #[test]
    fn formal_sources_are_explicit_owned_terminal_and_image_scoped() {
        let root = tempfile::tempdir().unwrap();
        let f = TestData::new(root.path());
        let i = &f.saved.intent;
        let sources = || {
            f.store
                .delivery_run_sources(&i.project_id, i.conversation_id, i.task_id, f.image)
        };
        let found = sources().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].run_id, f.run.to_string());
        assert!(
            f.store
                .delivery_run_sources("foreign", i.conversation_id, i.task_id, f.image)
                .is_err()
        );
        assert!(
            f.store
                .delivery_run_sources(&i.project_id, i.conversation_id, i.task_id, ImageId::new())
                .is_err()
        );
        f.store
            .with_connection(|db| {
                db.execute(
                    "UPDATE runs SET status='running' WHERE id=?1",
                    [f.run.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(sources().unwrap().is_empty());
        f.store
            .with_connection(|db| {
                db.execute(
                    "UPDATE runs SET status='completed',project_id='foreign' WHERE id=?1",
                    [f.run.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(sources().unwrap().is_empty());
    }
    #[test]
    fn formal_annotation_message_requires_current_full_lineage_and_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let f = TestData::new(root.path());
        f.put(&f.annotation);
        let confirmed = f
            .confirm(&f.input(DeliveryImageDecision::PositiveComplete))
            .unwrap();
        let i = &f.saved.intent;
        let source = &f
            .store
            .delivery_run_sources(&i.project_id, i.conversation_id, i.task_id, f.image)
            .unwrap()[0];
        let revision = f
            .store
            .list_revisions(f.annotation.id)
            .unwrap()
            .pop()
            .unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST this formal box is too wide".into(),
            image: Some(crate::ConversationImageRef {
                image_id: f.image.to_string(),
                sha256: "a".repeat(64),
            }),
            reference: Some(ConversationSelectionRef::FormalAnnotation {
                task_id: i.task_id,
                project_schema_revision: "a".repeat(64),
                intent_revision: f.saved.revision,
                intent_sha256: f.saved.content_sha256.clone(),
                processing_operation_id: source.processing_operation_id.parse().unwrap(),
                batch_id: source.batch_id.parse().unwrap(),
                source_run_id: f.run,
                annotation_id: f.annotation.id,
                annotation_revision_id: revision.revision_id,
                expected_snapshot_sha256: confirmed.snapshot.sha256,
            }),
        };
        let send = ConversationSendInput {
            message: message.clone(),
            task_id: Some(i.task_id),
            schema_revision: "a".repeat(64),
            agent_model: None,
            mode: Some(ConversationSendMode::Plan),
        };
        let saved = f
            .store
            .send_conversation_message(&i.project_id, i.conversation_id, &send)
            .unwrap();
        assert_eq!(
            saved.disposition,
            ConversationSendDisposition::FormalFeedback
        );
        assert_eq!(
            saved,
            f.store
                .send_conversation_message(&i.project_id, i.conversation_id, &send)
                .unwrap()
        );
        let mut stale = message;
        stale.id = Uuid::new_v4();
        if let Some(ConversationSelectionRef::FormalAnnotation {
            processing_operation_id,
            ..
        }) = &mut stale.reference
        {
            *processing_operation_id = Uuid::new_v4();
        }
        let stale = ConversationSendInput {
            message: stale,
            task_id: Some(i.task_id),
            schema_revision: "a".repeat(64),
            agent_model: None,
            mode: Some(ConversationSendMode::Plan),
        };
        assert!(
            f.store
                .send_conversation_message(&i.project_id, i.conversation_id, &stale)
                .is_err()
        );
    }
    #[test]
    fn object_edits_use_snapshot_cas_and_existing_revision_history() {
        let root = tempfile::tempdir().unwrap();
        let f = TestData::new(root.path());
        f.put(&f.annotation);
        let i = &f.saved.intent;
        let confirmed = f
            .confirm(&f.input(DeliveryImageDecision::PositiveComplete))
            .unwrap();
        let input = DeliveryObjectEdit {
            command_id: Uuid::new_v4(),
            intent_revision: f.saved.revision,
            intent_sha256: f.saved.content_sha256.clone(),
            source_run_id: f.run,
            annotation_id: f.annotation.id,
            expected_snapshot_sha256: confirmed.snapshot.sha256.clone(),
            label: "target".into(),
            value: annotagent_core::AnnotationValue::BoundingBox {
                rect: annotagent_core::NormalizedRect::new(0.12, 0.12, 0.16, 0.16).unwrap(),
            },
            review_status: ReviewStatus::NeedsReview,
            reason: "TEST tighten boundary".into(),
        };
        let edit = |input: &DeliveryObjectEdit| {
            f.store.edit_delivery_object(
                &i.project_id,
                i.conversation_id,
                i.task_id,
                f.image,
                input,
            )
        };
        let result = edit(&input).unwrap();
        assert_eq!(result.revision_id.0, input.command_id);
        assert_eq!(edit(&input).unwrap(), result);
        let mut changed = input.clone();
        changed.reason = "different request".into();
        assert!(edit(&changed).is_err());
        changed.command_id = Uuid::new_v4();
        assert!(
            edit(&changed)
                .unwrap_err()
                .to_string()
                .contains("annotations changed")
        );
        assert!(
            f.store
                .edit_delivery_object("foreign", i.conversation_id, i.task_id, f.image, &input)
                .is_err()
        );
        let current = f
            .store
            .delivery_image_snapshot(
                &i.project_id,
                i.conversation_id,
                i.task_id,
                f.image,
                Some(f.run),
            )
            .unwrap();
        assert_ne!(current.sha256, confirmed.snapshot.sha256);
        assert_eq!(current.annotations[0].value, input.value);
        assert_eq!(
            current.annotations[0].review_status,
            ReviewStatus::NeedsReview
        );
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
                .is_err()
        );
        changed.expected_snapshot_sha256 = current.sha256;
        changed.label = "outside-schema".into();
        assert!(
            edit(&changed)
                .unwrap_err()
                .to_string()
                .contains("outside the delivery Schema")
        );
    }
    impl TestData {
        fn new(root: &std::path::Path) -> Self {
            let path = root.join("TEST-whole-image.db");
            let store = SqliteStore::open(&path).unwrap();
            let project = Uuid::new_v4().to_string();
            let conversation = store.create_conversation(&project).unwrap();
            let message = ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST whole image".into(),
                reference: None,
                image: None,
            };
            store
                .append_conversation_message(&project, conversation, &message)
                .unwrap();
            let task = Uuid::new_v4();
            store
                .begin_conversation_task(
                    &project,
                    conversation,
                    &BeginConversationTask {
                        id: task,
                        source_message_id: message.id,
                        schema_revision: "a".repeat(64),
                    },
                )
                .unwrap();
            let image = ImageId::new();
            let run = RunId::new();
            let intent = TaskDeliveryIntent {
                version: 1,
                project_id: project.clone(),
                conversation_id: conversation,
                task_id: task,
                dataset_scope: Some(vec![DeliveryImage {
                    image_id: image,
                    content_sha256: "a".repeat(64),
                    content_revision: "TEST-1".into(),
                    existing_split: None,
                    group_ids: vec![],
                }]),
                label_spec: Some(vec![DeliveryLabel {
                    stable_id: "target".into(),
                    display_name: "目标".into(),
                    aliases: vec![],
                    include: String::new(),
                    exclude: String::new(),
                }]),
                training_target: Some(TrainingTarget {
                    annotation_kind: TaskKind::BoundingBox,
                    framework: "ultralytics".into(),
                    export_profile: DETECTION_PROFILE.into(),
                    profile_revision: 1,
                }),
                split_policy: DeliverySplitPolicy::default(),
                review_policy: DeliveryReviewPolicy::HumanWholeImage,
            };
            let saved = store
                .save_task_delivery_intent(Uuid::new_v4(), 0, &intent)
                .unwrap();
            let schema=annotagent_core::ProjectSchema::from_yaml("version: 1\nproject:\n  name: TEST\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target]\nreview:\n  auto_accept_confidence: 0.99\n  force_review_below: 0.95\nexport:\n  formats: [native]\n").unwrap();
            let processing = Uuid::new_v4();
            let processing_state = serde_json::json!({
                "id":processing,"batch_id":processing,"phase":"started",
                "authorization":{
                    "conversation":{"conversation_id":conversation,"task_id":task},
                    "delivery_scope":{"intent_revision":saved.revision,"intent_sha256":saved.content_sha256},
                    "images":[{"image_id":image,"content_hash":"a".repeat(64)}]
                }
            });
            store.with_connection(|db| {
                db.execute("INSERT INTO images(id,project_id,relative_path,sha256,metadata_json,imported_at) VALUES(?1,?2,'TEST.png',?3,'{}','TEST')",params![image.to_string(),project,"a".repeat(64)])?;
                db.execute("INSERT INTO runs(id,project_id,project_name,skill_id,provider,model,status,project_schema_json,created_at,updated_at) VALUES(?1,?2,'TEST','TEST','TEST','TEST','completed',?3,'TEST','TEST')",params![run.to_string(),project,serde_json::to_string(&schema)?])?;
                db.execute("INSERT INTO run_images(run_id,image_id,status) VALUES(?1,?2,'completed')",params![run.to_string(),image.to_string()])?;
                db.execute("INSERT INTO processing_operations(id,project_id,request_json,state_json,created_at,updated_at) VALUES(?1,'TEST-route','{}',?2,'TEST','TEST')",params![processing.to_string(),serde_json::to_string(&processing_state)?])?;
                db.execute("INSERT INTO dataset_batches(id,project_id,project_path,provider,status,max_concurrency,workflow_version,workflow_snapshot_json,project_snapshot_json,budget_limits_json,budget_ledger_json,event_sequence,created_at,updated_at) VALUES(?1,'TEST-route','TEST','TEST','completed',1,'TEST','{}','{}','{}','{}',0,'TEST','TEST')",[processing.to_string()])?;
                db.execute("INSERT INTO batch_images(batch_id,image_id,image_path,position,status,child_run_id,attempt_count,reservation_json,actual_usage_json,checkpoint_json,updated_at) VALUES(?1,?2,'TEST.png',0,'completed',?3,0,'{}','{}','{}','TEST')",params![processing.to_string(),image.to_string(),run.to_string()])?;
                Ok(())
            }).unwrap();
            let annotation = Annotation {
                id: AnnotationId::new(),
                image_id: image,
                task_id: "objects".into(),
                label: Some("target".into()),
                value: AnnotationValue::BoundingBox {
                    rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
                },
                attributes: std::collections::BTreeMap::default(),
                confidence: None,
                source: AnnotationSource::Human,
                review_status: ReviewStatus::HumanAccepted,
                provenance: AnnotationProvenance::default(),
                created_at: chrono::Utc::now(),
            };
            Self {
                store,
                path,
                saved,
                image,
                run,
                annotation,
            }
        }
        fn put(&self, annotation: &Annotation) {
            self.store.with_connection(|db| {
                db.execute("INSERT OR REPLACE INTO annotations(id,run_id,image_id,task_id,label,review_status,annotation_json,created_at) VALUES(?1,?2,?3,'objects','target',?4,?5,'TEST')",params![annotation.id.to_string(),self.run.to_string(),self.image.to_string(),serde_json::to_string(&annotation.review_status)?,serde_json::to_string(annotation)?])?;
                Ok(())
            }).unwrap();
        }
        fn input(&self, decision: DeliveryImageDecision) -> DeliveryImageReviewInput {
            let i = &self.saved.intent;
            let snapshot = self
                .store
                .delivery_image_snapshot(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    self.image,
                    Some(self.run),
                )
                .unwrap();
            DeliveryImageReviewInput {
                command_id: Uuid::new_v4(),
                intent_revision: self.saved.revision,
                intent_sha256: self.saved.content_sha256.clone(),
                image_id: self.image,
                source_run_id: Some(self.run),
                expected_snapshot_sha256: snapshot.sha256,
                expected_review_revision: 0,
                decision,
                reason: None,
                confirmed: true,
            }
        }
        fn confirm(
            &self,
            input: &DeliveryImageReviewInput,
        ) -> Result<DeliveryImageReview, StorageError> {
            let i = &self.saved.intent;
            self.store
                .confirm_delivery_image(&i.project_id, i.conversation_id, i.task_id, input)
        }
    }

    #[test]
    fn whole_image_requires_explicit_confirmation_and_resolved_objects() {
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        // Empty model output is not a saved negative decision.
        assert!(
            f.store
                .delivery_image_reviews(&i.project_id, i.conversation_id, i.task_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
                .is_err()
        );
        f.put(&f.annotation);
        // Object acceptance itself still creates no image receipt.
        assert!(
            f.store
                .delivery_image_reviews(&i.project_id, i.conversation_id, i.task_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::NegativeConfirmed))
                .is_err()
        );
        let mut pending = f.annotation.clone();
        pending.id = AnnotationId::new();
        pending.review_status = ReviewStatus::NeedsReview;
        f.put(&pending);
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
                .is_err()
        );
        pending.review_status = ReviewStatus::Rejected;
        f.put(&pending);
        let mut input = f.input(DeliveryImageDecision::PositiveComplete);
        input.confirmed = false;
        assert!(f.confirm(&input).is_err());
        input.confirmed = true;
        let receipt = f.confirm(&input).unwrap();
        assert_eq!(receipt.snapshot.annotations.len(), 2);
        assert_eq!(f.confirm(&input).unwrap(), receipt);
        input.reason = Some("changed retry".into());
        assert!(f.confirm(&input).is_err());
        assert!(
            f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
                .is_err()
        );
        let restored = SqliteStore::open(&f.path)
            .unwrap()
            .delivery_image_reviews(&i.project_id, i.conversation_id, i.task_id)
            .unwrap();
        assert_eq!(restored, vec![receipt]);
    }

    #[test]
    fn whole_image_rejects_stale_snapshots_owners_and_scope_and_preserves_old_receipts() {
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        let empty = f.input(DeliveryImageDecision::NegativeConfirmed);
        f.put(&f.annotation);
        assert!(f.confirm(&empty).is_err());
        let input = f.input(DeliveryImageDecision::PositiveComplete);
        assert!(
            f.store
                .confirm_delivery_image(
                    &Uuid::new_v4().to_string(),
                    i.conversation_id,
                    i.task_id,
                    &input
                )
                .is_err()
        );
        let mut foreign = input.clone();
        foreign.source_run_id = Some(RunId::new());
        assert!(f.confirm(&foreign).is_err());
        foreign = input.clone();
        foreign.image_id = ImageId::new();
        assert!(f.confirm(&foreign).is_err());
        let receipt = f.confirm(&input).unwrap();
        let mut changed = i.clone();
        changed.label_spec.as_mut().unwrap()[0].display_name = "Changed".into();
        f.store
            .save_task_delivery_intent(Uuid::new_v4(), 1, &changed)
            .unwrap();
        assert!(
            f.store
                .delivery_image_reviews(&i.project_id, i.conversation_id, i.task_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(f.confirm(&input).unwrap(), receipt); // immutable command receipt, not current readiness
        let mut stale = input;
        stale.command_id = Uuid::new_v4();
        assert!(f.confirm(&stale).is_err());
    }

    #[test]
    fn negative_and_exclusion_are_deliberate_and_image_byte_changes_invalidate_review() {
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let mut input = f.input(DeliveryImageDecision::Excluded);
        assert!(f.confirm(&input).is_err());
        input.reason = Some("TEST incomplete: missing target cannot be added yet".into());
        let excluded = f.confirm(&input).unwrap();
        assert_eq!(excluded.input.decision, DeliveryImageDecision::Excluded);
        let mut negative = f.input(DeliveryImageDecision::NegativeConfirmed);
        negative.expected_review_revision = 1;
        assert_eq!(f.confirm(&negative).unwrap().revision, 2);
        f.store
            .with_connection(|db| {
                db.execute(
                    "UPDATE images SET sha256=?1 WHERE id=?2",
                    params!["b".repeat(64), f.image.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        negative.command_id = Uuid::new_v4();
        negative.expected_review_revision = 2;
        assert!(f.confirm(&negative).is_err());
    }

    #[test]
    fn package_freezes_reviews_retries_and_existing_export_history() {
        use crate::{DeliveryPackageInput, DeliveryPackagePhase};
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        let mut input = DeliveryPackageInput {
            command_id: Uuid::new_v4(),
            intent_revision: 1,
            intent_sha256: f.saved.content_sha256.clone(),
            image_reviews: std::collections::BTreeMap::from([(f.image, 1)]),
            confirmed: true,
        };
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        f.put(&f.annotation);
        f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
            .unwrap();
        let (frozen, created) = f
            .store
            .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
            .unwrap();
        assert!(created);
        assert_eq!(frozen.phase, DeliveryPackagePhase::Preparing);
        assert_eq!(
            frozen.snapshot.images[0].review.snapshot.annotations[0],
            f.annotation
        );
        assert!(frozen.snapshot.images[0].source_evidence_sha256.is_some());
        assert_eq!(frozen.snapshot.images[0].annotation_revision_ids.len(), 1);
        let baseline: annotagent_core::AnnotationRevision = f
            .store
            .with_connection(|db| {
                let json: String = db.query_row(
                    "SELECT revision_json FROM annotation_revisions WHERE annotation_id=?1",
                    [f.annotation.id.to_string()],
                    |r| r.get(0),
                )?;
                Ok(serde_json::from_str(&json)?)
            })
            .unwrap();
        assert_eq!(baseline.actor, annotagent_core::RevisionActor::Import);
        assert!(baseline.before.is_none());
        assert_eq!(baseline.after, Some(f.annotation.snapshot()));
        assert!(
            baseline
                .reason
                .unwrap()
                .contains("earlier revision history unavailable")
        );
        assert_eq!(
            f.store
                .conversation_exports(&i.project_id, i.conversation_id, i.task_id)
                .unwrap()[0]
                .id,
            input.command_id
        );
        let reopened = SqliteStore::open(&f.path).unwrap();
        let (retry, created) = reopened
            .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
            .unwrap();
        assert!(!created);
        assert_eq!(retry.snapshot_sha256, frozen.snapshot_sha256);
        // New object changes are never incorporated into the already frozen package.
        let mut changed = f.annotation.clone();
        changed.review_status = ReviewStatus::NeedsReview;
        f.put(&changed);
        assert_eq!(
            f.store
                .delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    input.command_id
                )
                .unwrap()
                .snapshot
                .images[0]
                .review
                .snapshot
                .annotations[0],
            f.annotation
        );
        let old_id = input.command_id;
        input.command_id = Uuid::new_v4();
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        input.command_id = old_id;
        input.image_reviews.clear();
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        assert!(
            f.store
                .delivery_package("foreign", i.conversation_id, i.task_id, old_id)
                .is_err()
        );
    }

    #[test]
    fn automatic_package_permission_is_one_shot_and_consumed_only_with_ready_snapshot() {
        use crate::{DeliveryPackageConsentInput, DeliveryPackageInput};
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        let grant = DeliveryPackageConsentInput {
            id: Uuid::new_v4(),
            intent_revision: 1,
            intent_sha256: f.saved.content_sha256.clone(),
            confirmed: true,
        };
        let authorize = |g: &DeliveryPackageConsentInput| {
            f.store
                .authorize_delivery_package(&i.project_id, i.conversation_id, i.task_id, g)
        };
        let read = || {
            f.store
                .delivery_package_consent(&i.project_id, i.conversation_id, i.task_id, grant.id)
                .unwrap()
        };
        assert_eq!(authorize(&grant).unwrap().state, "armed");
        assert_eq!(authorize(&grant).unwrap().input, grant);
        let reopened = SqliteStore::open(dir.path().join("TEST-whole-image.db")).unwrap();
        assert_eq!(
            reopened
                .delivery_package_consent(&i.project_id, i.conversation_id, i.task_id, grant.id)
                .unwrap()
                .state,
            "armed"
        );
        let mut changed = grant.clone();
        changed.intent_revision = 2;
        assert!(authorize(&changed).is_err());
        changed = grant.clone();
        changed.id = Uuid::new_v4();
        assert!(authorize(&changed).is_err());
        let input = DeliveryPackageInput {
            command_id: grant.id,
            intent_revision: 1,
            intent_sha256: grant.intent_sha256.clone(),
            image_reviews: std::collections::BTreeMap::from([(f.image, 1)]),
            confirmed: true,
        };
        assert!(
            f.store
                .begin_authorized_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    &input
                )
                .is_err()
        );
        assert_eq!(read().state, "armed");
        f.put(&f.annotation);
        f.confirm(&f.input(DeliveryImageDecision::PositiveComplete))
            .unwrap();
        let (_, created) = f
            .store
            .begin_authorized_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
            .unwrap();
        assert!(created);
        assert_eq!(read().state, "consumed");
        assert!(
            !f.store
                .begin_authorized_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    &input
                )
                .unwrap()
                .1
        );
        assert!(
            f.store
                .cancel_delivery_package_consent(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    grant.id
                )
                .is_err()
        );
        assert!(
            f.store
                .delivery_package_consent(
                    &Uuid::new_v4().to_string(),
                    i.conversation_id,
                    i.task_id,
                    grant.id
                )
                .is_err()
        );
        let mut cancelled = grant.clone();
        cancelled.id = Uuid::new_v4();
        authorize(&cancelled).unwrap();
        f.store
            .cancel_delivery_package_consent(
                &i.project_id,
                i.conversation_id,
                i.task_id,
                cancelled.id,
            )
            .unwrap();
        assert_eq!(authorize(&cancelled).unwrap().state, "cancelled");
        let mut cancelled_input = input;
        cancelled_input.command_id = cancelled.id;
        assert!(
            f.store
                .begin_authorized_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    &cancelled_input
                )
                .is_err()
        );
    }

    #[test]
    fn package_cancel_and_worker_claim_are_terminal_cas_and_not_generic_success() {
        use crate::{DeliveryPackageInput, DeliveryPackagePhase as Phase};
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        f.confirm(&f.input(DeliveryImageDecision::NegativeConfirmed))
            .unwrap();
        let input = DeliveryPackageInput {
            command_id: Uuid::new_v4(),
            intent_revision: 1,
            intent_sha256: f.saved.content_sha256.clone(),
            image_reviews: std::collections::BTreeMap::from([(f.image, 1)]),
            confirmed: true,
        };
        f.store
            .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
            .unwrap();
        let id = input.command_id;
        let result = serde_json::json!({"TEST":"not a real package"});
        assert!(
            f.store
                .finish_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Ok(&result),
                    false
                )
                .is_err()
        );
        f.store
            .finish_conversation_export(id, Some(&result), None)
            .unwrap();
        assert!(
            f.store
                .conversation_export(&i.project_id, i.conversation_id, i.task_id, id)
                .unwrap()
                .result
                .is_none()
        );
        assert!(
            f.store
                .advance_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Phase::Preparing,
                    Phase::Exporting
                )
                .unwrap()
        );
        assert!(
            !f.store
                .advance_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Phase::Preparing,
                    Phase::Exporting
                )
                .unwrap()
        );
        assert!(
            f.store
                .finish_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Err("TEST user cancelled"),
                    true
                )
                .unwrap()
        );
        assert!(
            !f.store
                .advance_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Phase::Exporting,
                    Phase::Validating
                )
                .unwrap()
        );
        assert!(
            !f.store
                .finish_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Ok(&result),
                    false
                )
                .unwrap()
        );
        let job = f
            .store
            .delivery_package(&i.project_id, i.conversation_id, i.task_id, id)
            .unwrap();
        assert_eq!(job.phase, Phase::Cancelled);
        assert!(job.result.is_none());
        assert_eq!(
            f.store
                .conversation_export_events(&i.project_id, i.conversation_id, i.task_id, 0)
                .unwrap()
                .1
                .into_iter()
                .map(|e| e.kind)
                .collect::<Vec<_>>(),
            vec!["requested", "failed"]
        );
    }

    #[test]
    fn package_scope_and_review_revision_must_match_and_ready_is_immutable() {
        use crate::{DeliveryPackageInput, DeliveryPackagePhase as Phase};
        let dir = tempfile::tempdir().unwrap();
        let f = TestData::new(dir.path());
        let i = &f.saved.intent;
        f.confirm(&f.input(DeliveryImageDecision::NegativeConfirmed))
            .unwrap();
        let mut input = DeliveryPackageInput {
            command_id: Uuid::new_v4(),
            intent_revision: 1,
            intent_sha256: f.saved.content_sha256.clone(),
            image_reviews: std::collections::BTreeMap::new(),
            confirmed: true,
        };
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        input.image_reviews.insert(f.image, 2);
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        input.image_reviews.insert(f.image, 1);
        input.confirmed = false;
        assert!(
            f.store
                .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
                .is_err()
        );
        input.confirmed = true;
        f.store
            .begin_delivery_package(&i.project_id, i.conversation_id, i.task_id, &input)
            .unwrap();
        let id = input.command_id;
        assert!(
            f.store
                .advance_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Phase::Preparing,
                    Phase::Exporting
                )
                .unwrap()
        );
        assert!(
            f.store
                .advance_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Phase::Exporting,
                    Phase::Validating
                )
                .unwrap()
        );
        let result = serde_json::json!({"TEST":"storage transition only, no actual ZIP"});
        assert!(
            f.store
                .finish_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Ok(&result),
                    false
                )
                .unwrap()
        );
        assert!(
            !f.store
                .finish_delivery_package(
                    &i.project_id,
                    i.conversation_id,
                    i.task_id,
                    id,
                    Err("late failure"),
                    false
                )
                .unwrap()
        );
        assert_eq!(
            f.store
                .delivery_package(&i.project_id, i.conversation_id, i.task_id, id)
                .unwrap()
                .phase,
            Phase::Ready
        );
    }
}
