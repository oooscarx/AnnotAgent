use annotagent_core::{
    ManagementAction, ManagementBlocker, ManagementObjectKind, ManagementPreview,
    ManagementReceipt, ManagementRequest, ProjectId, RunId, TrashEntry,
};
use annotagent_storage::{ManagementScope, StorageError};
use anyhow::{Result, anyhow};

use crate::{LocalApplication, stable_project_id};

impl LocalApplication {
    pub fn management_scope(&self, project_id: &str) -> Result<ManagementScope> {
        let project_path = self.project_path(project_id)?;
        let root = project_path
            .parent()
            .ok_or_else(|| anyhow!("Project root was not found"))?;
        Ok(ManagementScope {
            project_id: project_id.to_owned(),
            stable_project_id: stable_project_id(root),
        })
    }

    pub fn preview_management(&self, request: &ManagementRequest) -> Result<ManagementPreview> {
        let scope = self.management_scope(&request.project_id)?;
        let mut preview = self.store.preview_management(&scope, request)?;
        if matches!(
            request.action,
            ManagementAction::MoveToTrash | ManagementAction::Purge
        ) {
            for object in preview
                .objects
                .iter()
                .filter(|object| object.kind == ManagementObjectKind::Run)
            {
                let Ok(run_id) = object.id.parse::<RunId>() else {
                    continue;
                };
                if self.is_run_controllable(run_id)
                    && !preview
                        .blockers
                        .iter()
                        .any(|blocker| blocker.code == "run_active" && blocker.object == *object)
                {
                    preview.blockers.push(ManagementBlocker {
                        code: "run_active".to_owned(),
                        object: object.clone(),
                        message: "Run has a live in-process control handle. Cancel it and wait for terminal state before deleting."
                            .to_owned(),
                        related_ids: vec![object.id.clone()],
                    });
                }
            }
        }
        preview.can_execute = preview.blockers.is_empty();
        Ok(preview)
    }

    pub fn execute_management(&self, request: &ManagementRequest) -> Result<ManagementReceipt> {
        if matches!(
            request.action,
            ManagementAction::MoveToTrash | ManagementAction::Purge
        ) {
            for object in request
                .objects
                .iter()
                .filter(|object| object.kind == ManagementObjectKind::Run)
            {
                let Ok(run_id) = object.id.parse::<RunId>() else {
                    continue;
                };
                if self.is_run_controllable(run_id) {
                    return Err(anyhow!(StorageError::Management {
                        code: "run_active".to_owned(),
                        message: "Run has a live in-process control handle. Cancel it and wait for terminal state before deleting."
                            .to_owned(),
                    }));
                }
            }
        }
        let scope = self.management_scope(&request.project_id)?;
        self.store
            .execute_management(&scope, request)
            .map_err(Into::into)
    }

    pub fn list_trash(
        &self,
        project_id: &str,
        kind: Option<ManagementObjectKind>,
    ) -> Result<Vec<TrashEntry>> {
        let scope = self.management_scope(project_id)?;
        self.store
            .list_project_trash(&scope, kind)
            .map_err(Into::into)
    }

    pub fn management_operation(
        &self,
        project_id: &str,
        operation_id: &str,
    ) -> Result<ManagementReceipt> {
        let scope = self.management_scope(project_id)?;
        self.store
            .get_management_operation(&scope, operation_id)
            .map_err(Into::into)
    }

    pub fn stable_project_scope_id(&self, project_id: &str) -> Result<ProjectId> {
        Ok(self.management_scope(project_id)?.stable_project_id)
    }
}
