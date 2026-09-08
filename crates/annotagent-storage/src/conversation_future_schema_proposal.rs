//! Frozen one-call future Schema proposals. No executor or implicit Schema mutation.
use crate::{
    ConversationCallGrant, ConversationFeedbackAuthorization,
    ConversationFeedbackAuthorizationRecord, ConversationFeedbackScopeChoice,
    ConversationFutureSchemaInput, ConversationSchemaDraft, SqliteStore, StorageError,
};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFutureSchemaProposalSource {
    pub feedback_call_id: Uuid,
    pub scope_answer_command_id: Uuid,
    pub context_digest: String,
    pub base_schema_id: Uuid,
    pub base_schema_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFutureSchemaProposalAuthorizationRecord {
    pub consent: ConversationFeedbackAuthorization,
    pub grant: ConversationCallGrant,
    pub source: ConversationFutureSchemaProposalSource,
    pub context: Value,
    pub summary: Value,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn read(
    db: &Connection,
    task: Uuid,
    call: Uuid,
) -> Result<Option<ConversationFutureSchemaProposalAuthorizationRecord>, StorageError> {
    let row:Option<(String,String)>=db.query_row("SELECT task_id,record_json FROM conversation_future_schema_proposal_authorizations WHERE call_id=?1",[call.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    row.map(|(owner, json)| {
        if owner != task.to_string() {
            return Err(invalid("Future Schema proposal belongs to another task"));
        }
        Ok(serde_json::from_str(&json)?)
    })
    .transpose()
}

/// Final transactional admission fence. Explicit authorization may precede dispatch;
/// intervening cancellation, a confirmed fork or DB-backed source edits cannot spend.
pub(crate) fn require_current_source_for_call(
    db: &Connection,
    project: &str,
    task: Uuid,
    call: Uuid,
) -> Result<(), StorageError> {
    let Some(record) = read(db, task, call)? else {
        return Ok(());
    };
    let conversation = crate::conversation_feedback::owned(db, project, task)?;
    let current: String = db.query_row(
        "SELECT id FROM conversation_call_grants WHERE task_id=?1",
        [task.to_string()],
        |r| r.get(0),
    )?;
    let confirmed: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_future_schema_drafts WHERE feedback_call_id=?1)",
        [record.source.feedback_call_id.to_string()],
        |r| r.get(0),
    )?;
    if current != record.grant.id.to_string() || confirmed {
        return Err(invalid(
            "Future Schema proposal authorization was superseded or already confirmed",
        ));
    }
    let (feedback, base) = validate_source(db, project, conversation, task, &record.source)?;
    if record.context
        != serde_json::json!({"source":record.source,"base_schema":base,"feedback":feedback.context,"scope":"future_tasks_only"})
    {
        return Err(invalid(
            "Future Schema proposal source changed before model admission",
        ));
    }
    Ok(())
}

pub(crate) fn validate_source(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    source: &ConversationFutureSchemaProposalSource,
) -> Result<
    (
        ConversationFeedbackAuthorizationRecord,
        ConversationSchemaDraft,
    ),
    StorageError,
> {
    if crate::conversation_feedback::owned(db, project, task)? != conversation
        || source.feedback_call_id.is_nil()
        || source.scope_answer_command_id.is_nil()
        || source.base_schema_id.is_nil()
        || source.base_schema_revision == 0
        || !digest(&source.context_digest)
    {
        return Err(invalid(
            "Future Schema requires stable owned source identities",
        ));
    }
    let answer = crate::conversation_feedback_scope::read(db, task, source.feedback_call_id)?
        .ok_or_else(|| invalid("Save the future-rule scope before proposing a Schema"))?;
    if answer.conversation_id != conversation
        || answer.input.choice != ConversationFeedbackScopeChoice::ProjectFutureRule
        || answer.input.command_id != source.scope_answer_command_id
        || answer.input.expected_context_digest != source.context_digest
    {
        return Err(invalid(
            "Future Schema requires the exact saved future-rule scope answer",
        ));
    }
    let receipt = crate::conversation_calls::receipt(db, source.feedback_call_id)?
        .ok_or_else(|| invalid("Future Schema feedback receipt is missing"))?;
    let feedback = crate::conversation_feedback_scope::validate_clarification_source(
        db,
        project,
        conversation,
        task,
        source.feedback_call_id,
        &receipt,
    )?;
    crate::conversation_feedback_scope::validate_live_context(
        db,
        project,
        conversation,
        task,
        &feedback,
        &source.context_digest,
    )?;
    let base = crate::conversation_future_schema::validate_tested_base(
        db,
        project,
        conversation,
        task,
        &feedback,
        source.base_schema_id,
        source.base_schema_revision,
    )?;
    Ok((feedback, base))
}

#[derive(Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
enum RawProposal {
    Draft {
        goal: String,
        kind: annotagent_core::TaskKind,
        labels: Vec<String>,
        multi_label: bool,
        attributes: BTreeMap<String, annotagent_core::AttributeDefinition>,
        boundary_rules: Vec<String>,
        rationale: String,
    },
    Clarify {
        goal: String,
        question: String,
        rationale: String,
    },
}

fn bounded(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

/// Validate the actual saved Provider output, never the cached parser result.
/// A human may edit a valid draft or answer a valid clarification before saving.
fn validate_response(
    response: &Value,
    base: &crate::ConversationSchemaDefinition,
) -> Result<(), StorageError> {
    if serde_json::to_vec(response)?.len() > 262_144 {
        return Err(invalid("Future Schema proposal response is too large"));
    }
    let response: annotagent_core::ModelResponse = serde_json::from_value(response.clone())?;
    if response.tool_calls.len() != 1
        || response.tool_calls[0].name != "propose_future_annotation_schema"
        || response
            .content
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(invalid(
            "Future Schema proposal requires exactly one bounded tool response",
        ));
    }
    match serde_json::from_value::<RawProposal>(response.tool_calls[0].arguments.clone())? {
        RawProposal::Draft {
            goal,
            kind,
            labels,
            multi_label,
            attributes,
            boundary_rules,
            rationale,
        } => {
            if !bounded(&goal, 4_000) || !bounded(&rationale, 4_000) {
                return Err(invalid("Invalid future Schema proposal text"));
            }
            let mut definition = base.clone();
            definition.goal = goal;
            definition.task.kind = kind;
            definition.task.labels = labels;
            definition.task.multi_label = multi_label;
            definition.task.attributes = attributes;
            definition.boundary_rules = boundary_rules;
            crate::conversation_future_schema::validate_definition(&definition, base)?;
        }
        RawProposal::Clarify {
            goal,
            question,
            rationale,
        } => {
            if !bounded(&goal, 4_000) || !bounded(&question, 2_000) || !bounded(&rationale, 4_000) {
                return Err(invalid("Invalid future Schema proposal clarification"));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_provenance(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    input: &ConversationFutureSchemaInput,
) -> Result<(), StorageError> {
    let (call, expected_digest) = match (input.proposal_call_id, input.proposal_digest.as_deref()) {
        (None, None) => return Ok(()),
        (Some(call), Some(hash)) if !call.is_nil() && digest(hash) => (call, hash),
        _ => {
            return Err(invalid(
                "Future Schema proposal identity and digest must be supplied together",
            ));
        }
    };
    if crate::conversation_feedback::owned(db, project, task)? != conversation {
        return Err(invalid(
            "Future Schema proposal belongs to another conversation",
        ));
    }
    let record = read(db, task, call)?
        .ok_or_else(|| invalid("Future Schema proposal authorization is missing"))?;
    let source = ConversationFutureSchemaProposalSource {
        feedback_call_id: input.feedback_call_id,
        scope_answer_command_id: input.scope_answer_command_id,
        context_digest: input.context_digest.clone(),
        base_schema_id: input.base_schema_id,
        base_schema_revision: input.base_schema_revision,
    };
    if record.source != source || record.consent.call_id != call || record.grant.task_id != task {
        return Err(invalid(
            "Future Schema proposal does not match this exact future-rule source",
        ));
    }
    let receipt = crate::conversation_calls::receipt(db, call)?
        .ok_or_else(|| invalid("Future Schema proposal receipt is missing"))?;
    let evidence = receipt
        .evidence
        .as_ref()
        .ok_or_else(|| invalid("Future Schema proposal response is missing"))?;
    let cancelled: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)",
        [call.to_string()],
        |r| r.get(0),
    )?;
    let grant: Option<String> = db
        .query_row(
            "SELECT grant_id FROM conversation_call_authorizations WHERE call_id=?1",
            [call.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    if receipt.task_id != task
        || receipt.status != crate::ConversationCallStatus::Completed
        || cancelled
        || evidence["cancelled"] == true
        || evidence["phase"] != "future_schema_patch_text"
        || evidence["context"]["subject"] != record.context
        || evidence["context"]["scope_hash"] != record.grant.scope_hash
        || grant.as_deref() != Some(record.grant.id.to_string().as_str())
    {
        return Err(invalid(
            "Future Schema requires its original completed uncancelled proposal",
        ));
    }
    let hash = annotagent_image_tools::sha256(&serde_json::to_vec(
        &serde_json::json!({"context":record.context,"response":evidence["response"]}),
    )?);
    if hash != expected_digest {
        return Err(invalid(
            "Future Schema proposal digest changed; reload the saved proposal",
        ));
    }
    let base: ConversationSchemaDraft =
        serde_json::from_value(record.context["base_schema"].clone())?;
    if base.id != input.base_schema_id
        || base.revision != input.base_schema_revision
        || base.task_id != task
    {
        return Err(invalid("Future Schema proposal base identity changed"));
    }
    validate_response(&evidence["response"], &base.definition)?;
    Ok(())
}

impl SqliteStore {
    pub fn authorize_conversation_future_schema_proposal(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        record: &ConversationFutureSchemaProposalAuthorizationRecord,
    ) -> Result<ConversationFutureSchemaProposalAuthorizationRecord, StorageError> {
        let consent = &record.consent;
        let previous = consent
            .previous_grant_id
            .ok_or_else(|| invalid("A future proposal requires the existing task authorization"))?;
        if consent.call_id.is_nil()
            || consent.message_id.is_nil()
            || consent.model_id.0.is_nil()
            || previous.is_nil()
            || previous == consent.call_id
            || consent.call_id == record.source.feedback_call_id
            || !consent.allow_unknown_cost
            || !digest(&consent.scope_hash)
            || record.grant.id != consent.call_id
            || record.grant.task_id != task
            || record.grant.scope_hash != consent.scope_hash
            || record.grant.expires_at != consent.expires_at
            || !record.context.is_object()
            || !record.summary.is_object()
            || serde_json::to_vec(&record.context)?.len() > 131_072
            || serde_json::to_vec(&record.summary)?.len() > 8_192
        {
            return Err(invalid(
                "Invalid bounded future Schema proposal authorization",
            ));
        }
        self.advance_conversation_authorization_with(project,previous,&record.grant,|tx,created| {
            if crate::conversation_feedback::owned(tx,project,task)?!=conversation {return Err(invalid("Future Schema proposal task belongs to another conversation"));}
            if let Some(saved)=read(tx,task,consent.call_id)? {
                if saved!=*record {return Err(invalid("Future Schema proposal retry changed its saved authorization"));}
                return Ok(());
            }
            if !created {return Err(invalid("Future Schema proposal ID belongs to another authorization"));}
            let used_source:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_future_schema_proposal_authorizations WHERE feedback_call_id=?1 UNION ALL SELECT 1 FROM conversation_future_schema_drafts WHERE feedback_call_id=?1)",[record.source.feedback_call_id.to_string()],|r|r.get(0))?;
            if used_source {return Err(invalid("This future-rule source already has a proposal or confirmed Schema; restore it"));}
            let (feedback,base)=validate_source(tx,project,conversation,task,&record.source)?;
            let context=serde_json::json!({"source":record.source,"base_schema":base,"feedback":feedback.context,"scope":"future_tasks_only"});
            if consent.message_id!=feedback.consent.message_id || record.context!=context {return Err(invalid("Future Schema proposal context differs from its frozen tested source"));}
            if consent.expires_at<=Utc::now() || consent.expires_at>Utc::now()+chrono::Duration::minutes(31) {return Err(invalid("Future Schema proposal authorization expiry is outside the bounded window"));}
            let maximum:u32=tx.query_row("SELECT maximum_calls FROM conversation_authorization_revisions WHERE id=?1 AND task_id=?2",params![previous.to_string(),task.to_string()],|r|r.get(0))?;
            if maximum.checked_add(1).filter(|n|*n<=128)!=Some(record.grant.maximum_calls) {return Err(invalid("Future Schema proposal must add exactly one bounded call without resetting the cumulative ceiling"));}
            let used:u32=tx.query_row("SELECT COUNT(*) FROM conversation_model_calls WHERE task_id=?1",[task.to_string()],|r|r.get(0))?;
            if used>=record.grant.maximum_calls {return Err(invalid("Task cumulative call allowance is exhausted"));}
            crate::conversation_calls::require_call_admission_clear(tx,task,consent.call_id)?;
            crate::conversation_stop::require_admission_clear(tx,task,&consent.call_id.to_string(),false)?;
            crate::conversation_project_budget::admit(tx,project)?;
            let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE id=?1 UNION ALL SELECT 1 FROM conversation_builder_operations WHERE id=?1)",[consent.call_id.to_string()],|r|r.get(0))?;
            if collision {return Err(invalid("Future Schema proposal call ID belongs to another operation"));}
            tx.execute("INSERT INTO conversation_future_schema_proposal_authorizations(call_id,feedback_call_id,task_id,conversation_id,record_json) VALUES(?1,?2,?3,?4,?5)",params![consent.call_id.to_string(),record.source.feedback_call_id.to_string(),task.to_string(),conversation.to_string(),serde_json::to_string(record)?])?;
            Ok(())
        })?;
        Ok(record.clone())
    }
    pub fn conversation_future_schema_proposal_authorization(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFutureSchemaProposalAuthorizationRecord>, StorageError> {
        self.with_connection(|db| {
            crate::conversation_feedback::owned(db, project, task)?;
            read(db, task, call)
        })
    }
    pub fn conversation_future_schema_proposal_for_feedback(
        &self,
        project: &str,
        task: Uuid,
        feedback_call: Uuid,
    ) -> Result<Option<ConversationFutureSchemaProposalAuthorizationRecord>, StorageError> {
        self.with_connection(|db|{
            crate::conversation_feedback::owned(db,project,task)?;
            let row:Option<(String,String)>=db.query_row("SELECT task_id,call_id FROM conversation_future_schema_proposal_authorizations WHERE feedback_call_id=?1",[feedback_call.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let Some((owner,call))=row else{return Ok(None);};
            if owner!=task.to_string(){return Err(invalid("Future Schema proposal source belongs to another task"));}
            read(db,task,Uuid::parse_str(&call).map_err(|_|invalid("Invalid saved future proposal ID"))?)
        })
    }
}

#[cfg(test)]
#[path = "conversation_future_schema_proposal_tests.rs"]
mod tests;
