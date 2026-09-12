//! Link the existing processing service to saved conversation evidence, not another executor.
use crate::LocalApplication;
use annotagent_core::WorkflowDraft;
use annotagent_storage::ConversationSchemaDraft;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConversationProcessingContext {
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    pub source_message_id: Uuid,
    pub schema: ConversationSchemaDraft,
}

impl LocalApplication {
    pub fn pending_conversation_schema_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationSchemaAuthorization>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.pending_conversation_schema_authorization(
            &self.conversation_project_identity(project)?,
            task,
        )?)
    }
    pub fn authorize_conversation_schema_request(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &annotagent_storage::ConversationSchemaAuthorization,
    ) -> Result<()> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.authorize_conversation_schema(
            &self.conversation_project_identity(project)?,
            task,
            input,
        )?)
    }
    pub fn conversation_schema_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationSchemaAuthorization>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.conversation_schema_authorization(
            &self.conversation_project_identity(project)?,
            task,
            call,
        )?)
    }
    pub fn authorize_conversation_schema_retry(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        input: &annotagent_storage::ConversationSchemaRetryAuthorization,
    ) -> Result<()> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.authorize_conversation_schema_retry(
            &self.conversation_project_identity(project)?,
            task,
            input,
        )?)
    }
    pub fn latest_conversation_schema_retry(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        source: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationSchemaRetryAuthorization>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.latest_conversation_schema_retry(
            &self.conversation_project_identity(project)?,
            task,
            source,
        )?)
    }
    pub fn conversation_schema_retry(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<annotagent_storage::ConversationSchemaRetryAuthorization>> {
        self.optional_conversation_builder_budget(project, conversation, task)?;
        Ok(self.store.conversation_schema_retry(
            &self.conversation_project_identity(project)?,
            task,
            call,
        )?)
    }
    pub fn project_conversation_call_limit(
        &self,
        project: &str,
    ) -> Result<annotagent_storage::ProjectCallLimit> {
        Ok(self
            .store
            .project_conversation_call_limit(&self.conversation_project_identity(project)?)?)
    }
    pub fn set_project_conversation_call_limit(
        &self,
        project: &str,
        input: &annotagent_storage::ProjectCallLimitInput,
    ) -> Result<annotagent_storage::ProjectCallLimit> {
        Ok(self.store.set_project_conversation_call_limit(
            &self.conversation_project_identity(project)?,
            input,
        )?)
    }
    pub fn conversation_task_budget(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<annotagent_storage::ConversationTaskBudget> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Budget task does not belong to this conversation");
        }
        Ok(self
            .store
            .conversation_task_budget(&self.conversation_project_identity(project)?, task)?)
    }
    /// Derive ownership from the persisted sample command; never trust active UI selection.
    /// A later Schema revision does not reinterpret the exact revision tested by this Draft.
    pub fn conversation_processing_context(
        &self,
        project: &str,
        draft: &WorkflowDraft,
        sample_id: &str,
    ) -> Result<Option<ConversationProcessingContext>> {
        if draft.project_id != project {
            bail!("Processing Draft belongs to another Project");
        }
        let operation = self.store.sample_operation(sample_id)?;
        let consent = operation
            .as_ref()
            .and_then(|value| value.request.get("conversation"));
        let Some(consent) = consent.filter(|value| !value.is_null()) else {
            if draft.annotation_schema.is_some() {
                bail!("Conversation Schema requires its saved conversation Sample Operation");
            }
            return Ok(None);
        };
        let operation = operation.as_ref().context("Sample Operation missing")?;
        if operation.project_id != project || operation.draft_id != draft.id {
            bail!("Conversation Sample Operation belongs to another Project or Draft");
        }
        let conversation_id: Uuid = serde_json::from_value(consent["conversation_id"].clone())?;
        let task_id: Uuid = serde_json::from_value(consent["task_id"].clone())?;
        let task = self
            .conversation_tasks(project, conversation_id)?
            .into_iter()
            .find(|task| task.input.id == task_id)
            .context("Processing task does not belong to this conversation")?;
        let binding = draft
            .annotation_schema
            .as_ref()
            .context("Conversation processing requires the tested Schema snapshot")?;
        let schema = self.conversation_schema_draft(
            project,
            Uuid::parse_str(&binding.schema_draft_id)?,
            Some(binding.revision),
        )?;
        if schema.task_id != task_id
            || schema.definition.goal != binding.goal
            || schema.definition.task != binding.task
            || schema.definition.boundary_rules != binding.boundary_rules
        {
            bail!("Processing Schema snapshot differs from the owned conversation revision");
        }
        Ok(Some(ConversationProcessingContext {
            conversation_id,
            task_id,
            source_message_id: task.input.source_message_id,
            schema,
        }))
    }

    pub fn conversation_processing_history(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<serde_json::Value>> {
        if !self
            .conversation_tasks(project, conversation)?
            .iter()
            .any(|item| item.input.id == task)
        {
            bail!("Processing task does not belong to this conversation");
        }
        Ok(self
            .store
            .conversation_processing_operations(project, conversation, task)?)
    }
}
