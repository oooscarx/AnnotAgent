//! Future-only semantics fork. Existing plans, samples and published snapshots stay fixed.
use crate::{
    ConversationFeedbackContext, ConversationFeedbackDecision, ConversationSchemaDecision,
    LocalApplication,
};
use annotagent_core::WorkflowSchemaBinding;
use annotagent_storage::{
    ConversationFutureSchemaInput, ConversationFutureSchemaRecord, ConversationSchemaDraft,
    ConversationSelectionRef,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFutureSchemaRequest {
    pub command_id: Uuid,
    pub expected_scope_answer_command_id: Uuid,
    pub expected_context_digest: String,
    pub base_schema_id: Uuid,
    pub base_schema_revision: u64,
    pub goal: String,
    pub decision: ConversationSchemaDecision,
}

#[derive(Debug, Serialize)]
pub struct ConversationFutureSchemaSource {
    pub scope_answer_command_id: Uuid,
    pub context_digest: String,
}

#[derive(Debug, Serialize)]
pub struct ConversationFutureSchemaView {
    pub record: Option<ConversationFutureSchemaRecord>,
    pub base_schema: ConversationSchemaDraft,
    pub schema: Option<ConversationSchemaDraft>,
    pub source: ConversationFutureSchemaSource,
    pub scope: &'static str,
}

impl LocalApplication {
    /// GET restoration never creates a Schema or authorizes inference.
    pub fn conversation_future_schema(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<ConversationFutureSchemaView> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        let owner = self.conversation_project_identity(project)?;
        if let Some(record) = self.store.future_schema_draft(&owner, task, call)? {
            let base_schema = self.conversation_schema_draft(
                project,
                record.input.base_schema_id,
                Some(record.input.base_schema_revision),
            )?;
            let schema = self.conversation_schema_draft(project, record.schema_id, None)?;
            if base_schema.task_id != task || schema.task_id != task {
                bail!("Future Schema history does not belong to this task");
            }
            return Ok(ConversationFutureSchemaView {
                source: ConversationFutureSchemaSource {
                    scope_answer_command_id: record.input.scope_answer_command_id,
                    context_digest: record.input.context_digest.clone(),
                },
                record: Some(record),
                base_schema,
                schema: Some(schema),
                scope: "future_tasks_only",
            });
        }
        let authorization = self
            .conversation_feedback_authorization(project, conversation, task, call)?
            .context("Saved feedback authorization is missing")?;
        let answer = self
            .conversation_feedback_scope_answer(project, conversation, task, call)?
            .context("Clarify and save the intended scope before proposing future rules")?;
        if answer.input.choice
            != annotagent_storage::ConversationFeedbackScopeChoice::ProjectFutureRule
        {
            bail!(
                "Only explicit future Project rule scope can create a future Schema Draft; image-local feedback does not grant that scope"
            );
        }
        let result = self
            .read_conversation_feedback(project, conversation, task, call)?
            .context("Saved feedback proposal is missing")?;
        if !matches!(
            result.decision.map_err(|error| anyhow::anyhow!(error))?,
            ConversationFeedbackDecision::ClarifyScope { .. }
        ) {
            bail!("Future rules require the original saved scope clarification");
        }
        let context: ConversationFeedbackContext =
            serde_json::from_value(authorization.context.clone())?;
        if self.conversation_feedback_context(
            project,
            conversation,
            task,
            authorization.consent.message_id,
        )? != context
            || answer.input.expected_context_digest
                != annotagent_storage::conversation_feedback_context_digest(&authorization.context)?
        {
            bail!("Feedback context changed; no future rule or Schema was created");
        }
        let Some(ConversationSelectionRef::SampleCandidate { sample_test_id, .. }) =
            &context.message.input.reference
        else {
            bail!("Future Schema source lacks an exact sample reference");
        };
        let seal = self.store.sample_scope_seal(sample_test_id)?.context("This older Sample Test did not freeze its Schema; test an explicitly bound plan before proposing future rules")?;
        let binding: WorkflowSchemaBinding = serde_json::from_value(
            seal.get("annotation_schema")
                .cloned()
                .filter(|value| !value.is_null())
                .context(
                    "Sample Test has no frozen annotation Schema; no other Schema was substituted",
                )?,
        )?;
        let base_schema = self.conversation_schema_draft(
            project,
            Uuid::parse_str(&binding.schema_draft_id)?,
            Some(binding.revision),
        )?;
        if base_schema.task_id != task
            || binding.task != base_schema.definition.task
            || binding.goal != base_schema.definition.goal
            || binding.boundary_rules != base_schema.definition.boundary_rules
        {
            bail!("The sample's frozen Schema differs from its saved task revision");
        }
        if self
            .conversation_schema_draft(project, base_schema.id, None)?
            .revision
            != base_schema.revision
        {
            bail!(
                "The tested Schema has a newer revision. Review that change and test it before creating a future-rule fork; no silent rebase was performed"
            );
        }
        Ok(ConversationFutureSchemaView {
            record: None,
            base_schema,
            schema: None,
            source: ConversationFutureSchemaSource {
                scope_answer_command_id: answer.input.command_id,
                context_digest: answer.input.expected_context_digest,
            },
            scope: "future_tasks_only",
        })
    }

    /// Human-authored bounded proposal. This is not a model result or Project migration.
    pub fn save_conversation_future_schema(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
        request: &ConversationFutureSchemaRequest,
    ) -> Result<ConversationFutureSchemaView> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        if request.goal.trim().is_empty()
            || request.goal.len() > 4_000
            || request.goal.contains('\0')
        {
            bail!("Describe a nonempty future annotation goal within 4000 bytes");
        }
        let config = request
            .decision
            .task_config(task)?
            .context("A scope question is not a future Schema Draft")?;
        let ConversationSchemaDecision::Draft { boundary_rules, .. } = &request.decision else {
            unreachable!()
        };
        let input = ConversationFutureSchemaInput {
            command_id: request.command_id,
            feedback_call_id: call,
            scope_answer_command_id: request.expected_scope_answer_command_id,
            context_digest: request.expected_context_digest.clone(),
            base_schema_id: request.base_schema_id,
            base_schema_revision: request.base_schema_revision,
            definition: annotagent_storage::ConversationSchemaDefinition {
                goal: request.goal.clone(),
                task: config,
                boundary_rules: boundary_rules.clone(),
            },
        };
        let owner = self.conversation_project_identity(project)?;
        if let Some(record) = self.store.future_schema_draft(&owner, task, call)? {
            if record.input != input {
                bail!(
                    "A different future Schema request is already saved. It was not replaced; open that Draft to edit it"
                );
            }
            return self.conversation_future_schema(project, conversation, task, call);
        }
        let preview = self.conversation_future_schema(project, conversation, task, call)?;
        if preview.base_schema.id != request.base_schema_id
            || preview.base_schema.revision != request.base_schema_revision
            || preview.source.scope_answer_command_id != request.expected_scope_answer_command_id
            || preview.source.context_digest != request.expected_context_digest
        {
            bail!(
                "Future Schema source changed. The original goal and your entered changes were not applied to another source"
            );
        }
        self.store
            .create_future_schema_draft(&owner, conversation, task, &input)?;
        self.conversation_future_schema(project, conversation, task, call)
    }
}
