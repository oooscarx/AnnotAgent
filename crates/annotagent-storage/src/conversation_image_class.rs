//! One frozen image-class review; append-only Sandbox feedback, never formal annotations.
use crate::{
    ConversationCallReceipt, ConversationFeedbackScopeAnswer, SampleFeedbackRevision, SqliteStore,
    StorageError,
};
use annotagent_core::{FinalCandidateProjection, TaskKind, VisionArtifactValue};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageClassScope {
    pub feedback_call_id: Uuid,
    pub scope_answer: ConversationFeedbackScopeAnswer,
    pub feedback_receipt: ConversationCallReceipt,
    pub feedback_context: Value,
    pub sample_test_id: String,
    pub draft_id: String,
    pub draft_revision: u64,
    pub draft_content_hash: String,
    pub image_id: String,
    pub content_hash: String,
    pub baseline_sequence: u64,
    pub baseline_feedback: Vec<SampleFeedbackRevision>,
    pub kind: TaskKind,
    pub target_label: String,
    pub members: Vec<FinalCandidateProjection>,
}
impl ConversationImageClassScope {
    pub fn digest(&self) -> Result<String, StorageError> {
        Ok(annotagent_image_tools::sha256(&serde_json::to_vec(self)?))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageClassCreateInput {
    pub id: Uuid,
    pub feedback_call_id: Uuid,
    pub target_label: Option<String>,
    pub expected_scope_digest: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversationImageClassAction {
    Keep {
        outcome_id: String,
        source_artifact_id: Uuid,
    },
    Edit {
        outcome_id: String,
        source_artifact_id: Uuid,
        corrected_value: VisionArtifactValue,
        corrected_label: String,
    },
    ReplaceLabel {
        outcome_id: String,
        source_artifact_id: Uuid,
        replacement: String,
    },
    Exclude {
        outcome_id: String,
        source_artifact_id: Uuid,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageClassAnswerInput {
    pub command_id: Uuid,
    pub expected_scope_digest: String,
    pub actions: Vec<ConversationImageClassAction>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationImageClassStatus {
    Pending,
    Answered,
    Applied,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageClassReview {
    pub id: Uuid,
    pub task_id: Uuid,
    pub conversation_id: Uuid,
    pub input: ConversationImageClassCreateInput,
    pub scope: ConversationImageClassScope,
    pub scope_digest: String,
    pub status: ConversationImageClassStatus,
    pub answer: Option<ConversationImageClassAnswerInput>,
    pub revisions: Vec<SampleFeedbackRevision>,
    pub resume_checkpoint_ref: Uuid,
    pub repair_draft_id: Option<String>,
    pub resume_error: Option<String>,
    pub created_at: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn bounded(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.contains('\0')
}
fn owned(db: &Connection, owner: &str, conversation: Uuid, task: Uuid) -> Result<(), StorageError> {
    if crate::conversation_feedback::owned(db, owner, task)? != conversation {
        return Err(invalid(
            "Image class review belongs to another conversation",
        ));
    }
    Ok(())
}
fn sample(db: &Connection, id: &str) -> Result<crate::WorkflowSampleTest, StorageError> {
    db.query_row("SELECT id,draft_id,project_id,draft_revision,request_revision,draft_content_hash,image_set_hash,model_snapshot_hash,status,input_json,model_bindings_json,report_json,started_at,completed_at FROM workflow_sample_tests WHERE id=?1",[id],crate::workflow_sample_test_row).optional()?.map(crate::workflow_sample_test_from_columns).transpose()?.ok_or_else(||invalid("Saved Sandbox sample is missing"))
}
fn feedback(
    db: &Connection,
    test: &str,
    image: &str,
) -> Result<Vec<SampleFeedbackRevision>, StorageError> {
    let mut query=db.prepare("SELECT feedback_json FROM sample_feedback_revisions WHERE sample_test_id=?1 AND image_id=?2 ORDER BY sequence")?;
    query
        .query_map(params![test, image], |r| r.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect()
}

/// Original model evidence stays untouched. Only this explicit overlay is used for
/// class membership and human edits; Keep never resets earlier human corrections.
#[must_use]
pub fn conversation_image_class_effective_outcome(
    candidate: &FinalCandidateProjection,
    baseline: &[SampleFeedbackRevision],
) -> Option<annotagent_core::SampleTestOutcome> {
    let mut outcome = candidate.outcome.clone();
    let mut excluded = false;
    for revision in baseline
        .iter()
        .filter(|r| r.outcome_id.as_deref() == Some(outcome.id.as_str()))
    {
        if revision.reason == crate::SampleFeedbackReason::ExcludeTarget {
            excluded = true;
            continue;
        }
        if let Some(value) = &revision.corrected_value {
            outcome.value = Some(value.clone());
            excluded = false;
        }
        if let Some(label) = &revision.corrected_label {
            outcome.label.clone_from(label);
            excluded = false;
        }
    }
    (!excluded).then_some(outcome)
}

fn scope(
    db: &Connection,
    owner: &str,
    conversation: Uuid,
    task: Uuid,
    call: Uuid,
    target: Option<&str>,
    check_source_cancel: bool,
) -> Result<ConversationImageClassScope, StorageError> {
    owned(db, owner, conversation, task)?;
    let answer = crate::conversation_feedback_scope::read(db, task, call)?
        .ok_or_else(|| invalid("Save the image-class scope first"))?;
    if answer.conversation_id != conversation
        || answer.input.choice != crate::ConversationFeedbackScopeChoice::CurrentImageClass
    {
        return Err(invalid(
            "Only the exact saved current-image-class scope permits this review",
        ));
    }
    let receipt = crate::conversation_calls::receipt(db, call)?
        .ok_or_else(|| invalid("Feedback receipt missing"))?;
    let authorization = if check_source_cancel {
        crate::conversation_feedback_scope::validate_clarification_source(
            db,
            owner,
            conversation,
            task,
            call,
            &receipt,
        )?
    } else {
        crate::conversation_feedback::read(db, task, call)?
            .ok_or_else(|| invalid("Feedback authorization missing"))?
    };
    crate::conversation_feedback_scope::validate_live_context(
        db,
        owner,
        conversation,
        task,
        &authorization,
        &answer.input.expected_context_digest,
    )?;
    let message: crate::ConversationMessage =
        serde_json::from_value(authorization.context["message"].clone())?;
    let Some(crate::ConversationSelectionRef::SampleCandidate {
        sample_test_id,
        draft_id,
        draft_revision,
        candidate_id,
        source_artifact_id,
        ..
    }) = &message.input.reference
    else {
        return Err(invalid(
            "Image class scope requires a saved candidate reference",
        ));
    };
    let image = message
        .input
        .image
        .as_ref()
        .ok_or_else(|| invalid("Saved image reference missing"))?;
    let test = sample(db, sample_test_id)?;
    let linked:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sample_operations WHERE id=?1 AND project_id=?2 AND draft_id=?3 AND json_extract(request_json,'$.conversation.conversation_id')=?4 AND json_extract(request_json,'$.conversation.task_id')=?5 AND status NOT IN ('queued','running','cancelling'))",params![sample_test_id,test.project_id,draft_id,conversation.to_string(),task.to_string()],|r|r.get(0))?;
    let current:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE id=?1 AND project_id=?2 AND revision=?3 AND content_hash=?4 AND deleted_at IS NULL AND archived_at IS NULL)",params![draft_id,test.project_id,i64::try_from(*draft_revision).map_err(|_|invalid("Draft revision overflow"))?,test.draft_content_hash],|r|r.get(0))?;
    if !linked
        || !current
        || !test.report.sandbox
        || test.inputs.len() != test.report.samples.len()
        || test.draft_id != *draft_id
        || test.draft_revision != *draft_revision
        || authorization.context["sample_content_hash"] != test.draft_content_hash
    {
        return Err(invalid("Saved Sample, task or tested Draft changed"));
    }
    let positions = test
        .inputs
        .iter()
        .enumerate()
        .filter(|(_, i)| i.image_id == image.image_id && i.content_hash == image.sha256)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if positions.len() != 1 {
        return Err(invalid("Image identity is ambiguous in this Sample"));
    }
    let result = &test.report.samples[positions[0]];
    let terminal = result
        .projection
        .final_candidates
        .iter()
        .chain(
            result
                .projection
                .review_candidates
                .iter()
                .map(|r| &r.candidate),
        )
        .collect::<Vec<_>>();
    let ids = terminal
        .iter()
        .map(|c| &c.outcome.id)
        .collect::<BTreeSet<_>>();
    if ids.len() != terminal.len() {
        return Err(invalid(
            "Terminal outcome IDs are ambiguous across Artifacts",
        ));
    }
    let anchor = terminal
        .iter()
        .find(|c| c.outcome.id == *candidate_id && c.source_artifact_id.0 == *source_artifact_id)
        .ok_or_else(|| invalid("Original terminal candidate missing"))?;
    let saved_anchor = &authorization.context["candidate"];
    let restored_anchor: FinalCandidateProjection = serde_json::from_value(saved_anchor.clone())?;
    let expected_anchor = serde_json::to_value(anchor)?;
    let persisted_anchor: Value = serde_json::from_slice(&serde_json::to_vec(&expected_anchor)?)?;
    // Keep all identity/evidence fields and unknown-field rejection, while accepting
    // JSON's last-digit f64 parsing drift for the same typed f32 candidate.
    if **anchor != restored_anchor
        || (*saved_anchor != expected_anchor && *saved_anchor != persisted_anchor)
    {
        return Err(invalid("Original candidate evidence changed"));
    }
    let baseline = feedback(db, sample_test_id, &image.image_id)?;
    let baseline_sequence = baseline.last().map_or(0, |r| r.sequence);
    let effective = conversation_image_class_effective_outcome(anchor, &baseline)
        .ok_or_else(|| invalid("The original candidate was already excluded by human feedback"))?;
    let (kind, label) = match effective.value.as_ref() {
        Some(VisionArtifactValue::BoundingBox { .. }) => {
            if target.is_some_and(|label| label != effective.label) {
                return Err(invalid(
                    "Bounding-box class must equal the original candidate's effective label",
                ));
            }
            (TaskKind::BoundingBox, effective.label)
        }
        Some(VisionArtifactValue::Classification { labels }) => {
            let label = target
                .filter(|label| labels.iter().any(|item| item.as_str() == *label))
                .ok_or_else(|| {
                    invalid("Select an exact label token from the original classification")
                })?;
            (TaskKind::Classification, label.to_owned())
        }
        _ => {
            return Err(invalid(
                "Image class review supports boxes or classification only",
            ));
        }
    };
    if !bounded(&label) {
        return Err(invalid("Invalid image class label"));
    }
    let mut members = terminal
        .into_iter()
        .filter(|candidate| {
            conversation_image_class_effective_outcome(candidate, &baseline).is_some_and(
                |effective| match &effective.value {
                    Some(VisionArtifactValue::BoundingBox { .. }) => {
                        kind == TaskKind::BoundingBox && effective.label == label
                    }
                    Some(VisionArtifactValue::Classification { labels }) => {
                        kind == TaskKind::Classification
                            && labels.iter().any(|item| item.as_str() == label)
                    }
                    _ => false,
                },
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if members.is_empty() || members.len() > 256 {
        return Err(invalid(
            "Image class review requires 1–256 complete terminal members; narrow the scope instead of truncating",
        ));
    }
    members.sort_by(|a, b| {
        a.outcome
            .id
            .cmp(&b.outcome.id)
            .then_with(|| a.source_artifact_id.cmp(&b.source_artifact_id))
    });
    let saved = ConversationImageClassScope {
        feedback_call_id: call,
        scope_answer: answer,
        feedback_receipt: receipt,
        feedback_context: authorization.context,
        sample_test_id: sample_test_id.clone(),
        draft_id: draft_id.clone(),
        draft_revision: *draft_revision,
        draft_content_hash: test.draft_content_hash,
        image_id: image.image_id.clone(),
        content_hash: image.sha256.clone(),
        baseline_sequence,
        baseline_feedback: baseline,
        kind,
        target_label: label,
        members,
    };
    if serde_json::to_vec(&saved)?.len() > 1_048_576 {
        return Err(invalid(
            "Frozen image class review exceeds its bounded context limit",
        ));
    }
    Ok(saved)
}

fn read(
    db: &Connection,
    owner: &str,
    conversation: Uuid,
    task: Uuid,
    id: Uuid,
) -> Result<Option<ConversationImageClassReview>, StorageError> {
    owned(db, owner, conversation, task)?;
    let row:Option<(String,String,String)>=db.query_row("SELECT task_id,conversation_id,record_json FROM conversation_image_class_reviews WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    row.map(|(t, c, json)| {
        if t != task.to_string() || c != conversation.to_string() {
            return Err(invalid("Image class review belongs to another task"));
        }
        Ok(serde_json::from_str(&json)?)
    })
    .transpose()
}
pub(crate) fn require_no_pending_on_image(
    db: &Connection,
    task: Uuid,
    test: &str,
    image: &str,
) -> Result<(), StorageError> {
    let waiting:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_image_class_reviews WHERE task_id=?1 AND sample_test_id=?2 AND image_id=?3 AND status='pending')",params![task.to_string(),test,image],|r|r.get(0))?;
    if waiting {
        return Err(invalid(
            "An image-class human review is already pending on this image",
        ));
    }
    Ok(())
}
fn update(db: &Connection, review: &ConversationImageClassReview) -> Result<(), StorageError> {
    let state = match review.status {
        ConversationImageClassStatus::Pending => "pending",
        ConversationImageClassStatus::Answered => "answered",
        ConversationImageClassStatus::Applied => "applied",
        ConversationImageClassStatus::Cancelled => "cancelled",
    };
    db.execute("UPDATE conversation_image_class_reviews SET status=?2,answer_command_id=?3,record_json=?4 WHERE id=?1",params![review.id.to_string(),state,review.answer.as_ref().map(|a|a.command_id.to_string()),serde_json::to_string(review)?])?;
    Ok(())
}
impl ConversationImageClassAction {
    fn identity(&self) -> (&str, Uuid) {
        match self {
            Self::Keep {
                outcome_id,
                source_artifact_id,
            }
            | Self::Edit {
                outcome_id,
                source_artifact_id,
                ..
            }
            | Self::ReplaceLabel {
                outcome_id,
                source_artifact_id,
                ..
            }
            | Self::Exclude {
                outcome_id,
                source_artifact_id,
            } => (outcome_id, *source_artifact_id),
        }
    }
}
fn revisions(
    review: &ConversationImageClassReview,
    input: &ConversationImageClassAnswerInput,
) -> Result<Vec<SampleFeedbackRevision>, StorageError> {
    use crate::SampleFeedbackReason as Reason;
    if input.command_id.is_nil()
        || input.expected_scope_digest != review.scope_digest
        || input.actions.len() != review.scope.members.len()
    {
        return Err(invalid(
            "Answer requires the exact complete frozen member set and digest",
        ));
    }
    let identities = input
        .actions
        .iter()
        .map(ConversationImageClassAction::identity)
        .collect::<BTreeSet<_>>();
    if identities.len() != input.actions.len() {
        return Err(invalid(
            "Each frozen member must receive exactly one human decision",
        ));
    }
    let now = chrono::Utc::now();
    let mut result = Vec::new();
    for (index, member) in review.scope.members.iter().enumerate() {
        let action = input
            .actions
            .iter()
            .find(|a| a.identity() == (member.outcome.id.as_str(), member.source_artifact_id.0))
            .ok_or_else(|| invalid("Answer includes a foreign or missing frozen member"))?;
        let effective =
            conversation_image_class_effective_outcome(member, &review.scope.baseline_feedback)
                .ok_or_else(|| invalid("Frozen member was excluded"))?;
        let (reason, value, label) = match action {
            ConversationImageClassAction::Keep { .. } => (Reason::Correct, None, None),
            ConversationImageClassAction::Edit {
                corrected_value,
                corrected_label,
                ..
            } => {
                if review.scope.kind != TaskKind::BoundingBox
                    || !matches!(corrected_value, VisionArtifactValue::BoundingBox { .. })
                    || !bounded(corrected_label)
                {
                    return Err(invalid(
                        "Only a box member permits explicit box or label editing",
                    ));
                }
                corrected_value
                    .validate()
                    .map_err(|e| invalid(&e.to_string()))?;
                (
                    Reason::PoorBoundary,
                    Some(corrected_value.clone()),
                    Some(corrected_label.clone()),
                )
            }
            ConversationImageClassAction::Exclude { .. }
                if review.scope.kind == TaskKind::BoundingBox =>
            {
                (Reason::ExcludeTarget, None, None)
            }
            ConversationImageClassAction::ReplaceLabel { .. }
            | ConversationImageClassAction::Exclude { .. } => {
                let Some(VisionArtifactValue::Classification { labels }) = effective.value else {
                    return Err(invalid(
                        "This action requires an exact classification token",
                    ));
                };
                if labels
                    .iter()
                    .filter(|label| label.as_str() == review.scope.target_label)
                    .count()
                    != 1
                {
                    return Err(invalid(
                        "Selected classification token is missing or ambiguous",
                    ));
                }
                let replacement = if let ConversationImageClassAction::ReplaceLabel {
                    replacement,
                    ..
                } = action
                {
                    if !bounded(replacement)
                        || labels.iter().any(|label| {
                            label.as_str() == replacement
                                && label.as_str() != review.scope.target_label
                        })
                    {
                        return Err(invalid(
                            "Replacement must be a bounded distinct label token",
                        ));
                    }
                    Some(replacement)
                } else {
                    None
                };
                let labels = labels
                    .into_iter()
                    .filter_map(|label| {
                        if label.as_str() == review.scope.target_label {
                            replacement.map(|value| annotagent_core::LabelId::from(value.clone()))
                        } else {
                            Some(label)
                        }
                    })
                    .collect::<Vec<_>>();
                if labels.is_empty() {
                    (Reason::ExcludeTarget, None, None)
                } else {
                    let display = labels
                        .iter()
                        .map(annotagent_core::LabelId::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    (
                        Reason::WrongTarget,
                        Some(VisionArtifactValue::Classification { labels }),
                        Some(display),
                    )
                }
            }
        };
        result.push(SampleFeedbackRevision {
            revision_id: Uuid::new_v5(
                &input.command_id,
                format!(
                    "image-class-v1:{}:{}",
                    member.outcome.id, member.source_artifact_id
                )
                .as_bytes(),
            )
            .to_string(),
            sample_test_id: review.scope.sample_test_id.clone(),
            image_id: review.scope.image_id.clone(),
            sequence: review
                .scope
                .baseline_sequence
                .checked_add(u64::try_from(index).unwrap() + 1)
                .ok_or_else(|| invalid("Feedback sequence overflow"))?,
            reason,
            outcome_id: Some(member.outcome.id.clone()),
            corrected_value: value,
            corrected_label: label,
            addition_id: None,
            note: "Explicit human decision on a frozen image-class review; Sandbox only.".into(),
            created_at: now,
        });
    }
    let baseline = member_baseline(review);
    if baseline.len() + result.len() > 1024
        || serde_json::to_vec(
            &baseline
                .into_iter()
                .chain(result.iter())
                .collect::<Vec<_>>(),
        )?
        .len()
            > 262_144
    {
        return Err(invalid(
            "Class answer and its prior member corrections exceed the bounded revision evidence limit",
        ));
    }
    Ok(result)
}
fn member_baseline(review: &ConversationImageClassReview) -> Vec<&SampleFeedbackRevision> {
    let ids = review
        .scope
        .members
        .iter()
        .map(|m| m.outcome.id.as_str())
        .collect::<BTreeSet<_>>();
    review
        .scope
        .baseline_feedback
        .iter()
        .filter(|r| r.outcome_id.as_deref().is_some_and(|id| ids.contains(id)))
        .collect()
}

impl SqliteStore {
    pub fn conversation_image_class_scope(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
        target_label: Option<&str>,
    ) -> Result<ConversationImageClassScope, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let saved = scope(&tx, owner, conversation, task, call, target_label, true)?;
            tx.commit()?;
            Ok(saved)
        })
    }
    pub fn conversation_image_class_review(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationImageClassReview>, StorageError> {
        self.with_connection(|db| read(db, owner, conversation, task, id))
    }
    pub fn conversation_image_class_review_for_feedback(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationImageClassReview>, StorageError> {
        self.with_connection(|db| {
            owned(db, owner, conversation, task)?;
            let id: Option<String> = db
                .query_row(
                    "SELECT id FROM conversation_image_class_reviews WHERE feedback_call_id=?1",
                    [call.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            id.map(|id| {
                read(
                    db,
                    owner,
                    conversation,
                    task,
                    Uuid::parse_str(&id).map_err(|_| invalid("Invalid saved review ID"))?,
                )
            })
            .transpose()
            .map(Option::flatten)
        })
    }
    pub fn create_conversation_image_class_review(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        input: &ConversationImageClassCreateInput,
    ) -> Result<ConversationImageClassReview, StorageError> {
        self.with_connection(|db|{
        let tx=db.unchecked_transaction()?;
        if let Some(saved)=read(&tx,owner,conversation,task,input.id)? {if saved.input!=*input {return Err(invalid("Review creation retry changed its immutable input"));}return Ok(saved);}
        if input.id.is_nil() || input.feedback_call_id.is_nil() {return Err(invalid("Review requires stable command and feedback identities"));}
        let scope=scope(&tx,owner,conversation,task,input.feedback_call_id,input.target_label.as_deref(),true)?;
        let digest=scope.digest()?;if digest!=input.expected_scope_digest {return Err(invalid("Image-class preview changed; review its exact members again"));}
        require_no_pending_on_image(&tx,task,&scope.sample_test_id,&scope.image_id)?;
        let waiting:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE task_id=?1 AND status='pending' AND json_extract(request_json,'$.sample_test_id')=?2 AND json_extract(request_json,'$.image_id')=?3)",params![task.to_string(),scope.sample_test_id,scope.image_id],|r|r.get(0))?;
        if waiting {return Err(invalid("Resolve the existing single-candidate request before creating an image-class review"));}
        let saved=ConversationImageClassReview{id:input.id,task_id:task,conversation_id:conversation,input:input.clone(),scope,scope_digest:digest,status:ConversationImageClassStatus::Pending,answer:None,revisions:vec![],resume_checkpoint_ref:Uuid::new_v5(&input.id,b"image-class-repair-draft-v1"),repair_draft_id:None,resume_error:None,created_at:chrono::Utc::now().to_rfc3339()};
        tx.execute("INSERT INTO conversation_image_class_reviews(id,feedback_call_id,task_id,conversation_id,sample_test_id,image_id,status,record_json) VALUES(?1,?2,?3,?4,?5,?6,'pending',?7)",params![saved.id.to_string(),input.feedback_call_id.to_string(),task.to_string(),conversation.to_string(),saved.scope.sample_test_id,saved.scope.image_id,serde_json::to_string(&saved)?])?;
        tx.commit()?;Ok(saved)
    })
    }
    pub fn answer_conversation_image_class_review(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        input: &ConversationImageClassAnswerInput,
    ) -> Result<ConversationImageClassReview, StorageError> {
        let saved = self
            .conversation_image_class_review(owner, conversation, task, id)?
            .ok_or_else(|| invalid("Image-class review missing"))?;
        if let Some(answer) = &saved.answer {
            if answer != input {
                return Err(invalid(
                    "Answer retry conflicts with its saved human decisions",
                ));
            }
            return Ok(saved);
        }
        if saved.status != ConversationImageClassStatus::Pending {
            return Err(invalid("Image-class review is not pending"));
        }
        let answers = revisions(&saved, input)?;
        self.save_sample_feedback_batch_with_checks(
            &answers,
            |tx| {
                let current = read(tx, owner, conversation, task, id)?
                    .ok_or_else(|| invalid("Review missing"))?;
                if let Some(answer) = &current.answer {
                    if answer == input {
                        return Ok(false);
                    }
                    return Err(invalid("Concurrent answer changed this review"));
                }
                if current.status != ConversationImageClassStatus::Pending {
                    return Err(invalid("Review was cancelled before its answer was saved"));
                }
                // Cancelling the original feedback after review creation does not revoke
                // this concrete human request. The group has its own cancellation command.
                if scope(
                    tx,
                    owner,
                    conversation,
                    task,
                    current.input.feedback_call_id,
                    current.input.target_label.as_deref(),
                    false,
                )? != current.scope
                {
                    return Err(invalid(
                        "Image, tested plan, members or feedback changed since review creation",
                    ));
                }
                Ok(true)
            },
            |tx| {
                let mut current = read(tx, owner, conversation, task, id)?
                    .ok_or_else(|| invalid("Review missing"))?;
                current.answer = Some(input.clone());
                current.revisions.clone_from(&answers);
                current.status = ConversationImageClassStatus::Answered;
                current.resume_error = None;
                update(tx, &current)
            },
        )?;
        self.conversation_image_class_review(owner, conversation, task, id)?
            .ok_or_else(|| invalid("Review disappeared after answer"))
    }
    pub fn cancel_conversation_image_class_review(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationImageClassReview, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let mut saved = read(&tx, owner, conversation, task, id)?
                .ok_or_else(|| invalid("Image-class review missing"))?;
            if saved.status == ConversationImageClassStatus::Cancelled {
                return Ok(saved);
            }
            if saved.status != ConversationImageClassStatus::Pending {
                return Err(invalid(
                    "An answered review cannot be cancelled or erase saved feedback",
                ));
            }
            saved.status = ConversationImageClassStatus::Cancelled;
            update(&tx, &saved)?;
            tx.commit()?;
            Ok(saved)
        })
    }
    pub fn pending_conversation_image_class_resumes(
        &self,
    ) -> Result<Vec<(String, ConversationImageClassReview)>, StorageError> {
        self.with_connection(|db|{
        let mut query=db.prepare("SELECT c.project_id,r.record_json FROM conversation_image_class_reviews r JOIN project_conversations c ON c.id=r.conversation_id WHERE r.status='answered' ORDER BY r.rowid")?;
        query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.map(|row|{let(owner,json)=row?;Ok((owner,serde_json::from_str(&json)?))}).collect()
    })
    }
    pub fn record_conversation_image_class_resume_failure(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
        error: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let mut saved = read(&tx, owner, conversation, task, id)?
                .ok_or_else(|| invalid("Image-class review missing"))?;
            if saved.status == ConversationImageClassStatus::Answered {
                saved.resume_error = Some(error.chars().take(4000).collect());
                update(&tx, &saved)?;
            }
            tx.commit()?;
            Ok(())
        })
    }
    pub fn resume_conversation_image_class_review(
        &self,
        owner: &str,
        conversation: Uuid,
        task: Uuid,
        id: Uuid,
    ) -> Result<ConversationImageClassReview, StorageError> {
        let saved = self
            .conversation_image_class_review(owner, conversation, task, id)?
            .ok_or_else(|| invalid("Image-class review missing"))?;
        if saved.status == ConversationImageClassStatus::Applied {
            return Ok(saved);
        }
        if saved.status != ConversationImageClassStatus::Answered {
            return Err(invalid(
                "Save the human class answer before preparing its revision Draft",
            ));
        }
        let test = self
            .get_workflow_sample_test_by_id(&saved.scope.sample_test_id)?
            .ok_or_else(|| invalid("Sample missing"))?;
        let ids = member_baseline(&saved)
            .into_iter()
            .chain(saved.revisions.iter())
            .map(|r| r.revision_id.clone())
            .collect::<Vec<_>>();
        let draft = self.copy_sample_plan_for_feedback_revisions(
            &test.id,
            &test.project_id,
            &saved.resume_checkpoint_ref.to_string(),
            &ids,
        )?;
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let mut current = read(&tx, owner, conversation, task, id)?
                .ok_or_else(|| invalid("Review missing"))?;
            if current.answer != saved.answer || current.revisions != saved.revisions {
                return Err(invalid("Saved class answer changed before resume"));
            }
            current.status = ConversationImageClassStatus::Applied;
            current.repair_draft_id = Some(draft.id);
            current.resume_error = None;
            update(&tx, &current)?;
            tx.commit()?;
            Ok(current)
        })
    }
}

#[cfg(test)]
#[path = "conversation_image_class_tests.rs"]
mod tests;
