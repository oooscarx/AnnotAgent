//! Atomic persistence for the repository-owned Demo onboarding boundary.
//! This records initialization and provenance only; it grants no model call or review.

use crate::{SqliteStore, StorageError};
use annotagent_core::{
    Annotation, AnnotationId, AnnotationRevision, AnnotationRevisionId, ImageId, ModelProfileId,
    ProjectId, ReviewStatus, RevisionActor, dataset_delivery::TaskDeliveryIntent,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoSourceMode {
    PresetCandidates,
    LiveModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoStartScope {
    pub command_id: Uuid,
    pub demo_id: String,
    pub demo_version: String,
    pub source_mode: DemoSourceMode,
    pub model_profile_id: Option<ModelProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoStartStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoStartReceipt {
    pub contract_version: String,
    pub command_id: Uuid,
    pub demo_id: String,
    pub demo_version: String,
    pub source_mode: DemoSourceMode,
    pub catalog_revision: String,
    pub manifest_sha256: String,
    pub status: DemoStartStatus,
    pub project_id: String,
    pub project_owner_id: String,
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    pub work_route: String,
    pub source_provenance: DemoSourceProvenance,
    pub replayed: bool,
    pub retry_safe: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoSourceProvenance {
    pub kind: DemoSourceMode,
    pub live_inference_occurred: bool,
    pub review_status: Option<String>,
    pub source_asset_id: Option<String>,
    pub source_asset_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoImageSeed {
    pub relative_path: String,
    pub sha256: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone)]
pub struct DemoPresetAnnotationSeed {
    pub annotation: Annotation,
    pub source_artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoPresetObjectReviewInput {
    pub command_id: Uuid,
    pub intent_revision: u32,
    pub intent_sha256: String,
    pub annotation_id: AnnotationId,
    pub expected_snapshot_sha256: String,
    pub label: String,
    pub value: annotagent_core::AnnotationValue,
    pub review_status: ReviewStatus,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct DemoStartSeed {
    pub scope: DemoStartScope,
    pub receipt: DemoStartReceipt,
    pub source_message_id: Uuid,
    pub goal: String,
    pub schema_revision: String,
    pub images: Vec<DemoImageSeed>,
    pub delivery_intent: TaskDeliveryIntent,
    pub delivery_command_id: Uuid,
    pub model_profile_id: Option<ModelProfileId>,
    pub preset_candidates: Option<serde_json::Value>,
    pub preset_annotations: Vec<DemoPresetAnnotationSeed>,
}

impl SqliteStore {
    pub fn demo_start_receipt(
        &self,
        command_id: Uuid,
    ) -> Result<Option<(DemoStartScope, DemoStartReceipt)>, StorageError> {
        self.with_connection(|db| {
            let row: Option<(String, String)> = db
                .query_row(
                    "SELECT scope_json,receipt_json FROM demo_start_commands WHERE command_id=?1",
                    [command_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            row.map(|(scope, receipt)| {
                Ok((
                    serde_json::from_str(&scope)?,
                    serde_json::from_str(&receipt)?,
                ))
            })
            .transpose()
        })
    }

    /// Persists all database-owned initialization in one transaction. It deliberately creates
    /// no Schema, Draft, Run, annotation, authorization or model-attempt row.
    pub fn start_demo_context(
        &self,
        seed: &DemoStartSeed,
    ) -> Result<(DemoStartReceipt, bool), StorageError> {
        seed.delivery_intent
            .validate()
            .map_err(|message| StorageError::InvalidConversation(message.into()))?;
        let preset = seed.receipt.source_mode == DemoSourceMode::PresetCandidates;
        if seed.scope.command_id.is_nil()
            || seed.source_message_id.is_nil()
            || seed.receipt.task_id.is_nil()
            || seed.schema_revision.len() != 64
            || !seed
                .schema_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || seed.scope.command_id != seed.receipt.command_id
            || seed.scope.demo_id != seed.receipt.demo_id
            || seed.scope.demo_version != seed.receipt.demo_version
            || seed.scope.source_mode != seed.receipt.source_mode
            || seed.scope.model_profile_id != seed.model_profile_id
            || seed.receipt.project_owner_id != seed.delivery_intent.project_id
            || seed.receipt.conversation_id != seed.delivery_intent.conversation_id
            || seed.receipt.task_id != seed.delivery_intent.task_id
            || preset != seed.preset_candidates.is_some()
            || preset == seed.preset_annotations.is_empty()
            || preset == seed.model_profile_id.is_some()
        {
            return Err(StorageError::InvalidConversation(
                "Demo initialization identities or source boundary are invalid".into(),
            ));
        }
        let scope_json = serde_json::to_string(&seed.scope)?;
        let receipt_json = serde_json::to_string(&seed.receipt)?;
        let intent_json = serde_json::to_string(&seed.delivery_intent)?;
        let intent_sha256 = format!("{:x}", Sha256::digest(intent_json.as_bytes()));
        let owner: ProjectId =
            seed.receipt.project_owner_id.parse().map_err(|_| {
                StorageError::InvalidConversation("invalid Demo Project owner".into())
            })?;

        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT scope_json,receipt_json FROM demo_start_commands WHERE command_id=?1",
                    [seed.scope.command_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((saved_scope, saved_receipt)) = existing {
                if saved_scope != scope_json {
                    return Err(StorageError::ConversationContract {
                        code: "demo_command_conflict",
                        message: "Demo command ID is already bound to another exact scope".into(),
                    });
                }
                let mut receipt: DemoStartReceipt = serde_json::from_str(&saved_receipt)?;
                receipt.replayed = true;
                return Ok((receipt, true));
            }

            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO project_conversations(id,project_id,created_at) VALUES(?1,?2,?3)",
                params![seed.receipt.conversation_id.to_string(), seed.receipt.project_owner_id, now],
            )?;
            let message = crate::ConversationMessageInput {
                id: seed.source_message_id,
                text: seed.goal.clone(),
                image: None,
                reference: None,
            };
            tx.execute(
                "INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES(?1,1,?2,?3,?4)",
                params![seed.receipt.conversation_id.to_string(), seed.source_message_id.to_string(), serde_json::to_string(&message)?, now],
            )?;
            tx.execute(
                "INSERT INTO conversation_tasks(id,conversation_id,source_message_id,schema_revision,created_at) VALUES(?1,?2,?3,?4,?5)",
                params![seed.receipt.task_id.to_string(), seed.receipt.conversation_id.to_string(), seed.source_message_id.to_string(), seed.schema_revision, now],
            )?;
            for image in &seed.images {
                let image_id = ImageId(Uuid::new_v5(&owner.0, image.relative_path.as_bytes()));
                tx.execute(
                    "INSERT INTO images(id,project_id,relative_path,sha256,metadata_json,imported_at) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![image_id.to_string(), owner.to_string(), image.relative_path, image.sha256, image.metadata_json, now],
                )?;
            }
            tx.execute(
                "INSERT INTO task_delivery_intents(task_id,revision,command_id,expected_revision,content_sha256,intent_json,created_at) VALUES(?1,1,?2,0,?3,?4,?5)",
                params![seed.receipt.task_id.to_string(), seed.delivery_command_id.to_string(), intent_sha256, intent_json, now],
            )?;
            if let Some(model) = seed.model_profile_id {
                tx.execute(
                    "INSERT INTO conversation_agent_models(conversation_id,revision,model_profile_id) VALUES(?1,1,?2)",
                    params![seed.receipt.conversation_id.to_string(), model.to_string()],
                )?;
                let selection = crate::SelectConversationAgentModel {
                    request_id: seed.scope.command_id,
                    expected_revision: 0,
                    model_profile_id: Some(model),
                };
                tx.execute(
                    "INSERT INTO conversation_agent_model_commands(conversation_id,request_id,input_json) VALUES(?1,?2,?3)",
                    params![seed.receipt.conversation_id.to_string(), seed.scope.command_id.to_string(), serde_json::to_string(&selection)?],
                )?;
            }
            tx.execute(
                "INSERT INTO demo_start_commands(command_id,scope_json,receipt_json,project_owner_id,conversation_id,task_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![seed.scope.command_id.to_string(), scope_json, receipt_json, seed.receipt.project_owner_id, seed.receipt.conversation_id.to_string(), seed.receipt.task_id.to_string(), now],
            )?;
            if let Some(candidates) = &seed.preset_candidates {
                let provenance = &seed.receipt.source_provenance;
                tx.execute(
                    "INSERT INTO demo_preset_candidate_imports(command_id,project_owner_id,conversation_id,task_id,manifest_sha256,source_asset_id,source_asset_sha256,review_status,candidates_json,imported_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'needs_review',?8,?9)",
                    params![seed.scope.command_id.to_string(), seed.receipt.project_owner_id, seed.receipt.conversation_id.to_string(), seed.receipt.task_id.to_string(), seed.receipt.manifest_sha256, provenance.source_asset_id, provenance.source_asset_sha256, serde_json::to_string(candidates)?, now],
                )?;
                for candidate in &seed.preset_annotations {
                    candidate.annotation.validate().map_err(|_| {
                        StorageError::InvalidConversation(
                            "Demo preset annotation geometry is invalid".into(),
                        )
                    })?;
                    if candidate.annotation.source != annotagent_core::AnnotationSource::Imported
                        || candidate.annotation.review_status != ReviewStatus::NeedsReview
                        || candidate.annotation.confidence.is_some()
                        || !seed.images.iter().any(|image| {
                            ImageId(Uuid::new_v5(&owner.0, image.relative_path.as_bytes()))
                                == candidate.annotation.image_id
                        })
                    {
                        return Err(StorageError::InvalidConversation(
                            "Demo preset annotations must be owned unscored imports awaiting review"
                                .into(),
                        ));
                    }
                    tx.execute(
                        "INSERT INTO demo_preset_annotations(annotation_id,command_id,project_owner_id,conversation_id,task_id,image_id,source_artifact_id,annotation_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        params![candidate.annotation.id.to_string(),seed.scope.command_id.to_string(),seed.receipt.project_owner_id,seed.receipt.conversation_id.to_string(),seed.receipt.task_id.to_string(),candidate.annotation.image_id.to_string(),candidate.source_artifact_id,serde_json::to_string(&candidate.annotation)?,now],
                    )?;
                }
            }
            tx.commit()?;
            Ok((seed.receipt.clone(), false))
        })
    }

    pub fn demo_preset_candidate_import(
        &self,
        project_owner_id: &str,
        task_id: Uuid,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.with_connection(|db| {
            let json: Option<String> = db
                .query_row(
                    "SELECT candidates_json FROM demo_preset_candidate_imports WHERE project_owner_id=?1 AND task_id=?2 AND review_status='needs_review'",
                    params![project_owner_id, task_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            json.map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(Into::into)
        })
    }

    pub fn is_demo_preset_task(
        &self,
        project_owner_id: &str,
        conversation_id: Uuid,
        task_id: Uuid,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM demo_preset_candidate_imports WHERE project_owner_id=?1 AND conversation_id=?2 AND task_id=?3)",
                params![project_owner_id,conversation_id.to_string(),task_id.to_string()],
                |row| row.get(0),
            )
            .map_err(Into::into)
        })
    }

    pub fn demo_preset_annotation_origin(
        &self,
        project_owner_id: &str,
        task_id: Uuid,
        annotation_id: AnnotationId,
    ) -> Result<Option<String>, StorageError> {
        self.with_connection(|db| {
            db.query_row(
                "SELECT source_artifact_id FROM demo_preset_annotations WHERE project_owner_id=?1 AND task_id=?2 AND annotation_id=?3",
                params![project_owner_id,task_id.to_string(),annotation_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub(crate) fn demo_preset_annotations(
        db: &rusqlite::Connection,
        project_owner_id: &str,
        conversation_id: Uuid,
        task_id: Uuid,
        image_id: ImageId,
    ) -> Result<Vec<Annotation>, StorageError> {
        let mut statement = db.prepare(
            "SELECT annotation_json FROM demo_preset_annotations WHERE project_owner_id=?1 AND conversation_id=?2 AND task_id=?3 AND image_id=?4 ORDER BY annotation_id",
        )?;
        statement
            .query_map(
                params![
                    project_owner_id,
                    conversation_id.to_string(),
                    task_id.to_string(),
                    image_id.to_string()
                ],
                |row| row.get::<_, String>(0),
            )?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect()
    }

    /// Applies one human decision to a repository-imported candidate. The imported origin stays
    /// visible; the revision actor records the human change. It cannot target model Run rows.
    pub fn review_demo_preset_object(
        &self,
        project_owner_id: &str,
        conversation_id: Uuid,
        task_id: Uuid,
        image_id: ImageId,
        input: &DemoPresetObjectReviewInput,
    ) -> Result<AnnotationRevision, StorageError> {
        if input.command_id.is_nil()
            || input.reason.trim().is_empty()
            || input.reason.len() > 2000
            || !matches!(
                input.review_status,
                ReviewStatus::HumanAccepted | ReviewStatus::Rejected
            )
            || !matches!(
                input.value,
                annotagent_core::AnnotationValue::BoundingBox { .. }
            )
        {
            return Err(StorageError::InvalidConversation(
                "Preset candidate review requires a bbox, final human decision and bounded reason"
                    .into(),
            ));
        }
        input.value.validate().map_err(|_| {
            StorageError::InvalidConversation("Invalid preset candidate geometry".into())
        })?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                project_owner_id,
                conversation_id,
                task_id,
                image_id,
                input
            ))?)
        );
        let reason = format!("{} [demo preset review {fingerprint}]", input.reason);
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let saved = crate::delivery_image_review::intent(
                &tx,
                project_owner_id,
                conversation_id,
                task_id,
            )?;
            if let Some(json) = tx
                .query_row(
                    "SELECT revision_json FROM annotation_revisions WHERE revision_id=?1",
                    [input.command_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                let revision: AnnotationRevision = serde_json::from_str(&json)?;
                if revision.annotation_id != input.annotation_id
                    || revision.reason.as_deref() != Some(reason.as_str())
                {
                    return Err(StorageError::InvalidConversation(
                        "Preset review command changed its immutable scope".into(),
                    ));
                }
                return Ok(revision);
            }
            if saved.revision != input.intent_revision
                || saved.content_sha256 != input.intent_sha256
                || !saved.intent.label_spec.as_ref().is_some_and(|labels| {
                    labels.iter().any(|label| label.stable_id == input.label)
                })
            {
                return Err(StorageError::InvalidConversation(
                    "Delivery version or label changed before preset review".into(),
                ));
            }
            let snapshot = crate::delivery_image_review::snapshot(
                &tx,
                &saved,
                image_id,
                None,
            )?;
            if snapshot.sha256 != input.expected_snapshot_sha256 {
                return Err(StorageError::InvalidConversation(
                    "Preset candidates changed; reload before reviewing".into(),
                ));
            }
            let json: String = tx
                .query_row(
                    "SELECT annotation_json FROM demo_preset_annotations WHERE annotation_id=?1 AND project_owner_id=?2 AND conversation_id=?3 AND task_id=?4 AND image_id=?5",
                    params![input.annotation_id.to_string(),project_owner_id,conversation_id.to_string(),task_id.to_string(),image_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| StorageError::InvalidConversation("Owned preset candidate not found".into()))?;
            let mut annotation: Annotation = serde_json::from_str(&json)?;
            let before = annotation.snapshot();
            annotation.label = Some(input.label.as_str().into());
            annotation.value = input.value.clone();
            annotation.review_status = input.review_status;
            annotation.validate().map_err(|_| {
                StorageError::InvalidConversation("Invalid reviewed preset annotation".into())
            })?;
            let parent_revision_id: Option<String> = tx
                .query_row(
                    "SELECT revision_id FROM annotation_revisions WHERE annotation_id=?1 ORDER BY created_at DESC,revision_id DESC LIMIT 1",
                    [annotation.id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            let revision = AnnotationRevision {
                revision_id: AnnotationRevisionId(input.command_id),
                annotation_id: annotation.id,
                parent_revision_id: parent_revision_id
                    .map(|value| value.parse())
                    .transpose()
                    .map_err(|_| StorageError::InvalidConversation("Invalid saved revision identity".into()))?,
                before: Some(before),
                after: Some(annotation.snapshot()),
                actor: RevisionActor::Human,
                reason: Some(reason),
                created_at: chrono::Utc::now(),
            };
            revision.validate().map_err(|_| {
                StorageError::InvalidConversation("Invalid preset annotation revision".into())
            })?;
            tx.execute(
                "UPDATE demo_preset_annotations SET annotation_json=?2 WHERE annotation_id=?1",
                params![annotation.id.to_string(),serde_json::to_string(&annotation)?],
            )?;
            tx.execute(
                "INSERT INTO annotation_revisions(revision_id,annotation_id,parent_revision_id,revision_json,created_at) VALUES(?1,?2,?3,?4,?5)",
                params![revision.revision_id.to_string(),revision.annotation_id.to_string(),revision.parent_revision_id.map(|value|value.to_string()),serde_json::to_string(&revision)?,revision.created_at.to_rfc3339()],
            )?;
            tx.commit()?;
            Ok(revision)
        })
    }
}
