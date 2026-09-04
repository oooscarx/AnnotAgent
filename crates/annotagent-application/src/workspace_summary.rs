//! Project guidance and workspace-summary application service.
//!
//! This module is deliberately narrow: it coordinates persisted Project, Workflow, Run, and
//! Review state without owning HTTP representation or UI policy.

use std::collections::BTreeMap;

use annotagent_core::{ProjectId, WorkflowDraftStatus, WorkflowNodeKind};
use annotagent_storage::{PageRequest, SummaryPage};
use anyhow::Result;

use super::{
    LocalApplication, ProjectGuidance, ProjectGuidanceInput, ProjectSummary,
    ProjectWorkspaceSummary, SampleTestState, Settings, derive_project_guidance, stable_project_id,
};

impl LocalApplication {
    /// Enumerate only the requested Project page before loading its richer summaries. This keeps
    /// a global index bounded even when the workspace contains hundreds of Project directories.
    pub fn list_projects_summary(
        &self,
        request: PageRequest,
    ) -> Result<SummaryPage<ProjectSummary>> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.workspace)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
                continue;
            }
            if entry.path().join("project.yaml").is_file() {
                ids.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        ids.sort();
        let total = ids.len();
        let selected = ids
            .into_iter()
            .skip(request.offset)
            .take(request.limit)
            .collect::<Vec<_>>();
        let consumed = request.offset.saturating_add(selected.len());
        let items = selected
            .into_iter()
            .filter_map(|id| self.get_project(&id).ok())
            .collect();
        Ok(SummaryPage {
            items,
            total,
            limit: request.limit,
            offset: request.offset,
            next_offset: (consumed < total).then_some(consumed),
        })
    }

    /// Resolve storage Project IDs to URL route IDs without constructing every Project summary.
    pub fn list_project_route_ids(&self) -> Result<BTreeMap<ProjectId, String>> {
        let mut ids = BTreeMap::new();
        for entry in std::fs::read_dir(&self.workspace)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink()
                || !entry.file_type()?.is_dir()
                || !entry.path().join("project.yaml").is_file()
            {
                continue;
            }
            ids.insert(
                stable_project_id(&entry.path()),
                entry.file_name().to_string_lossy().into_owned(),
            );
        }
        Ok(ids)
    }

    pub fn project_guidance(
        &self,
        project_id: &str,
        settings: &Settings,
        workspace_model_connected: bool,
    ) -> Result<ProjectGuidance> {
        let summary = self.get_project(project_id)?;
        let project_path = self.project_path(project_id)?;
        let mut updated_at = project_path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map_or_else(
                |_| chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                chrono::DateTime::<chrono::Utc>::from,
            );
        let mut drafts = self
            .store
            .list_workflow_drafts(Some(project_id))?
            .into_iter()
            .filter(|draft| draft.status != WorkflowDraftStatus::Archived)
            .collect::<Vec<_>>();
        drafts.sort_by_key(|draft| std::cmp::Reverse(draft.updated_at));
        if let Some(draft) = drafts.first() {
            updated_at = updated_at.max(draft.updated_at);
        }
        let published = self
            .store
            .list_published_workflow_versions(Some(project_id))?
            .into_iter()
            .max_by_key(|version| version.published_at);
        if let Some(version) = &published {
            updated_at = updated_at.max(version.published_at);
        }
        for run in summary.active_run.iter().chain(summary.last_run.iter()) {
            if let Ok(value) = chrono::DateTime::parse_from_rfc3339(&run.updated_at) {
                updated_at = updated_at.max(value.with_timezone(&chrono::Utc));
            }
        }

        let editable_draft = drafts.iter().find(|draft| {
            matches!(
                draft.status,
                WorkflowDraftStatus::Suggested
                    | WorkflowDraftStatus::Editing
                    | WorkflowDraftStatus::Validated
            )
        });
        let automation = published
            .as_ref()
            .map(|version| &version.draft)
            .or(editable_draft);
        let has_automation = automation.is_some();
        let automation_valid = if published.is_some() {
            true
        } else if let Some(draft) = editable_draft {
            self.validate_workflow_draft(draft, settings, false)?
                .issues
                .iter()
                .all(|issue| !issue.blocking || issue.code == "unresolved_model_binding")
        } else {
            true
        };
        let model_nodes = automation
            .into_iter()
            .flat_map(|draft| draft.nodes.iter())
            .filter(|node| {
                matches!(
                    node.kind,
                    WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                )
            })
            .collect::<Vec<_>>();
        let all_model_nodes_bound = model_nodes.iter().all(|node| node.model_binding.is_some());
        let needs_workspace_connection = model_nodes.iter().any(|node| {
            node.model_binding
                .as_deref()
                .is_some_and(|binding| !binding.starts_with("mock"))
        });
        let has_model_binding =
            all_model_nodes_bound && (!needs_workspace_connection || workspace_model_connected);

        let sample_test = if published.is_some() {
            SampleTestState::Passed
        } else if let Some(draft) = editable_draft {
            self.store.get_workflow_sample_test(&draft.id)?.map_or(
                SampleTestState::NotRun,
                |record| {
                    updated_at = updated_at.max(record.completed_at);
                    if record.report.validation.valid
                        && record.report.summary.failed_count == 0
                        && record.report.summary.needs_review_count == 0
                    {
                        SampleTestState::Passed
                    } else {
                        SampleTestState::NeedsAttention
                    }
                },
            )
        } else {
            SampleTestState::NotRun
        };
        let project_root = project_path.parent().unwrap_or(&self.workspace);
        let stable_id = stable_project_id(project_root);
        let has_completed_run = self.store.project_has_completed_run(
            stable_id,
            published
                .as_ref()
                .map(|workflow| (workflow.workflow_id.as_str(), workflow.version)),
        )?;
        let has_labels = !summary.annotation_schema.is_empty()
            && summary
                .annotation_schema
                .iter()
                .any(|task| !task.labels.is_empty());
        Ok(derive_project_guidance(ProjectGuidanceInput {
            project_id: project_id.to_owned(),
            image_count: summary.image_count,
            has_labels,
            has_automation,
            has_model_binding,
            automation_valid,
            sample_test,
            automation_activated: published.is_some(),
            active_run_id: summary.active_run.as_ref().map(|run| run.id.to_string()),
            active_batch_id: summary
                .active_batch
                .as_ref()
                .map(|batch| batch.id.to_string()),
            review_count: summary.review_count,
            has_completed_run,
            updated_at,
        }))
    }

    pub fn project_workspace_summary(
        &self,
        project_id: &str,
        settings: &Settings,
        workspace_model_connected: bool,
    ) -> Result<ProjectWorkspaceSummary> {
        let project = self.get_project(project_id)?;
        let guidance = self.project_guidance(project_id, settings, workspace_model_connected)?;
        let readiness = guidance.readiness_summary();
        Ok(ProjectWorkspaceSummary {
            project,
            guidance,
            readiness,
        })
    }
}
