//! Ownership boundary for inert context archives; no runtime or secret dependency.
use super::LocalApplication;
use annotagent_storage::context_archives::{ConfirmContextImport, ContextArchive};
use anyhow::Result;
use serde_json::Value;
use uuid::Uuid;
impl LocalApplication {
    pub fn export_context_archive(
        &self,
        project: &str,
        conversation: Uuid,
    ) -> Result<ContextArchive> {
        Ok(self.store.export_context_archive(
            &self.conversation_project_identity(project)?,
            project,
            conversation,
        )?)
    }
    pub fn preview_context_import(&self, project: &str, archive: &ContextArchive) -> Result<Value> {
        Ok(self.store.preview_context_import(
            &self.conversation_project_identity(project)?,
            project,
            archive,
        )?)
    }
    pub fn confirm_context_import(
        &self,
        project: &str,
        input: &ConfirmContextImport,
    ) -> Result<Value> {
        Ok(self.store.confirm_context_import(
            &self.conversation_project_identity(project)?,
            project,
            input,
        )?)
    }
    pub fn context_import_receipt(&self, project: &str, command: Uuid) -> Result<Value> {
        Ok(self
            .store
            .context_import_receipt(&self.conversation_project_identity(project)?, command)?)
    }
    pub fn archived_context(&self, project: &str, context: Uuid) -> Result<Value> {
        Ok(self
            .store
            .archived_context(&self.conversation_project_identity(project)?, context)?)
    }
    pub fn archived_contexts(&self, project: &str, after: i64, limit: u32) -> Result<Value> {
        Ok(self.store.archived_contexts(
            &self.conversation_project_identity(project)?,
            after,
            limit,
        )?)
    }
}
