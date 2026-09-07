//! Project-scoped lifecycle management contracts for Runs and Pipelines.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_MANAGEMENT_OBJECTS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementObjectKind {
    Run,
    Batch,
    WorkflowDraft,
    WorkflowVersion,
    Pipeline,
}

impl ManagementObjectKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Run => "Run",
            Self::Batch => "Dataset Run",
            Self::WorkflowDraft => "Pipeline Draft",
            Self::WorkflowVersion => "Published Version",
            Self::Pipeline => "Pipeline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementAction {
    MoveToTrash,
    Restore,
    Archive,
    Unarchive,
    Rename,
    SetDefault,
    ClearDefault,
    Purge,
    CancelAndDelete,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ManagementObjectRef {
    pub kind: ManagementObjectKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    pub expected_revision: u64,
}

impl ManagementObjectRef {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.id.len() > 128 {
            return Err("management object id must be non-empty and at most 128 bytes".to_owned());
        }
        if self.kind == ManagementObjectKind::WorkflowVersion && self.version.is_none() {
            return Err("Published Version management requires a version".to_owned());
        }
        if self.kind != ManagementObjectKind::WorkflowVersion && self.version.is_some() {
            return Err(format!("{} does not accept a version", self.kind.label()));
        }
        if self.expected_revision == 0 {
            return Err("expected_revision must be greater than zero".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowVersionRef {
    pub workflow_id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowLifecycleItem {
    pub object: ManagementObjectRef,
    pub display_name: String,
    pub content_hash: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deletion_operation_id: Option<String>,
    pub is_default: bool,
    pub historical_run_references: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineLifecycleSummary {
    pub project_id: String,
    pub workflow_id: String,
    pub display_name: String,
    pub lifecycle_revision: u64,
    pub archived_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deletion_operation_id: Option<String>,
    pub default_version: Option<u32>,
    pub drafts: Vec<WorkflowLifecycleItem>,
    pub versions: Vec<WorkflowLifecycleItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementRequest {
    pub project_id: String,
    pub objects: Vec<ManagementObjectRef>,
    pub action: ManagementAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_default_version: Option<WorkflowVersionRef>,
    #[serde(default)]
    pub clear_default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_token: Option<String>,
}

impl ManagementRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.project_id.trim().is_empty() || self.project_id.len() > 128 {
            return Err("project_id must be non-empty and at most 128 bytes".to_owned());
        }
        if self.objects.is_empty() || self.objects.len() > MAX_MANAGEMENT_OBJECTS {
            return Err(format!(
                "management request requires 1 to {MAX_MANAGEMENT_OBJECTS} explicit objects"
            ));
        }
        for object in &self.objects {
            object.validate()?;
        }
        let mut unique = self.objects.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != self.objects.len() {
            return Err("management request contains duplicate objects".to_owned());
        }
        if self.idempotency_key.trim().is_empty() || self.idempotency_key.len() > 128 {
            return Err("idempotency_key must be non-empty and at most 128 bytes".to_owned());
        }
        if self.replacement_default_version.is_some() && self.clear_default {
            return Err(
                "replacement_default_version and clear_default are mutually exclusive".to_owned(),
            );
        }
        if let Some(name) = self.display_name.as_deref()
            && (name.trim().is_empty() || name.len() > 160)
        {
            return Err("display_name must be non-empty and at most 160 bytes".to_owned());
        }
        if self.action == ManagementAction::Rename
            && (self.objects.len() != 1 || self.display_name.is_none())
        {
            return Err("rename requires exactly one object and a display_name".to_owned());
        }
        if matches!(
            self.action,
            ManagementAction::SetDefault | ManagementAction::ClearDefault
        ) && (self.objects.len() != 1
            || self.objects[0].kind != ManagementObjectKind::WorkflowVersion)
        {
            return Err("default management requires exactly one Published Version".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementBlocker {
    pub code: String,
    pub object: ManagementObjectRef,
    pub message: String,
    #[serde(default)]
    pub related_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementImpact {
    pub top_level_objects: usize,
    pub child_runs: usize,
    pub unresolved_reviews_hidden: usize,
    pub confirmed_annotations_retained: usize,
    pub historical_run_references: usize,
    pub calibration_references: usize,
    pub debug_rows: usize,
    pub estimated_reclaimable_bytes: Option<u64>,
    pub estimate_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementPreview {
    pub project_id: String,
    pub action: ManagementAction,
    pub objects: Vec<ManagementObjectRef>,
    pub impact: ManagementImpact,
    pub blockers: Vec<ManagementBlocker>,
    pub confirmation_token: String,
    pub can_execute: bool,
    pub recoverable: bool,
    pub navigation_target: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementOperationStatus {
    Prepared,
    WaitingForCancellation,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeReport {
    pub database_rows_removed: usize,
    pub files_removed: usize,
    pub bytes_reclaimed: u64,
    pub retained_annotation_records: usize,
    pub retained_usage_records: usize,
    pub retained_provenance_records: usize,
    #[serde(default)]
    pub retained_reasons: Vec<String>,
    #[serde(default)]
    pub failed_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementReceipt {
    pub operation_id: String,
    pub project_id: String,
    pub action: ManagementAction,
    pub status: ManagementOperationStatus,
    pub affected_objects: Vec<ManagementObjectRef>,
    pub impact: ManagementImpact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purge: Option<PurgeReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashEntry {
    pub project_id: String,
    pub object: ManagementObjectRef,
    pub display_name: String,
    pub deleted_at: DateTime<Utc>,
    pub deletion_operation_id: String,
    pub source_project: String,
    pub recoverable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_requests_require_explicit_bounded_unique_objects() {
        let object = ManagementObjectRef {
            kind: ManagementObjectKind::Run,
            id: "run-1".to_owned(),
            version: None,
            expected_revision: 1,
        };
        let mut request = ManagementRequest {
            project_id: "project".to_owned(),
            objects: vec![object.clone(), object],
            action: ManagementAction::MoveToTrash,
            replacement_default_version: None,
            clear_default: false,
            display_name: None,
            idempotency_key: "request-1".to_owned(),
            confirmation_token: None,
        };
        assert!(request.validate().is_err());
        request.objects.truncate(1);
        assert!(request.validate().is_ok());
    }
}
