use annotagent_core::{
    BatchId, ManagementAction, ManagementBlocker, ManagementObjectKind, ManagementOperationStatus,
    ManagementPreview, ManagementReceipt, ManagementRequest, ManagementUsageSummary,
    PipelineLifecycleSummary, ProjectId, RunId, TrashEntry,
};
use annotagent_storage::{ManagementScope, StorageError};
use anyhow::{Result, anyhow};

use crate::{AnnotAgentApplication, DatasetCoordinator, LocalApplication, stable_project_id};

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

    pub async fn execute_management_action(
        &self,
        request: &ManagementRequest,
    ) -> Result<ManagementReceipt> {
        if request.action != ManagementAction::CancelAndDelete {
            return self.execute_management(request);
        }
        let scope = self.management_scope(&request.project_id)?;
        let prepared = self.store.prepare_cancel_and_delete(&scope, request)?;
        if prepared.status == ManagementOperationStatus::Completed {
            return Ok(prepared);
        }
        if prepared.status == ManagementOperationStatus::Failed {
            return Err(anyhow!(StorageError::Management {
                code: "cancellation_not_completed".to_owned(),
                message: prepared.error.unwrap_or_else(|| {
                    "the earlier cancellation attempt failed; retry with a new explicit action"
                        .to_owned()
                }),
            }));
        }
        let cancellation = async {
            for object in &prepared.affected_objects {
                match object.kind {
                    ManagementObjectKind::Run => {
                        let run_id = object
                            .id
                            .parse::<RunId>()
                            .map_err(|error| anyhow!("invalid Run id: {error}"))?;
                        if self.is_run_controllable(run_id) {
                            AnnotAgentApplication::cancel_run(self, run_id).await?;
                        } else if self.store.run_status(run_id).is_ok_and(|status| {
                            matches!(
                                status,
                                annotagent_core::RunStatus::Pending
                                    | annotagent_core::RunStatus::Running
                                    | annotagent_core::RunStatus::Paused
                                    | annotagent_core::RunStatus::AwaitingReview
                            )
                        }) {
                            return Err(anyhow!(
                                "Run has no live control handle; cancellation cannot be confirmed"
                            ));
                        }
                    }
                    ManagementObjectKind::Batch => {
                        let batch_id = object
                            .id
                            .parse::<BatchId>()
                            .map_err(|error| anyhow!("invalid Dataset Run id: {error}"))?;
                        let batch = self.store.get_batch(batch_id)?;
                        if !batch.status.is_terminal() {
                            DatasetCoordinator::new(self).cancel(batch_id)?;
                        }
                    }
                    _ => {
                        return Err(anyhow!(
                            "cancel_and_delete only accepts Runs and Dataset Runs"
                        ));
                    }
                }
            }
            for _ in 0..200 {
                match self.store.complete_cancel_and_delete(&scope, request) {
                    Ok(receipt) => return Ok(receipt),
                    Err(StorageError::Management { code, .. })
                        if code == "cancellation_not_completed" =>
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Err(error) => return Err(anyhow!(error)),
                }
            }
            Err(anyhow!(
                "cancellation did not reach a terminal lease-free state within 10 seconds"
            ))
        }
        .await;
        match cancellation {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                let message = error.to_string();
                let _ignored = self.store.fail_cancel_and_delete(&scope, request, &message);
                Err(anyhow!(StorageError::Management {
                    code: "cancellation_not_completed".to_owned(),
                    message,
                }))
            }
        }
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

    pub fn list_pipeline_lifecycle(
        &self,
        project_id: &str,
        include_archived: bool,
        include_deleted: bool,
    ) -> Result<Vec<PipelineLifecycleSummary>> {
        let scope = self.management_scope(project_id)?;
        self.store
            .list_project_pipeline_lifecycle(&scope, include_archived, include_deleted)
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

    pub fn management_usage_summary(&self, project_id: &str) -> Result<ManagementUsageSummary> {
        let scope = self.management_scope(project_id)?;
        self.store
            .project_management_usage(&scope)
            .map_err(Into::into)
    }

    pub fn stable_project_scope_id(&self, project_id: &str) -> Result<ProjectId> {
        Ok(self.management_scope(project_id)?.stable_project_id)
    }
}
