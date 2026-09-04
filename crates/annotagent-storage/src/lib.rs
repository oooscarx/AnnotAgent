//! `SQLite` persistence for projects, auditable runs, revisions, and correction memory.

mod batch;

pub use batch::BatchClaimResult;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Mutex,
};

use annotagent_core::{
    AgentSession, Annotation, AnnotationRevision, AnnotationRevisionId, AnnotationValue,
    ArtifactId, ArtifactValidationState, BindingMutationActor, CorrectionRecord,
    GeometryCalibrationId, GeometryCalibrationReport, GeometryCorrectionEvidence,
    GeometryQualityReport, GlobalModelDefaults, ImageId, LabelId, ModelBindingId,
    ModelBindingMatch, ModelMessage, ModelProfile, ModelProfileId, PipelineImprovementId,
    PipelineImprovementSession, ProjectGeometryPolicy, ProjectId, ProjectModelBinding,
    ProviderAdapterKind, ProviderId, ProviderProfile, PublishedWorkflowVersion, RelationEndpoint,
    ReviewStatus, RevisionActor, RunEvent, RunEventPayload, RunId, RunStatus, TaskId, TaskKind,
    TaskRunStatus, ToolCallId, ToolResult, UsageRecord, ValidationIssue, VisionArtifact,
    VisionArtifactValue, WorkflowDraft, WorkflowDraftStatus, WorkflowDryRunReport,
    WorkflowSnapshot,
};
use annotagent_runtime::{RunRecord, RuntimeStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/0001_initial.sql");
const BATCH_MIGRATION: &str =
    include_str!("../../../migrations/0003_persistent_dataset_batches.sql");
const AGENT_SESSION_MIGRATION: &str = include_str!("../../../migrations/0004_agent_sessions.sql");
const WORKFLOW_SAMPLE_TEST_MIGRATION: &str =
    include_str!("../../../migrations/0005_workflow_sample_tests.sql");
const PROVIDER_REGISTRY_MIGRATION: &str =
    include_str!("../../../migrations/0006_provider_registry.sql");
const MODEL_PROFILE_MIGRATION: &str = include_str!("../../../migrations/0007_model_profiles.sql");
const PROVIDER_PROBE_USAGE_MIGRATION: &str =
    include_str!("../../../migrations/0008_provider_probe_usage.sql");
const LEGACY_REGISTRY_IMPORT_MIGRATION: &str =
    include_str!("../../../migrations/0009_legacy_registry_imports.sql");
const GEOMETRY_CORRECTION_EVIDENCE_MIGRATION: &str =
    include_str!("../../../migrations/0010_geometry_correction_evidence.sql");
const GEOMETRY_CALIBRATION_MIGRATION: &str =
    include_str!("../../../migrations/0011_geometry_calibration.sql");
const PIPELINE_IMPROVEMENT_MIGRATION: &str =
    include_str!("../../../migrations/0012_pipeline_improvements.sql");
const RUST_PLUGIN_MIGRATION: &str = include_str!("../../../migrations/0013_rust_plugins.sql");
const MODEL_BUNDLE_MIGRATION: &str = include_str!("../../../migrations/0014_model_bundles.sql");
const WORKSPACE_IDENTITY_MIGRATION: &str =
    include_str!("../../../migrations/0015_workspace_identity.sql");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("history serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("database lock poisoned")]
    Poisoned,
    #[error("run {0} was not found")]
    RunNotFound(RunId),
    #[error("dataset batch {0} was not found")]
    BatchNotFound(annotagent_core::BatchId),
    #[error("dataset batch lease conflict: {0}")]
    BatchLeaseConflict(String),
    #[error("Provider Profile {0} was not found")]
    ProviderNotFound(ProviderId),
    #[error("Model Profile {0} revision {1} was not found")]
    ModelProfileNotFound(ModelProfileId, u64),
    #[error("Pipeline improvement {0} was not found")]
    PipelineImprovementNotFound(PipelineImprovementId),
    #[error("invalid Model Profile revision: {0}")]
    InvalidModelRevision(String),
    #[error("semantic Model Profile changes require a new revision")]
    ModelSemanticChangeRequiresRevision,
    #[error("a new Model Profile revision requires a semantic change")]
    ModelRevisionRequiresSemanticChange,
    #[error("Project Model Binding {0} was not found")]
    ModelBindingNotFound(ModelBindingId),
    #[error("Agent cannot modify a locked Project Model Binding")]
    ModelBindingLocked,
    #[error("unsupported history schema version {0}")]
    UnsupportedHistoryVersion(u32),
    #[error("invalid stored enum value: {0}")]
    InvalidEnum(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryRun {
    pub id: RunId,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    pub project_name: String,
    pub skill_id: String,
    pub provider: String,
    pub model: String,
    pub status: RunStatus,
    pub project_schema_json: String,
    #[serde(default)]
    pub workflow_snapshot_json: Option<String>,
    pub terminal_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryToolCall {
    pub call_id: ToolCallId,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<ToolResult>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryModelMessage {
    pub image_id: Option<ImageId>,
    pub task_id: Option<TaskId>,
    pub message: ModelMessage,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryTaskRun {
    pub run_id: RunId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub status: TaskRunStatus,
    pub reason: Option<String>,
    pub updated_at: String,
}

/// Durable Project-scoped identity for an imported image. `image_id` is derived from the
/// Project namespace and content digest, so display order and filename changes cannot retarget
/// annotations or Runs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredProjectImage {
    pub image_id: ImageId,
    pub project_id: ProjectId,
    pub relative_path: String,
    pub sha256: String,
    pub metadata_json: String,
    pub imported_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryDocument {
    pub schema_version: u32,
    pub run: HistoryRun,
    pub events: Vec<RunEvent>,
    pub annotations: Vec<Annotation>,
    pub revisions: Vec<AnnotationRevision>,
    #[serde(default)]
    pub task_runs: Vec<HistoryTaskRun>,
    pub usage: Vec<UsageRecord>,
    #[serde(default)]
    pub model_messages: Vec<HistoryModelMessage>,
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
    pub tool_calls: Vec<HistoryToolCall>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryImportReport {
    pub run_id: RunId,
    pub ids_remapped: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowSampleTest {
    pub draft_id: String,
    pub project_id: String,
    pub report: WorkflowDryRunReport,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistryReference {
    pub kind: String,
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderProbeUsage {
    pub id: Uuid,
    pub provider_id: ProviderId,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost: String,
    pub currency: String,
    pub duration_ms: u64,
    pub succeeded: bool,
    pub safe_message: String,
    pub created_at: DateTime<Utc>,
}

/// Complete, non-secret input for the explicit compatibility-registry import.
///
/// The caller derives deterministic IDs from the legacy connection fingerprint. The storage
/// boundary applies the Provider, Model Profile and Project bindings in one transaction and never
/// reads, copies or deletes a credential value.
#[derive(Debug, Clone)]
pub struct LegacyRegistryImport {
    pub fingerprint: String,
    pub provider: ProviderProfile,
    pub model: ModelProfile,
    pub project_bindings: Vec<ProjectModelBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LegacyRegistryImportReport {
    pub fingerprint: String,
    pub provider_id: ProviderId,
    pub model_profile_id: ModelProfileId,
    pub provider_created: bool,
    pub model_created: bool,
    pub bindings_created: usize,
    pub bindings_preserved: usize,
    pub already_applied: bool,
    pub credential_source: Option<String>,
    pub historical_runs_modified: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStartReservation {
    Reserved,
    Idempotent { run_id: RunId, status: RunStatus },
    Conflict { run_id: RunId, status: RunStatus },
}

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    pub fn update_run_workflow_snapshot(
        &self,
        run_id: RunId,
        snapshot_json: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE runs SET workflow_snapshot_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id.to_string(), snapshot_json, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        let store = Self {
            connection: Mutex::new(connection),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let store = Self {
            connection: Mutex::new(Connection::open_in_memory()?),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute_batch(INITIAL_MIGRATION)?;
            let has_project_id = {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|name| name == "project_id")
            };
            if !has_project_id {
                connection.execute("ALTER TABLE runs ADD COLUMN project_id TEXT", [])?;
            }
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_runs_project_created ON runs(project_id, created_at DESC)",
                [],
            )?;
            let has_distinct_review_state = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 1)",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if !has_distinct_review_state {
                let transaction = connection.unchecked_transaction()?;
                transaction.execute(
                    "UPDATE runs SET status = 'completed_with_review' WHERE status = 'awaiting_review'",
                    [],
                )?;
                transaction.execute(
                    "UPDATE active_project_runs SET status = 'completed_with_review' WHERE status = 'awaiting_review'",
                    [],
                )?;
                transaction.execute(
                    "DELETE FROM active_project_runs WHERE status = 'completed_with_review'",
                    [],
                )?;
                transaction.execute(
                    "INSERT INTO schema_migrations(version, name, applied_at) VALUES (1, ?1, ?2)",
                    params!["distinct_awaiting_review_state", Utc::now().to_rfc3339()],
                )?;
                transaction.commit()?;
            }
            let has_workflow_snapshot = {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|name| name == "workflow_snapshot_json")
            };
            if !has_workflow_snapshot {
                connection.execute(
                    "ALTER TABLE runs ADD COLUMN workflow_snapshot_json TEXT",
                    [],
                )?;
            }
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (2, ?1, ?2)",
                params!["immutable_workflow_run_snapshot", Utc::now().to_rfc3339()],
            )?;
            connection.execute_batch(BATCH_MIGRATION)?;
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (3, ?1, ?2)",
                params!["persistent_dataset_batches", Utc::now().to_rfc3339()],
            )?;
            connection.execute_batch(AGENT_SESSION_MIGRATION)?;
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (4, ?1, ?2)",
                params!["agent_sessions", Utc::now().to_rfc3339()],
            )?;
            connection.execute_batch(WORKFLOW_SAMPLE_TEST_MIGRATION)?;
            connection.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (5, ?1, ?2)",
                params!["workflow_sample_tests", Utc::now().to_rfc3339()],
            )?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(PROVIDER_REGISTRY_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (6, ?1, ?2)",
                params!["provider_registry", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(MODEL_PROFILE_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (7, ?1, ?2)",
                params!["model_profiles", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(PROVIDER_PROBE_USAGE_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (8, ?1, ?2)",
                params!["provider_probe_usage", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(LEGACY_REGISTRY_IMPORT_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (9, ?1, ?2)",
                params!["legacy_registry_imports", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(GEOMETRY_CORRECTION_EVIDENCE_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (10, ?1, ?2)",
                params!["geometry_correction_evidence", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(GEOMETRY_CALIBRATION_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (11, ?1, ?2)",
                params!["geometry_calibration", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(PIPELINE_IMPROVEMENT_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (12, ?1, ?2)",
                params!["pipeline_improvements", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(RUST_PLUGIN_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (13, ?1, ?2)",
                params!["rust_model_plugins", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let transaction = connection.unchecked_transaction()?;
            transaction.execute_batch(MODEL_BUNDLE_MIGRATION)?;
            transaction.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (14, ?1, ?2)",
                params!["model_bundle_provisioning", Utc::now().to_rfc3339()],
            )?;
            transaction.commit()?;
            let has_workspace_identity = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 15)",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if !has_workspace_identity {
                let transaction = connection.unchecked_transaction()?;
                transaction.execute_batch(WORKSPACE_IDENTITY_MIGRATION)?;
                transaction.execute(
                    "INSERT INTO schema_migrations(version, name, applied_at) VALUES (15, ?1, ?2)",
                    params!["project_scoped_workspace_identity", Utc::now().to_rfc3339()],
                )?;
                transaction.commit()?;
            }
            Ok(())
        })
    }

    /// Import the legacy singleton Provider/model/default binding into the reusable Registry.
    ///
    /// A completed fingerprint is immutable and makes repeated calls a no-op. Any validation or
    /// identity collision rolls the complete transaction back. Existing user-selected Project
    /// bindings win and are reported as preserved rather than silently overwritten.
    pub fn apply_legacy_registry_import(
        &self,
        import: &LegacyRegistryImport,
    ) -> Result<LegacyRegistryImportReport, StorageError> {
        if import.fingerprint.trim().is_empty() || import.fingerprint.len() > 128 {
            return Err(StorageError::InvalidEnum(
                "legacy Registry fingerprint must be non-empty and at most 128 bytes".to_owned(),
            ));
        }
        import
            .provider
            .validate()
            .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
        import
            .model
            .validate()
            .map_err(|error| StorageError::InvalidModelRevision(error.to_string()))?;
        if import.model.provider_id != import.provider.id || import.model.revision != 1 {
            return Err(StorageError::InvalidModelRevision(
                "legacy Model Profile must be revision 1 and belong to the imported Provider"
                    .to_owned(),
            ));
        }
        for binding in &import.project_bindings {
            binding
                .validate_for_model(&import.model)
                .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
        }

        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            if let Some(json) = transaction
                .query_row(
                    "SELECT report_json FROM legacy_registry_imports WHERE fingerprint = ?1",
                    [&import.fingerprint],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                let mut report: LegacyRegistryImportReport = serde_json::from_str(&json)?;
                report.already_applied = true;
                transaction.commit()?;
                return Ok(report);
            }

            let existing_provider = transaction
                .query_row(
                    "SELECT profile_json FROM provider_profiles WHERE id = ?1",
                    [import.provider.id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let provider_created = existing_provider.is_none();
            if let Some(json) = existing_provider {
                let existing: ProviderProfile = serde_json::from_str(&json)?;
                if existing.adapter != import.provider.adapter
                    || existing.base_url != import.provider.base_url
                {
                    return Err(StorageError::InvalidEnum(
                        "legacy Provider identity collides with different connection semantics"
                            .to_owned(),
                    ));
                }
            } else {
                transaction.execute(
                    "INSERT INTO provider_profiles
                     (id, display_name, preset_id, adapter, enabled, credential_source,
                      profile_json, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        import.provider.id.to_string(),
                        import.provider.display_name,
                        import.provider.preset_id,
                        enum_string(import.provider.adapter)?,
                        import.provider.enabled,
                        import
                            .provider
                            .credential_ref
                            .as_ref()
                            .map(|reference| enum_string(reference.source))
                            .transpose()?,
                        serde_json::to_string(&import.provider)?,
                        import.provider.created_at.to_rfc3339(),
                        import.provider.updated_at.to_rfc3339(),
                    ],
                )?;
            }

            let existing_model = transaction
                .query_row(
                    "SELECT profile_json FROM model_profiles WHERE id = ?1 AND revision = 1",
                    [import.model.id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let model_created = existing_model.is_none();
            if let Some(json) = existing_model {
                let existing: ModelProfile = serde_json::from_str(&json)?;
                if !existing.has_same_semantics(&import.model) {
                    return Err(StorageError::InvalidModelRevision(
                        "legacy Model Profile identity collides with different semantics"
                            .to_owned(),
                    ));
                }
            } else {
                transaction.execute(
                    "INSERT INTO model_profiles
                     (id, revision, provider_id, display_name, remote_model_id, status, enabled,
                      locked, profile_json, created_at, updated_at)
                     VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        import.model.id.to_string(),
                        import.model.provider_id.to_string(),
                        import.model.display_name,
                        import.model.remote_model_id,
                        enum_string(import.model.status)?,
                        import.model.enabled,
                        import.model.locked,
                        serde_json::to_string(&import.model)?,
                        import.model.created_at.to_rfc3339(),
                        import.model.updated_at.to_rfc3339(),
                    ],
                )?;
            }

            let mut bindings_created = 0;
            let mut bindings_preserved = 0;
            for binding in &import.project_bindings {
                let match_value = binding_match_value(binding)?;
                let existing = transaction
                    .query_row(
                        "SELECT binding_json FROM project_model_bindings
                         WHERE project_id = ?1 AND match_kind = ?2 AND match_value = ?3",
                        params![
                            binding.project_id.to_string(),
                            enum_string(binding.match_kind)?,
                            match_value,
                        ],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if existing.is_some() {
                    bindings_preserved += 1;
                    continue;
                }
                transaction.execute(
                    "INSERT INTO project_model_bindings
                     (id, project_id, match_kind, match_value, capability, role, model_profile_id,
                      locked, binding_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        binding.id.to_string(),
                        binding.project_id.to_string(),
                        enum_string(binding.match_kind)?,
                        binding_match_value(binding)?,
                        enum_string(binding.capability)?,
                        enum_string(binding.role)?,
                        binding.model_profile_id.to_string(),
                        binding.locked,
                        serde_json::to_string(binding)?,
                        binding.created_at.to_rfc3339(),
                    ],
                )?;
                bindings_created += 1;
            }

            let report = LegacyRegistryImportReport {
                fingerprint: import.fingerprint.clone(),
                provider_id: import.provider.id,
                model_profile_id: import.model.id,
                provider_created,
                model_created,
                bindings_created,
                bindings_preserved,
                already_applied: false,
                credential_source: import
                    .provider
                    .credential_ref
                    .as_ref()
                    .map(|reference| enum_string(reference.source))
                    .transpose()?,
                historical_runs_modified: 0,
            };
            transaction.execute(
                "INSERT INTO legacy_registry_imports
                 (fingerprint, provider_id, model_profile_id, report_json, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    report.fingerprint,
                    report.provider_id.to_string(),
                    report.model_profile_id.to_string(),
                    serde_json::to_string(&report)?,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(report)
        })
    }

    pub fn legacy_registry_import_report(
        &self,
        fingerprint: &str,
    ) -> Result<Option<LegacyRegistryImportReport>, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT report_json FROM legacy_registry_imports WHERE fingerprint = ?1",
                    [fingerprint],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .transpose()
        })
    }

    pub fn save_provider_profile(&self, profile: &ProviderProfile) -> Result<(), StorageError> {
        profile
            .validate()
            .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO provider_profiles
                 (id, display_name, preset_id, adapter, enabled, credential_source,
                  profile_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   display_name = excluded.display_name,
                   preset_id = excluded.preset_id,
                   adapter = excluded.adapter,
                   enabled = excluded.enabled,
                   credential_source = excluded.credential_source,
                   profile_json = excluded.profile_json,
                   updated_at = excluded.updated_at",
                params![
                    profile.id.to_string(),
                    profile.display_name,
                    profile.preset_id,
                    enum_string(profile.adapter)?,
                    profile.enabled,
                    profile
                        .credential_ref
                        .as_ref()
                        .map(|reference| enum_string(reference.source))
                        .transpose()?,
                    serde_json::to_string(profile)?,
                    profile.created_at.to_rfc3339(),
                    profile.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_provider_profile(
        &self,
        provider_id: ProviderId,
    ) -> Result<ProviderProfile, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT profile_json FROM provider_profiles WHERE id = ?1",
                    [provider_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::ProviderNotFound(provider_id))?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn list_provider_profiles(&self) -> Result<Vec<ProviderProfile>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT profile_json FROM provider_profiles ORDER BY updated_at DESC, id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn delete_provider_profile(&self, provider_id: ProviderId) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let deleted = connection.execute(
                "DELETE FROM provider_profiles WHERE id = ?1",
                [provider_id.to_string()],
            )?;
            if deleted == 0 {
                return Err(StorageError::ProviderNotFound(provider_id));
            }
            Ok(())
        })
    }

    /// Removes product-visible Provider Registry fixtures while preserving immutable Run history.
    /// Historical Run snapshots remain self-contained audit records, but fixture-backed Drafts and
    /// active Registry bindings must not survive into live authoring.
    pub fn purge_provider_adapter(
        &self,
        adapter: ProviderAdapterKind,
    ) -> Result<(usize, usize, usize), StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let adapter = enum_string(adapter)?;
            let provider_ids = transaction
                .prepare("SELECT id FROM provider_profiles WHERE adapter = ?1")?
                .query_map([adapter], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            if provider_ids.is_empty() {
                transaction.commit()?;
                return Ok((0, 0, 0));
            }

            let mut model_ids = BTreeSet::new();
            for provider_id in &provider_ids {
                let ids = transaction
                    .prepare("SELECT DISTINCT id FROM model_profiles WHERE provider_id = ?1")?
                    .query_map([provider_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                model_ids.extend(ids);
            }

            let mut removed_bindings = 0;
            for model_id in &model_ids {
                removed_bindings += transaction.execute(
                    "DELETE FROM project_model_bindings WHERE model_profile_id = ?1",
                    [model_id],
                )?;
                transaction.execute(
                    "DELETE FROM provider_probe_usage WHERE model_profile_id = ?1",
                    [model_id],
                )?;
            }

            if let Some(defaults_json) = transaction
                .query_row(
                    "SELECT defaults_json FROM global_model_defaults WHERE singleton = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                let mut defaults: GlobalModelDefaults = serde_json::from_str(&defaults_json)?;
                let is_fixture = |id: Option<ModelProfileId>| {
                    id.is_some_and(|id| model_ids.contains(&id.to_string()))
                };
                if is_fixture(defaults.pipeline_builder) {
                    defaults.pipeline_builder = None;
                }
                if is_fixture(defaults.vision_language) {
                    defaults.vision_language = None;
                }
                if is_fixture(defaults.text_generation) {
                    defaults.text_generation = None;
                }
                transaction.execute(
                    "UPDATE global_model_defaults SET defaults_json = ?1, updated_at = ?2
                     WHERE singleton = 1",
                    params![serde_json::to_string(&defaults)?, Utc::now().to_rfc3339()],
                )?;
            }

            let fixture_draft_ids = transaction
                .prepare("SELECT id FROM workflow_drafts WHERE lower(draft_json) LIKE '%mock%'")?
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for draft_id in &fixture_draft_ids {
                transaction.execute(
                    "DELETE FROM workflow_sample_tests WHERE draft_id = ?1",
                    [draft_id],
                )?;
                transaction.execute("DELETE FROM workflow_drafts WHERE id = ?1", [draft_id])?;
            }

            for provider_id in &provider_ids {
                transaction.execute(
                    "DELETE FROM provider_probe_usage WHERE provider_id = ?1",
                    [provider_id],
                )?;
                transaction.execute(
                    "DELETE FROM model_profiles WHERE provider_id = ?1",
                    [provider_id],
                )?;
                transaction
                    .execute("DELETE FROM provider_profiles WHERE id = ?1", [provider_id])?;
            }
            let removed_models = model_ids.len();
            let removed_providers = provider_ids.len();
            transaction.commit()?;
            Ok((removed_providers, removed_models, removed_bindings))
        })
    }

    /// Removes legacy Agent authoring sessions that exposed deterministic test fixtures as if they
    /// were product models. Formal Run history and immutable Workflow versions live in separate
    /// tables and are deliberately untouched.
    pub fn purge_mock_agent_sessions(&self) -> Result<usize, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let candidates = transaction
                .prepare("SELECT id, session_json FROM agent_sessions")?
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut removed = 0;
            for (id, session_json) in candidates {
                let session: AgentSession = serde_json::from_str(&session_json)?;
                let contains_fixture = session.model_selection.as_ref().is_some_and(|selection| {
                    selection.provider_adapter == ProviderAdapterKind::Mock
                        || mock_fixture_identity(&selection.remote_model_id)
                }) || session.model_calls.iter().any(|call| {
                    mock_fixture_identity(&call.provider_name)
                        || mock_fixture_identity(&call.remote_model_id)
                }) || session.steps.iter().any(|step| {
                    value_contains_mock_fixture(&step.arguments)
                        || value_contains_mock_fixture(&step.result)
                });
                if contains_fixture {
                    removed +=
                        transaction.execute("DELETE FROM agent_sessions WHERE id = ?1", [id])?;
                }
            }
            transaction.commit()?;
            Ok(removed)
        })
    }

    pub fn provider_references(
        &self,
        provider_id: ProviderId,
    ) -> Result<Vec<RegistryReference>, StorageError> {
        self.with_connection(|connection| {
            let provider = provider_id.to_string();
            let mut references = Vec::new();
            let mut statement = connection.prepare(
                "SELECT DISTINCT id FROM model_profiles WHERE provider_id = ?1 ORDER BY id",
            )?;
            for model_id in statement
                .query_map([&provider], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
            {
                references.push(RegistryReference {
                    kind: "model_profile".to_owned(),
                    location: model_id,
                });
            }
            append_json_references(
                connection,
                "SELECT id, draft_json FROM workflow_drafts",
                &provider,
                "workflow_draft",
                &mut references,
            )?;
            append_json_references(
                connection,
                "SELECT workflow_id || '@' || version, version_json FROM workflow_versions",
                &provider,
                "published_workflow",
                &mut references,
            )?;
            append_json_references(
                connection,
                "SELECT id, workflow_snapshot_json FROM runs WHERE workflow_snapshot_json IS NOT NULL",
                &provider,
                "run_snapshot",
                &mut references,
            )?;
            Ok(references)
        })
    }

    pub fn save_model_profile(&self, profile: &ModelProfile) -> Result<(), StorageError> {
        profile
            .validate()
            .map_err(|error| StorageError::InvalidModelRevision(error.to_string()))?;
        self.with_connection(|connection| {
            let provider_exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM provider_profiles WHERE id = ?1)",
                [profile.provider_id.to_string()],
                |row| row.get::<_, bool>(0),
            )?;
            if !provider_exists {
                return Err(StorageError::ProviderNotFound(profile.provider_id));
            }
            let latest = connection
                .query_row(
                    "SELECT revision, profile_json FROM model_profiles
                     WHERE id = ?1 ORDER BY revision DESC LIMIT 1",
                    [profile.id.to_string()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .map(|(revision, json)| {
                    let revision = u64::try_from(revision).map_err(|_| {
                        StorageError::InvalidModelRevision(
                            "stored revision must be a positive integer".to_owned(),
                        )
                    })?;
                    serde_json::from_str::<ModelProfile>(&json)
                        .map(|profile| (revision, profile))
                        .map_err(StorageError::from)
                })
                .transpose()?;
            match latest {
                None if profile.revision != 1 => {
                    return Err(StorageError::InvalidModelRevision(
                        "the first revision must be 1".to_owned(),
                    ));
                }
                Some((latest_revision, ref latest_profile))
                    if profile.revision == latest_revision =>
                {
                    if !profile.has_same_semantics(latest_profile) {
                        return Err(StorageError::ModelSemanticChangeRequiresRevision);
                    }
                }
                Some((latest_revision, ref latest_profile))
                    if profile.revision == latest_revision.saturating_add(1) =>
                {
                    if profile.has_same_semantics(latest_profile) {
                        return Err(StorageError::ModelRevisionRequiresSemanticChange);
                    }
                }
                Some((latest_revision, _)) => {
                    return Err(StorageError::InvalidModelRevision(format!(
                        "expected revision {latest_revision} for metadata-only update or {} for a semantic change",
                        latest_revision.saturating_add(1)
                    )));
                }
                None => {}
            }
            connection.execute(
                "INSERT INTO model_profiles
                 (id, revision, provider_id, display_name, remote_model_id, status, enabled,
                  locked, profile_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id, revision) DO UPDATE SET
                   display_name = excluded.display_name,
                   status = excluded.status,
                   enabled = excluded.enabled,
                   locked = excluded.locked,
                   profile_json = excluded.profile_json,
                   updated_at = excluded.updated_at",
                params![
                    profile.id.to_string(),
                    i64::try_from(profile.revision).map_err(|_| {
                        StorageError::InvalidModelRevision(
                            "revision exceeds SQLite integer range".to_owned(),
                        )
                    })?,
                    profile.provider_id.to_string(),
                    profile.display_name,
                    profile.remote_model_id,
                    enum_string(profile.status)?,
                    profile.enabled,
                    profile.locked,
                    serde_json::to_string(profile)?,
                    profile.created_at.to_rfc3339(),
                    profile.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_model_profile(
        &self,
        model_profile_id: ModelProfileId,
        revision: Option<u64>,
    ) -> Result<ModelProfile, StorageError> {
        self.with_connection(|connection| {
            let json = if let Some(revision) = revision {
                let revision = i64::try_from(revision).map_err(|_| {
                    StorageError::InvalidModelRevision(
                        "revision exceeds SQLite integer range".to_owned(),
                    )
                })?;
                connection
                    .query_row(
                        "SELECT profile_json FROM model_profiles WHERE id = ?1 AND revision = ?2",
                        params![model_profile_id.to_string(), revision],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
            } else {
                connection
                    .query_row(
                        "SELECT profile_json FROM model_profiles
                         WHERE id = ?1 ORDER BY revision DESC LIMIT 1",
                        [model_profile_id.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
            }
            .ok_or(StorageError::ModelProfileNotFound(
                model_profile_id,
                revision.unwrap_or(0),
            ))?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn list_model_profiles(
        &self,
        provider_id: Option<ProviderId>,
        include_all_revisions: bool,
    ) -> Result<Vec<ModelProfile>, StorageError> {
        self.with_connection(|connection| {
            let sql = match (provider_id.is_some(), include_all_revisions) {
                (false, false) => {
                    "SELECT current.profile_json FROM model_profiles current
                     WHERE current.revision = (SELECT MAX(revision) FROM model_profiles
                       WHERE id = current.id)
                     ORDER BY current.display_name, current.id"
                }
                (true, false) => {
                    "SELECT current.profile_json FROM model_profiles current
                     WHERE current.provider_id = ?1
                       AND current.revision = (SELECT MAX(revision) FROM model_profiles
                         WHERE id = current.id)
                     ORDER BY current.display_name, current.id"
                }
                (false, true) => {
                    "SELECT profile_json FROM model_profiles ORDER BY display_name, id, revision"
                }
                (true, true) => {
                    "SELECT profile_json FROM model_profiles WHERE provider_id = ?1
                     ORDER BY display_name, id, revision"
                }
            };
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(provider_id) = provider_id {
                statement
                    .query_map([provider_id.to_string()], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn model_profile_references(
        &self,
        model_profile_id: ModelProfileId,
    ) -> Result<Vec<RegistryReference>, StorageError> {
        self.with_connection(|connection| {
            let model = model_profile_id.to_string();
            let mut references = Vec::new();
            let mut statement = connection.prepare(
                "SELECT project_id || ':' || match_kind || ':' || match_value
                 FROM project_model_bindings WHERE model_profile_id = ?1 ORDER BY project_id",
            )?;
            for location in statement
                .query_map([&model], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
            {
                references.push(RegistryReference {
                    kind: "project_model_binding".to_owned(),
                    location,
                });
            }
            let mut statement = connection.prepare(
                "SELECT id FROM provider_probe_usage WHERE model_profile_id = ?1 ORDER BY created_at",
            )?;
            for location in statement
                .query_map([&model], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
            {
                references.push(RegistryReference {
                    kind: "active_probe_usage".to_owned(),
                    location,
                });
            }
            append_json_references(
                connection,
                "SELECT id, draft_json FROM workflow_drafts",
                &model,
                "workflow_draft",
                &mut references,
            )?;
            append_json_references(
                connection,
                "SELECT workflow_id || '@' || version, version_json FROM workflow_versions",
                &model,
                "published_workflow",
                &mut references,
            )?;
            append_json_references(
                connection,
                "SELECT id, workflow_snapshot_json FROM runs WHERE workflow_snapshot_json IS NOT NULL",
                &model,
                "run_snapshot",
                &mut references,
            )?;
            Ok(references)
        })
    }

    pub fn delete_model_profile(
        &self,
        model_profile_id: ModelProfileId,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let deleted = connection.execute(
                "DELETE FROM model_profiles WHERE id = ?1",
                [model_profile_id.to_string()],
            )?;
            if deleted == 0 {
                return Err(StorageError::ModelProfileNotFound(model_profile_id, 0));
            }
            Ok(())
        })
    }

    pub fn record_provider_probe_usage(
        &self,
        usage: &ProviderProbeUsage,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO provider_probe_usage
                 (id, provider_id, model_profile_id, model_profile_revision, request_id,
                  input_tokens, output_tokens, total_tokens, cost, currency, duration_ms,
                  succeeded, safe_message, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    usage.id.to_string(),
                    usage.provider_id.to_string(),
                    usage.model_profile_id.to_string(),
                    sqlite_u64(usage.model_profile_revision),
                    usage.request_id,
                    usage.input_tokens.map(sqlite_u64),
                    usage.output_tokens.map(sqlite_u64),
                    usage.total_tokens.map(sqlite_u64),
                    usage.cost,
                    usage.currency,
                    sqlite_u64(usage.duration_ms),
                    usage.succeeded,
                    usage.safe_message,
                    usage.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_provider_probe_usage(
        &self,
        model_profile_id: Option<ModelProfileId>,
    ) -> Result<Vec<ProviderProbeUsage>, StorageError> {
        self.with_connection(|connection| {
            let sql = if model_profile_id.is_some() {
                "SELECT id, provider_id, model_profile_id, model_profile_revision, request_id,
                 input_tokens, output_tokens, total_tokens, cost, currency, duration_ms,
                 succeeded, safe_message, created_at
                 FROM provider_probe_usage WHERE model_profile_id = ?1 ORDER BY created_at DESC"
            } else {
                "SELECT id, provider_id, model_profile_id, model_profile_revision, request_id,
                 input_tokens, output_tokens, total_tokens, cost, currency, duration_ms,
                 succeeded, safe_message, created_at
                 FROM provider_probe_usage ORDER BY created_at DESC"
            };
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(model_profile_id) = model_profile_id {
                statement
                    .query_map(
                        [model_profile_id.to_string()],
                        provider_probe_usage_from_row,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], provider_probe_usage_from_row)?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter().map(parse_provider_probe_usage).collect()
        })
    }

    pub fn save_project_model_binding(
        &self,
        binding: &ProjectModelBinding,
        actor: BindingMutationActor,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let match_value = binding_match_value(binding)?;
            let existing = connection
                .query_row(
                    "SELECT binding_json FROM project_model_bindings
                     WHERE project_id = ?1 AND match_kind = ?2 AND match_value = ?3",
                    params![
                        binding.project_id.to_string(),
                        enum_string(binding.match_kind)?,
                        match_value,
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|json| serde_json::from_str::<ProjectModelBinding>(&json))
                .transpose()?;
            if existing
                .as_ref()
                .is_some_and(|existing| existing.locked && actor == BindingMutationActor::Agent)
            {
                return Err(StorageError::ModelBindingLocked);
            }
            let model_json = connection
                .query_row(
                    "SELECT profile_json FROM model_profiles WHERE id = ?1
                     ORDER BY revision DESC LIMIT 1",
                    [binding.model_profile_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::ModelProfileNotFound(
                    binding.model_profile_id,
                    0,
                ))?;
            let model: ModelProfile = serde_json::from_str(&model_json)?;
            binding
                .validate_for_model(&model)
                .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
            if let Some(existing) = existing
                && existing.id != binding.id
            {
                connection.execute(
                    "DELETE FROM project_model_bindings WHERE id = ?1",
                    [existing.id.to_string()],
                )?;
            }
            connection.execute(
                "INSERT INTO project_model_bindings
                 (id, project_id, match_kind, match_value, capability, role, model_profile_id,
                  locked, binding_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                   project_id = excluded.project_id,
                   match_kind = excluded.match_kind,
                   match_value = excluded.match_value,
                   capability = excluded.capability,
                   role = excluded.role,
                   model_profile_id = excluded.model_profile_id,
                   locked = excluded.locked,
                   binding_json = excluded.binding_json",
                params![
                    binding.id.to_string(),
                    binding.project_id.to_string(),
                    enum_string(binding.match_kind)?,
                    binding_match_value(binding)?,
                    enum_string(binding.capability)?,
                    enum_string(binding.role)?,
                    binding.model_profile_id.to_string(),
                    binding.locked,
                    serde_json::to_string(binding)?,
                    binding.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_project_model_bindings(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectModelBinding>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT binding_json FROM project_model_bindings
                 WHERE project_id = ?1 ORDER BY match_kind, match_value, id",
            )?;
            statement
                .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect()
        })
    }

    pub fn delete_project_model_binding(
        &self,
        binding_id: ModelBindingId,
        actor: BindingMutationActor,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let existing = connection
                .query_row(
                    "SELECT binding_json FROM project_model_bindings WHERE id = ?1",
                    [binding_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::ModelBindingNotFound(binding_id))?;
            let existing: ProjectModelBinding = serde_json::from_str(&existing)?;
            if existing.locked && actor == BindingMutationActor::Agent {
                return Err(StorageError::ModelBindingLocked);
            }
            connection.execute(
                "DELETE FROM project_model_bindings WHERE id = ?1",
                [binding_id.to_string()],
            )?;
            Ok(())
        })
    }

    pub fn save_global_model_defaults(
        &self,
        defaults: &GlobalModelDefaults,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            let mut models = BTreeMap::new();
            for model_profile_id in [
                defaults.pipeline_builder,
                defaults.vision_language,
                defaults.text_generation,
            ]
            .into_iter()
            .flatten()
            {
                let json = connection
                    .query_row(
                        "SELECT profile_json FROM model_profiles WHERE id = ?1
                         ORDER BY revision DESC LIMIT 1",
                        [model_profile_id.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                    .ok_or(StorageError::ModelProfileNotFound(model_profile_id, 0))?;
                models.insert(model_profile_id, serde_json::from_str(&json)?);
            }
            defaults
                .validate(&models)
                .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
            connection.execute(
                "INSERT INTO global_model_defaults(singleton, defaults_json, updated_at)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(singleton) DO UPDATE SET
                   defaults_json = excluded.defaults_json,
                   updated_at = excluded.updated_at",
                params![serde_json::to_string(defaults)?, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn get_global_model_defaults(&self) -> Result<GlobalModelDefaults, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT defaults_json FROM global_model_defaults WHERE singleton = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map_or_else(
                    || Ok(GlobalModelDefaults::default()),
                    |json| serde_json::from_str(&json).map_err(StorageError::from),
                )
        })
    }

    pub fn save_workflow_sample_test(
        &self,
        sample_test: &WorkflowSampleTest,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO workflow_sample_tests
                 (draft_id, project_id, report_json, completed_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(draft_id) DO UPDATE SET
                   project_id = excluded.project_id,
                   report_json = excluded.report_json,
                   completed_at = excluded.completed_at",
                params![
                    sample_test.draft_id,
                    sample_test.project_id,
                    serde_json::to_string(&sample_test.report)?,
                    sample_test.completed_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_workflow_sample_test(
        &self,
        draft_id: &str,
    ) -> Result<Option<WorkflowSampleTest>, StorageError> {
        self.with_connection(|connection| {
            let row = connection
                .query_row(
                    "SELECT draft_id, project_id, report_json, completed_at
                     FROM workflow_sample_tests WHERE draft_id = ?1",
                    [draft_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?;
            row.map(|(draft_id, project_id, report_json, completed_at)| {
                Ok(WorkflowSampleTest {
                    draft_id,
                    project_id,
                    report: serde_json::from_str(&report_json)?,
                    completed_at: DateTime::parse_from_rfc3339(&completed_at)
                        .map_err(|error| {
                            StorageError::InvalidEnum(format!(
                                "invalid sample test timestamp: {error}"
                            ))
                        })?
                        .with_timezone(&Utc),
                })
            })
            .transpose()
        })
    }

    pub fn save_agent_session(&self, session: &AgentSession) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO agent_sessions
                 (id, project_id, kind, status, session_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                   project_id = excluded.project_id,
                   status = excluded.status,
                   session_json = excluded.session_json,
                   updated_at = excluded.updated_at",
                params![
                    session.id.to_string(),
                    session.project_id,
                    format!("{:?}", session.kind).to_ascii_lowercase(),
                    format!("{:?}", session.status).to_ascii_lowercase(),
                    serde_json::to_string(session)?,
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_agent_sessions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<AgentSession>, StorageError> {
        self.with_connection(|connection| {
            let (sql, parameter) = project_id.map_or(
                (
                    "SELECT session_json FROM agent_sessions ORDER BY updated_at DESC",
                    None,
                ),
                |project_id| {
                    (
                        "SELECT session_json FROM agent_sessions WHERE project_id = ?1 ORDER BY updated_at DESC",
                        Some(project_id),
                    )
                },
            );
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(project_id) = parameter {
                statement
                    .query_map([project_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn get_agent_session(&self, id: Uuid) -> Result<AgentSession, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT session_json FROM agent_sessions WHERE id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("Agent Session {id} was not found"))
                })?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn schema_tables(&self) -> Result<Vec<String>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )?;
            let names = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(names)
        })
    }

    pub fn list_events(&self, run_id: RunId) -> Result<Vec<RunEvent>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT event_json FROM run_events WHERE run_id = ?1 ORDER BY sequence")?;
            statement
                .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn list_validation_issues(
        &self,
        run_id: RunId,
    ) -> Result<Vec<ValidationIssue>, StorageError> {
        self.with_connection(|connection| {
            query_json_rows::<ValidationIssue>(
                connection,
                "SELECT issue_json FROM validation_issues WHERE run_id = ?1 ORDER BY id",
                run_id,
            )
        })
    }

    pub fn list_annotations(&self, run_id: RunId) -> Result<Vec<Annotation>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT annotation_json FROM annotations WHERE run_id = ?1 ORDER BY created_at",
            )?;
            statement
                .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    /// Returns persisted annotations from an exact prior Run that can act as an explicit,
    /// replayable input to a published Workflow. The Project identity prevents a Draft from
    /// reading annotation history owned by another Project.
    pub fn list_project_annotations_for_run(
        &self,
        project_id: ProjectId,
        source_run_id: RunId,
    ) -> Result<Vec<Annotation>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT annotations.annotation_json
                 FROM annotations
                 INNER JOIN runs ON runs.id = annotations.run_id
                 WHERE runs.project_id = ?1
                   AND annotations.run_id = ?2
                 ORDER BY annotations.created_at",
            )?;
            statement
                .query_map(
                    params![project_id.to_string(), source_run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn pending_review_count(&self) -> Result<usize, StorageError> {
        let review_status = enum_string(ReviewStatus::NeedsReview)?;
        self.with_connection(|connection| {
            let count = connection.query_row(
                "SELECT COUNT(*) FROM annotations WHERE review_status = ?1",
                [review_status],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(usize::try_from(count).unwrap_or(usize::MAX))
        })
    }

    pub fn list_artifacts(&self, run_id: RunId) -> Result<Vec<VisionArtifact>, StorageError> {
        self.with_connection(|connection| {
            query_json_rows::<VisionArtifact>(
                connection,
                "SELECT artifact_json FROM vision_artifacts WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )
        })
    }

    pub fn find_annotation(
        &self,
        annotation_id: annotagent_core::AnnotationId,
    ) -> Result<Option<(RunId, Annotation)>, StorageError> {
        self.with_connection(|connection| {
            let row = connection
                .query_row(
                    "SELECT run_id, annotation_json FROM annotations WHERE id = ?1",
                    [annotation_id.to_string()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            row.map(|(run_id, annotation)| {
                Ok((
                    run_id.parse().map_err(|error| {
                        StorageError::InvalidEnum(format!("invalid run id: {error}"))
                    })?,
                    serde_json::from_str(&annotation)?,
                ))
            })
            .transpose()
        })
    }

    pub fn update_annotation(
        &self,
        annotation: &Annotation,
        reason: Option<&str>,
    ) -> Result<AnnotationRevision, StorageError> {
        annotation
            .validate()
            .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let before_json: String = transaction
                .query_row(
                    "SELECT annotation_json FROM annotations WHERE id = ?1",
                    [annotation.id.to_string()],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("annotation {} was not found", annotation.id))
                })?;
            let before: Annotation = serde_json::from_str(&before_json)?;
            let parent_revision_id = transaction
                .query_row(
                    "SELECT revision_id FROM annotation_revisions
                     WHERE annotation_id = ?1 ORDER BY created_at DESC LIMIT 1",
                    [annotation.id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|id| id.parse())
                .transpose()
                .map_err(|error| {
                    StorageError::InvalidEnum(format!("invalid revision id: {error}"))
                })?;
            let revision = AnnotationRevision {
                revision_id: AnnotationRevisionId::new(),
                annotation_id: annotation.id,
                parent_revision_id,
                before: Some(before.snapshot()),
                after: Some(annotation.snapshot()),
                actor: RevisionActor::Human,
                reason: reason.map(str::to_owned),
                created_at: Utc::now(),
            };
            revision
                .validate()
                .map_err(|error| StorageError::InvalidEnum(error.to_string()))?;
            transaction.execute(
                "UPDATE annotations SET label = ?2, review_status = ?3, annotation_json = ?4
                 WHERE id = ?1",
                params![
                    annotation.id.to_string(),
                    annotation.label.as_ref().map(LabelId::as_str),
                    enum_string(annotation.review_status)?,
                    serde_json::to_string(annotation)?,
                ],
            )?;
            transaction.execute(
                "INSERT INTO annotation_revisions
                 (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    revision.revision_id.to_string(),
                    revision.annotation_id.to_string(),
                    revision.parent_revision_id.map(|id| id.to_string()),
                    serde_json::to_string(&revision)?,
                    revision.created_at.to_rfc3339(),
                ],
            )?;
            transaction.execute(
                "UPDATE review_queue SET status = ?2, resolved_at = ?3 WHERE annotation_id = ?1",
                params![
                    annotation.id.to_string(),
                    enum_string(annotation.review_status)?,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(revision)
        })
    }

    pub fn list_revisions(
        &self,
        annotation_id: annotagent_core::AnnotationId,
    ) -> Result<Vec<AnnotationRevision>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT revision_json FROM annotation_revisions
                 WHERE annotation_id = ?1 ORDER BY created_at",
            )?;
            statement
                .query_map([annotation_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| {
                    let json = row?;
                    serde_json::from_str(&json).map_err(StorageError::from)
                })
                .collect()
        })
    }

    pub fn run_status(&self, run_id: RunId) -> Result<RunStatus, StorageError> {
        self.with_connection(|connection| {
            let status = connection
                .query_row(
                    "SELECT status FROM runs WHERE id = ?1",
                    [run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::RunNotFound(run_id))?;
            serde_json::from_value(serde_json::Value::String(status)).map_err(StorageError::from)
        })
    }

    pub fn list_runs(&self) -> Result<Vec<HistoryRun>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, project_id, project_name, skill_id, provider, model, status,
                        project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at
                 FROM runs ORDER BY created_at DESC",
            )?;
            statement
                .query_map([], history_run_from_row)?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    /// Assign legacy Runs to a Project only after the application has established that the
    /// legacy display name has exactly one owner. Existing stable ownership is never rewritten.
    pub fn backfill_legacy_run_project_id(
        &self,
        project_id: ProjectId,
        unique_legacy_project_name: &str,
    ) -> Result<usize, StorageError> {
        self.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE runs SET project_id = ?1 WHERE project_id IS NULL AND project_name = ?2",
                    params![project_id.to_string(), unique_legacy_project_name],
                )
                .map_err(StorageError::from)
        })
    }

    /// Insert or resolve a durable Project-scoped image identity based on its content digest.
    pub fn ensure_project_image(
        &self,
        project_id: ProjectId,
        relative_path: &str,
        sha256: &str,
        metadata_json: &str,
    ) -> Result<StoredProjectImage, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let existing_id = transaction
                .query_row(
                    "SELECT id FROM images
                     WHERE project_id = ?1 AND relative_path = ?2
                     ORDER BY imported_at DESC LIMIT 1",
                    params![project_id.to_string(), relative_path],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let image_id = if let Some(id) = existing_id {
                id.parse().map_err(|error| {
                    StorageError::InvalidEnum(format!("stored ImageId is invalid: {error}"))
                })?
            } else {
                ImageId(Uuid::new_v5(&project_id.0, relative_path.as_bytes()))
            };
            let imported_at = Utc::now().to_rfc3339();
            transaction.execute(
                "INSERT INTO images (id, project_id, relative_path, sha256, metadata_json, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   project_id = excluded.project_id,
                   relative_path = excluded.relative_path,
                   sha256 = excluded.sha256,
                   metadata_json = excluded.metadata_json",
                params![
                    image_id.to_string(),
                    project_id.to_string(),
                    relative_path,
                    sha256,
                    metadata_json,
                    imported_at,
                ],
            )?;
            let stored = transaction.query_row(
                "SELECT id, project_id, relative_path, sha256, metadata_json, imported_at
                 FROM images WHERE id = ?1",
                [image_id.to_string()],
                stored_project_image_from_row,
            )?;
            transaction.commit()?;
            Ok(stored)
        })
    }

    pub fn get_project_image(
        &self,
        project_id: ProjectId,
        image_id: ImageId,
    ) -> Result<Option<StoredProjectImage>, StorageError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, project_id, relative_path, sha256, metadata_json, imported_at
                     FROM images WHERE project_id = ?1 AND id = ?2",
                    params![project_id.to_string(), image_id.to_string()],
                    stored_project_image_from_row,
                )
                .optional()
                .map_err(StorageError::from)
        })
    }

    pub fn register_run_image(
        &self,
        run_id: RunId,
        image_id: ImageId,
        status: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO run_images (run_id, image_id, status) VALUES (?1, ?2, ?3)
                 ON CONFLICT(run_id, image_id) DO UPDATE SET status = excluded.status",
                params![run_id.to_string(), image_id.to_string(), status],
            )?;
            Ok(())
        })
    }

    /// Authoritative image ownership for a Run, including evidence from legacy tables.
    pub fn run_image_ids(&self, run_id: RunId) -> Result<BTreeSet<ImageId>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT image_id FROM run_images WHERE run_id = ?1
                 UNION SELECT image_id FROM task_runs WHERE run_id = ?1
                 UNION SELECT image_id FROM annotations WHERE run_id = ?1
                 UNION SELECT image_id FROM vision_artifacts WHERE run_id = ?1
                 UNION SELECT image_id FROM model_messages WHERE run_id = ?1 AND image_id IS NOT NULL
                 UNION SELECT image_id FROM batch_images WHERE child_run_id = ?1",
            )?;
            statement
                .query_map([run_id.to_string()], |row| {
                    row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
                })?
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(StorageError::from)
        })
    }

    /// Return the persisted image execution rows for one Run. The status is the latest
    /// task-level state written for that exact Run/Image pair.
    pub fn run_images(&self, run_id: RunId) -> Result<Vec<(ImageId, String)>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT image_id, status FROM run_images WHERE run_id = ?1 ORDER BY image_id",
            )?;
            statement
                .query_map([run_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok((image_id, row.get(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StorageError::from)
        })
    }

    /// Resolve the newest Run status independently for every image in a Project.
    /// Display ordering never participates in this association.
    pub fn latest_project_image_run_statuses(
        &self,
        project_id: ProjectId,
    ) -> Result<BTreeMap<ImageId, String>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT ri.image_id, r.status
                 FROM run_images ri
                 JOIN runs r ON r.id = ri.run_id
                 WHERE r.project_id = ?1
                 ORDER BY r.updated_at DESC, r.id DESC",
            )?;
            let rows = statement
                .query_map([project_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok((image_id, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut statuses = BTreeMap::new();
            for (image_id, status) in rows {
                statuses.entry(image_id).or_insert(status);
            }
            Ok(statuses)
        })
    }

    pub fn list_task_runs(&self, run_id: RunId) -> Result<Vec<HistoryTaskRun>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT image_id, task_id, status, reason, updated_at
                 FROM task_runs WHERE run_id = ?1 ORDER BY updated_at, task_id",
            )?;
            statement
                .query_map([run_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let status_text = row.get::<_, String>(2)?;
                    let status = serde_json::from_value(serde_json::Value::String(status_text))
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(HistoryTaskRun {
                        run_id,
                        image_id,
                        task_id: TaskId::from(row.get::<_, String>(1)?),
                        status,
                        reason: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                })?
                .map(|row| row.map_err(StorageError::from))
                .collect()
        })
    }

    pub fn reserve_project_run(
        &self,
        project_id: ProjectId,
        run_id: RunId,
        idempotency_key: Option<&str>,
    ) -> Result<RunStartReservation, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            if let Some(key) = idempotency_key {
                let existing = transaction
                    .query_row(
                        "SELECT r.run_id, COALESCE(a.status, runs.status, 'interrupted')
                         FROM run_start_requests r
                         LEFT JOIN active_project_runs a ON a.run_id = r.run_id
                         LEFT JOIN runs ON runs.id = r.run_id
                         WHERE r.project_id = ?1 AND r.idempotency_key = ?2",
                        params![project_id.to_string(), key],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                if let Some((existing_id, status)) = existing {
                    transaction.commit()?;
                    return Ok(RunStartReservation::Idempotent {
                        run_id: parse_run_id(&existing_id)?,
                        status: parse_run_status(&status)?,
                    });
                }
            }
            let active = transaction
                .query_row(
                    "SELECT run_id, status FROM active_project_runs WHERE project_id = ?1",
                    [project_id.to_string()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            if let Some((active_id, status)) = active {
                transaction.commit()?;
                return Ok(RunStartReservation::Conflict {
                    run_id: parse_run_id(&active_id)?,
                    status: parse_run_status(&status)?,
                });
            }
            let now = Utc::now().to_rfc3339();
            if let Some(key) = idempotency_key {
                transaction.execute(
                    "INSERT INTO run_start_requests (project_id, idempotency_key, run_id, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![project_id.to_string(), key, run_id.to_string(), now],
                )?;
            }
            transaction.execute(
                "INSERT INTO active_project_runs (project_id, run_id, status, idempotency_key, updated_at)
                 VALUES (?1, ?2, 'pending', ?3, ?4)",
                params![project_id.to_string(), run_id.to_string(), idempotency_key, now],
            )?;
            transaction.commit()?;
            Ok(RunStartReservation::Reserved)
        })
    }

    pub fn reconcile_interrupted_runs(&self) -> Result<Vec<RunId>, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let run_ids = {
                let mut statement =
                    transaction.prepare("SELECT run_id FROM active_project_runs")?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .map(|row| parse_run_id(&row?))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let now = Utc::now().to_rfc3339();
            for run_id in &run_ids {
                transaction.execute(
                    "UPDATE runs SET status = 'interrupted', terminal_reason = ?2, updated_at = ?3
                     WHERE id = ?1 AND status IN ('pending', 'running', 'paused', 'awaiting_review')",
                    params![
                        run_id.to_string(),
                        "run was interrupted because no worker lease survived server startup",
                        now
                    ],
                )?;
            }
            transaction.execute("DELETE FROM active_project_runs", [])?;
            transaction.commit()?;
            Ok(run_ids)
        })
    }

    pub fn save_workflow_draft(&self, draft: &WorkflowDraft) -> Result<(), StorageError> {
        let status = enum_string(draft.status)?;
        let draft_json = serde_json::to_string(draft)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO workflow_drafts
                 (id, project_id, status, draft_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   status = excluded.status,
                   draft_json = excluded.draft_json,
                   updated_at = excluded.updated_at",
                params![
                    draft.id,
                    draft.project_id,
                    status,
                    draft_json,
                    draft.created_at.to_rfc3339(),
                    draft.updated_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_workflow_drafts(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<WorkflowDraft>, StorageError> {
        self.with_connection(|connection| {
            let (sql, parameter) = project_id.map_or(
                ("SELECT draft_json FROM workflow_drafts ORDER BY updated_at DESC", None),
                |project_id| {
                    (
                        "SELECT draft_json FROM workflow_drafts WHERE project_id = ?1 ORDER BY updated_at DESC",
                        Some(project_id),
                    )
                },
            );
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(project_id) = parameter {
                statement
                    .query_map([project_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn get_workflow_draft(&self, id: &str) -> Result<WorkflowDraft, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT draft_json FROM workflow_drafts WHERE id = ?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("workflow draft {id:?} was not found"))
                })?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn publish_workflow_draft(
        &self,
        draft: &WorkflowDraft,
        content_hash: String,
        snapshot: WorkflowSnapshot,
    ) -> Result<PublishedWorkflowVersion, StorageError> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let next_version: u32 = transaction.query_row(
                "SELECT COALESCE(MAX(version), 0) + 1 FROM workflow_versions WHERE workflow_id = ?1",
                [&draft.id],
                |row| row.get(0),
            )?;
            let mut published_draft = draft.clone();
            published_draft.status = WorkflowDraftStatus::Published;
            published_draft.updated_at = Utc::now();
            let version = PublishedWorkflowVersion {
                workflow_id: draft.id.clone(),
                version: next_version,
                project_id: draft.project_id.clone(),
                source_draft_id: draft.id.clone(),
                content_hash,
                draft: published_draft.clone(),
                snapshot,
                published_at: Utc::now(),
            };
            transaction.execute(
                "INSERT INTO workflow_versions
                 (workflow_id, version, project_id, source_draft_id, version_json, published_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    version.workflow_id,
                    version.version,
                    version.project_id,
                    version.source_draft_id,
                    serde_json::to_string(&version)?,
                    version.published_at.to_rfc3339()
                ],
            )?;
            transaction.execute(
                "UPDATE workflow_drafts SET status = 'published', draft_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    draft.id,
                    serde_json::to_string(&published_draft)?,
                    published_draft.updated_at.to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(version)
        })
    }

    pub fn list_published_workflow_versions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<PublishedWorkflowVersion>, StorageError> {
        self.with_connection(|connection| {
            let (sql, parameter) = project_id.map_or(
                (
                    "SELECT version_json FROM workflow_versions ORDER BY project_id, published_at, workflow_id, version",
                    None,
                ),
                |project_id| {
                    (
                        "SELECT version_json FROM workflow_versions WHERE project_id = ?1 ORDER BY published_at, workflow_id, version",
                        Some(project_id),
                    )
                },
            );
            let mut statement = connection.prepare(sql)?;
            let rows = if let Some(project_id) = parameter {
                statement
                    .query_map([project_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn get_published_workflow_version(
        &self,
        workflow_id: &str,
        version: u32,
    ) -> Result<PublishedWorkflowVersion, StorageError> {
        self.with_connection(|connection| {
            let json = connection
                .query_row(
                    "SELECT version_json FROM workflow_versions WHERE workflow_id = ?1 AND version = ?2",
                    params![workflow_id, version],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!(
                        "published workflow {workflow_id:?} version {version} was not found"
                    ))
                })?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
    }

    pub fn history(&self, run_id: RunId) -> Result<HistoryDocument, StorageError> {
        self.with_connection(|connection| {
            let run = connection
                .query_row(
                    "SELECT id, project_id, project_name, skill_id, provider, model, status,
                            project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at
                     FROM runs WHERE id = ?1",
                    [run_id.to_string()],
                    history_run_from_row,
                )
                .optional()?
                .ok_or(StorageError::RunNotFound(run_id))?;
            let events = query_json_rows::<RunEvent>(
                connection,
                "SELECT event_json FROM run_events WHERE run_id = ?1 ORDER BY sequence",
                run_id,
            )?;
            let annotations = query_json_rows::<Annotation>(
                connection,
                "SELECT annotation_json FROM annotations WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )?;
            let revisions = query_json_rows::<AnnotationRevision>(
                connection,
                "SELECT r.revision_json FROM annotation_revisions r
                 JOIN annotations a ON a.id = r.annotation_id
                 WHERE a.run_id = ?1 ORDER BY r.created_at",
                run_id,
            )?;
            let mut task_statement = connection.prepare(
                "SELECT image_id, task_id, status, reason, updated_at
                 FROM task_runs WHERE run_id = ?1 ORDER BY updated_at, task_id",
            )?;
            let task_runs = task_statement
                .query_map([run_id.to_string()], |row| {
                    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let status_text = row.get::<_, String>(2)?;
                    let status = serde_json::from_value(serde_json::Value::String(status_text))
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(HistoryTaskRun {
                        run_id,
                        image_id,
                        task_id: TaskId::from(row.get::<_, String>(1)?),
                        status,
                        reason: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let usage = query_json_rows::<UsageRecord>(
                connection,
                "SELECT usage_json FROM usage_records WHERE run_id = ?1 ORDER BY id",
                run_id,
            )?;
            let mut message_statement = connection.prepare(
                "SELECT image_id, task_id, message_json, created_at
                 FROM model_messages WHERE run_id = ?1 ORDER BY sequence",
            )?;
            let model_messages = message_statement
                .query_map([run_id.to_string()], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .map(|row| {
                    let (image_id, task_id, message, created_at) = row?;
                    Ok(HistoryModelMessage {
                        image_id: image_id.map(|value| value.parse()).transpose().map_err(
                            |error| StorageError::InvalidEnum(format!("invalid image id: {error}")),
                        )?,
                        task_id: task_id.map(TaskId::from),
                        message: serde_json::from_str(&message)?,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            let artifacts = query_json_rows::<VisionArtifact>(
                connection,
                "SELECT artifact_json FROM vision_artifacts WHERE run_id = ?1 ORDER BY created_at",
                run_id,
            )?;
            let mut statement = connection.prepare(
                "SELECT call_id, name, arguments_json, result_json, error, created_at
                 FROM tool_calls WHERE run_id = ?1 ORDER BY created_at",
            )?;
            let tool_calls = statement
                .query_map([run_id.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?
                .map(|row| {
                    let (call_id, name, arguments, result, error, created_at) = row?;
                    Ok(HistoryToolCall {
                        call_id: ToolCallId::new(call_id),
                        name,
                        arguments: sanitize_trace_value(&serde_json::from_str(&arguments)?),
                        result: result
                            .map(|value| serde_json::from_str(&value))
                            .transpose()?,
                        error,
                        created_at,
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            Ok(HistoryDocument {
                schema_version: annotagent_core::HISTORY_SCHEMA_VERSION,
                run,
                events,
                annotations,
                revisions,
                task_runs,
                usage,
                model_messages,
                artifacts,
                tool_calls,
            })
        })
    }

    pub fn export_history(&self, run_id: RunId, path: &Path) -> Result<(), StorageError> {
        let bytes = serde_json::to_vec_pretty(&self.history(run_id)?)?;
        std::fs::write(path, bytes)
            .map_err(|error| StorageError::Serialization(serde_json::Error::io(error)))
    }

    pub fn import_history(
        &self,
        mut document: HistoryDocument,
    ) -> Result<HistoryImportReport, StorageError> {
        if document.schema_version != annotagent_core::HISTORY_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedHistoryVersion(
                document.schema_version,
            ));
        }
        self.with_connection(|connection| {
            let original_run_id = document.run.id;
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1)",
                [original_run_id.to_string()],
                |row| row.get(0),
            )?;
            let target_run_id = if exists { RunId::new() } else { original_run_id };
            let ids_remapped = target_run_id != original_run_id;
            document.run.id = target_run_id;
            let mut annotation_ids = BTreeMap::new();
            let mut artifact_ids = BTreeMap::new();
            let mut tool_call_ids = BTreeMap::new();
            if ids_remapped {
                for annotation in &mut document.annotations {
                    let replacement = annotagent_core::AnnotationId::new();
                    annotation_ids.insert(annotation.id, replacement);
                    annotation.id = replacement;
                }
                for artifact in &mut document.artifacts {
                    let replacement = ArtifactId::new();
                    artifact_ids.insert(artifact.id, replacement);
                    artifact.id = replacement;
                }
                for call in &mut document.tool_calls {
                    let replacement = ToolCallId::new(format!("imported-{}", Uuid::new_v4()));
                    tool_call_ids.insert(call.call_id.clone(), replacement.clone());
                    call.call_id = replacement;
                }
                for annotation in &mut document.annotations {
                    remap_annotation_value(
                        &mut annotation.value,
                        &annotation_ids,
                        &artifact_ids,
                    );
                    for artifact_id in &mut annotation.provenance.artifact_ids {
                        if let Some(replacement) = artifact_ids.get(artifact_id) {
                            *artifact_id = *replacement;
                        }
                    }
                }
                for artifact in &mut document.artifacts {
                    if let Some(replaces) = artifact.replaces_artifact_id
                        && let Some(replacement) = artifact_ids.get(&replaces)
                    {
                        artifact.replaces_artifact_id = Some(*replacement);
                    }
                    for input in &mut artifact.provenance.input_artifact_ids {
                        if let Some(replacement) = artifact_ids.get(input) {
                            *input = *replacement;
                        }
                    }
                    if let VisionArtifactValue::Relations { relations } = &mut artifact.value {
                        remap_relations(relations, &annotation_ids, &artifact_ids);
                    }
                }
                let mut revision_ids = BTreeMap::new();
                for revision in &mut document.revisions {
                    let replacement = annotagent_core::AnnotationRevisionId::new();
                    revision_ids.insert(revision.revision_id, replacement);
                    revision.revision_id = replacement;
                    if let Some(annotation_id) = annotation_ids.get(&revision.annotation_id) {
                        revision.annotation_id = *annotation_id;
                    }
                }
                for revision in &mut document.revisions {
                    if let Some(parent) = revision.parent_revision_id
                        && let Some(replacement) = revision_ids.get(&parent)
                    {
                        revision.parent_revision_id = Some(*replacement);
                    }
                    if let Some(before) = &mut revision.before {
                        remap_annotation_value(
                            &mut before.value,
                            &annotation_ids,
                            &artifact_ids,
                        );
                    }
                    if let Some(after) = &mut revision.after {
                        remap_annotation_value(
                            &mut after.value,
                            &annotation_ids,
                            &artifact_ids,
                        );
                    }
                }
                let string_ids = annotation_ids
                    .iter()
                    .map(|(source, target)| (source.to_string(), target.to_string()))
                    .chain(
                        artifact_ids
                            .iter()
                            .map(|(source, target)| (source.to_string(), target.to_string())),
                    )
                    .chain(tool_call_ids.iter().map(|(source, target)| {
                        (source.as_str().to_owned(), target.as_str().to_owned())
                    }))
                    .collect::<BTreeMap<_, _>>();
                for entry in &mut document.model_messages {
                    if let Some(call_id) = &entry.message.tool_call_id
                        && let Some(replacement) = tool_call_ids.get(call_id)
                    {
                        entry.message.tool_call_id = Some(replacement.clone());
                    }
                    for call in &mut entry.message.tool_calls {
                        if let Some(replacement) = tool_call_ids.get(&call.id) {
                            call.id = replacement.clone();
                        }
                        remap_json_strings(&mut call.arguments, &string_ids);
                    }
                    if let Ok(mut value) = serde_json::from_str(&entry.message.content) {
                        remap_json_strings(&mut value, &string_ids);
                        entry.message.content = serde_json::to_string(&value)?;
                    }
                }
                for call in &mut document.tool_calls {
                    remap_json_strings(&mut call.arguments, &string_ids);
                    if let Some(result) = &mut call.result {
                        remap_json_strings(&mut result.persisted_result, &string_ids);
                        remap_json_strings(&mut result.model_result, &string_ids);
                        for artifact in &mut result.artifacts {
                            if let Some(replacement) = artifact_ids.get(&artifact.id) {
                                artifact.id = *replacement;
                            }
                            if let Some(replaces) = artifact.replaces_artifact_id
                                && let Some(replacement) = artifact_ids.get(&replaces)
                            {
                                artifact.replaces_artifact_id = Some(*replacement);
                            }
                            remap_copy_ids(
                                &mut artifact.provenance.input_artifact_ids,
                                &artifact_ids,
                            );
                            if let VisionArtifactValue::Relations { relations } =
                                &mut artifact.value
                            {
                                remap_relations(relations, &annotation_ids, &artifact_ids);
                            }
                        }
                    }
                }
            }
            for event in &mut document.events {
                event.run_id = target_run_id;
                if ids_remapped {
                    event.event_id = annotagent_core::EventId::new();
                    match &mut event.payload {
                        RunEventPayload::Tool { call_id, .. } => {
                            if let Some(replacement) = tool_call_ids.get(call_id) {
                                *call_id = replacement.clone();
                            }
                        }
                        RunEventPayload::Annotation { annotation_ids: ids, .. } => {
                            remap_copy_ids(ids, &annotation_ids);
                        }
                        RunEventPayload::Artifact { artifact_ids: ids, .. } => {
                            remap_copy_ids(ids, &artifact_ids);
                        }
                        _ => {}
                    }
                }
            }
            for task_run in &mut document.task_runs {
                task_run.run_id = target_run_id;
            }
            let transaction = connection.unchecked_transaction()?;
            insert_history_run(&transaction, &document.run)?;
            for task_run in &document.task_runs {
                transaction.execute(
                    "INSERT INTO task_runs
                     (run_id, image_id, task_id, status, reason, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        target_run_id.to_string(),
                        task_run.image_id.to_string(),
                        task_run.task_id.as_str(),
                        enum_string(task_run.status)?,
                        task_run.reason,
                        task_run.updated_at,
                    ],
                )?;
            }
            for event in &document.events {
                transaction.execute(
                    "INSERT INTO run_events
                     (event_id, run_id, event_kind, event_json, occurred_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        event.event_id.to_string(),
                        target_run_id.to_string(),
                        enum_string(event.kind)?,
                        serde_json::to_string(event)?,
                        event.occurred_at.to_rfc3339()
                    ],
                )?;
            }
            for annotation in &document.annotations {
                transaction.execute(
                    "INSERT INTO annotations
                     (id, run_id, image_id, task_id, label, review_status, annotation_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        annotation.id.to_string(),
                        target_run_id.to_string(),
                        annotation.image_id.to_string(),
                        annotation.task_id.as_str(),
                        annotation.label.as_ref().map(LabelId::as_str),
                        enum_string(annotation.review_status)?,
                        serde_json::to_string(annotation)?,
                        annotation.created_at.to_rfc3339()
                    ],
                )?;
            }
            for revision in &document.revisions {
                transaction.execute(
                    "INSERT INTO annotation_revisions
                     (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        revision.revision_id.to_string(),
                        revision.annotation_id.to_string(),
                        revision.parent_revision_id.map(|id| id.to_string()),
                        serde_json::to_string(revision)?,
                        revision.created_at.to_rfc3339()
                    ],
                )?;
            }
            for usage in &document.usage {
                transaction.execute(
                    "INSERT INTO usage_records
                     (run_id, usage_json, input_tokens, output_tokens, total_tokens, cost, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        target_run_id.to_string(),
                        serde_json::to_string(usage)?,
                        usage.tokens.input_tokens.map(sqlite_u64),
                        usage.tokens.output_tokens.map(sqlite_u64),
                        usage.tokens.total_tokens.map(sqlite_u64),
                        usage.cost.total.to_string(),
                        usage.completed_at.to_rfc3339()
                    ],
                )?;
            }
            for entry in &document.model_messages {
                transaction.execute(
                    "INSERT INTO model_messages
                     (run_id, image_id, task_id, message_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        target_run_id.to_string(),
                        entry.image_id.map(|id| id.to_string()),
                        entry.task_id.as_ref().map(TaskId::as_str),
                        serde_json::to_string(&entry.message)?,
                        entry.created_at
                    ],
                )?;
            }
            for artifact in &document.artifacts {
                transaction.execute(
                    "INSERT INTO vision_artifacts
                     (artifact_id, run_id, image_id, task_id, source_node, validation_state,
                      artifact_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        artifact.id.to_string(),
                        target_run_id.to_string(),
                        artifact.image_id.to_string(),
                        artifact.task_id.as_ref().map(TaskId::as_str),
                        artifact.source_node,
                        enum_string(artifact.validation_state)?,
                        serde_json::to_string(artifact)?,
                        artifact.created_at.to_rfc3339()
                    ],
                )?;
            }
            for call in &document.tool_calls {
                transaction.execute(
                    "INSERT INTO tool_calls
                     (call_id, run_id, name, arguments_json, result_json, error, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        call.call_id.as_str(),
                        target_run_id.to_string(),
                        call.name,
                        serde_json::to_string(&sanitize_trace_value(&call.arguments))?,
                        call.result.as_ref().map(serde_json::to_string).transpose()?,
                        call.error,
                        call.created_at
                    ],
                )?;
            }
            transaction.commit()?;
            let warnings = if document.annotations.is_empty() {
                Vec::new()
            } else {
                vec![
                    "history import preserves image IDs and hashes but does not restore missing image files"
                        .to_owned(),
                ]
            };
            Ok(HistoryImportReport {
                run_id: target_run_id,
                ids_remapped,
                warnings,
            })
        })
    }

    pub fn save_correction(&self, record: &CorrectionRecord) -> Result<(), StorageError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO correction_records
                 (id, project_id, skill_id, task_id, predicted_label, corrected_label,
                  reason_code, record_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.id.to_string(),
                    record.project_id.to_string(),
                    record.skill_id,
                    record.task_id.as_str(),
                    record.predicted_label.as_ref().map(LabelId::as_str),
                    record.corrected_label.as_ref().map(LabelId::as_str),
                    record.reason_code,
                    serde_json::to_string(record)?,
                    record.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
    }

    pub fn save_geometry_correction(
        &self,
        report: &GeometryQualityReport,
        evidence: &GeometryCorrectionEvidence,
    ) -> Result<(), StorageError> {
        if evidence.quality_report_id != report.id
            || evidence.project_id != report.project_id
            || evidence.image_id != report.image_id
        {
            return Err(StorageError::InvalidEnum(
                "geometry report and correction evidence scopes do not match".to_owned(),
            ));
        }
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO geometry_quality_reports
                 (id, project_id, image_id, candidate_artifact_id, reference_artifact_id,
                  source, report_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    report.id.to_string(),
                    report.project_id.to_string(),
                    report.image_id.to_string(),
                    report.candidate_artifact_id.to_string(),
                    report.reference_artifact_id.map(|id| id.to_string()),
                    enum_string(report.source)?,
                    serde_json::to_string(report)?,
                    report.created_at.to_rfc3339(),
                ],
            )?;
            transaction.execute(
                "INSERT INTO geometry_correction_evidence
                 (quality_report_id, project_id, run_id, image_id, annotation_id,
                  source_node_id, source_model_profile_id, source_model_revision, reason,
                  evidence_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    evidence.quality_report_id.to_string(),
                    evidence.project_id.to_string(),
                    evidence.run_id.to_string(),
                    evidence.image_id.to_string(),
                    evidence.annotation_id.to_string(),
                    evidence.source_node_id.as_str(),
                    evidence.source_model_profile_id.map(|id| id.to_string()),
                    evidence
                        .source_model_revision
                        .map(|revision| i64::try_from(revision).unwrap_or(i64::MAX)),
                    enum_string(evidence.reason.clone())?,
                    serde_json::to_string(evidence)?,
                    evidence.created_at.to_rfc3339(),
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn list_project_geometry_corrections(
        &self,
        project_id: ProjectId,
        limit: usize,
    ) -> Result<Vec<(GeometryQualityReport, GeometryCorrectionEvidence)>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 1_000)).unwrap_or(1_000);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT q.report_json, e.evidence_json
                 FROM geometry_correction_evidence e
                 JOIN geometry_quality_reports q ON q.id = e.quality_report_id
                 WHERE e.project_id = ?1
                 ORDER BY e.created_at DESC LIMIT ?2",
            )?;
            let rows = statement
                .query_map(params![project_id.to_string(), limit], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|(report, evidence)| {
                    Ok((
                        serde_json::from_str(&report)?,
                        serde_json::from_str(&evidence)?,
                    ))
                })
                .collect()
        })
    }

    pub fn list_run_geometry_corrections(
        &self,
        run_id: RunId,
        limit: usize,
    ) -> Result<Vec<(GeometryQualityReport, GeometryCorrectionEvidence)>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 1_000)).unwrap_or(1_000);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT q.report_json, e.evidence_json
                 FROM geometry_correction_evidence e
                 JOIN geometry_quality_reports q ON q.id = e.quality_report_id
                 WHERE e.run_id = ?1
                 ORDER BY e.created_at DESC LIMIT ?2",
            )?;
            let rows = statement
                .query_map(params![run_id.to_string(), limit], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|(report, evidence)| {
                    Ok((
                        serde_json::from_str(&report)?,
                        serde_json::from_str(&evidence)?,
                    ))
                })
                .collect()
        })
    }

    pub fn save_project_geometry_policy(
        &self,
        policy: &ProjectGeometryPolicy,
    ) -> Result<(), StorageError> {
        policy.validate().map_err(StorageError::InvalidEnum)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO project_geometry_policies
                 (project_id, task_kind, policy_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, task_kind) DO UPDATE SET
                   policy_json = excluded.policy_json,
                   updated_at = excluded.updated_at",
                params![
                    policy.project_id.to_string(),
                    enum_string(policy.task_kind)?,
                    serde_json::to_string(policy)?,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_project_geometry_policies(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectGeometryPolicy>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT policy_json FROM project_geometry_policies
                 WHERE project_id = ?1 ORDER BY task_kind",
            )?;
            statement
                .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect()
        })
    }

    pub fn get_project_geometry_policy(
        &self,
        project_id: ProjectId,
        task_kind: TaskKind,
    ) -> Result<Option<ProjectGeometryPolicy>, StorageError> {
        self.with_connection(|connection| {
            let value = connection
                .query_row(
                    "SELECT policy_json FROM project_geometry_policies
                     WHERE project_id = ?1 AND task_kind = ?2",
                    params![project_id.to_string(), enum_string(task_kind)?],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
                .transpose()
        })
    }

    pub fn save_geometry_calibration(
        &self,
        report: &GeometryCalibrationReport,
    ) -> Result<(), StorageError> {
        report.key.validate().map_err(StorageError::InvalidEnum)?;
        report
            .thresholds
            .validate()
            .map_err(StorageError::InvalidEnum)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO geometry_calibration_reports
                 (id, project_id, task_id, label_id, model_profile_id, model_profile_revision,
                  node_definition_id, node_config_hash, status, report_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    report.id.to_string(),
                    report.key.project_id.to_string(),
                    report.key.task_id.as_str(),
                    report.key.label_id.as_ref().map(LabelId::as_str),
                    report.key.model_profile_id.to_string(),
                    i64::try_from(report.key.model_profile_revision).unwrap_or(i64::MAX),
                    report.key.node_definition_id,
                    report.key.node_config_hash,
                    enum_string(report.status)?,
                    serde_json::to_string(report)?,
                    report.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_geometry_calibration(
        &self,
        id: GeometryCalibrationId,
    ) -> Result<Option<GeometryCalibrationReport>, StorageError> {
        self.with_connection(|connection| {
            let value = connection
                .query_row(
                    "SELECT report_json FROM geometry_calibration_reports WHERE id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
                .transpose()
        })
    }

    pub fn list_project_geometry_calibrations(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<GeometryCalibrationReport>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT report_json FROM geometry_calibration_reports
                 WHERE project_id = ?1 ORDER BY created_at DESC",
            )?;
            statement
                .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect()
        })
    }

    pub fn save_pipeline_improvement(
        &self,
        session: &PipelineImprovementSession,
    ) -> Result<(), StorageError> {
        session.validate().map_err(StorageError::InvalidEnum)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO pipeline_improvement_sessions
                 (id, project_id, baseline_workflow_id, baseline_workflow_version, status,
                  session_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   status = excluded.status,
                   session_json = excluded.session_json,
                   updated_at = excluded.updated_at",
                params![
                    session.id.to_string(),
                    session.project_id,
                    session.baseline_workflow_id,
                    i64::from(session.baseline_workflow_version),
                    enum_string(session.status)?,
                    serde_json::to_string(session)?,
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_pipeline_improvement(
        &self,
        id: PipelineImprovementId,
    ) -> Result<Option<PipelineImprovementSession>, StorageError> {
        self.with_connection(|connection| {
            let value = connection
                .query_row(
                    "SELECT session_json FROM pipeline_improvement_sessions WHERE id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            value
                .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
                .transpose()
        })
    }

    pub fn list_project_pipeline_improvements(
        &self,
        project_id: &str,
    ) -> Result<Vec<PipelineImprovementSession>, StorageError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT session_json FROM pipeline_improvement_sessions
                 WHERE project_id = ?1 ORDER BY updated_at DESC",
            )?;
            statement
                .query_map([project_id], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect()
        })
    }

    pub fn query_corrections(
        &self,
        project_id: ProjectId,
        skill_id: &str,
        task_id: &TaskId,
        label: Option<&LabelId>,
        limit: usize,
    ) -> Result<Vec<CorrectionRecord>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT record_json FROM correction_records
                 WHERE project_id = ?1 AND skill_id = ?2 AND task_id = ?3
                   AND (?4 IS NULL OR predicted_label = ?4 OR corrected_label = ?4)
                 ORDER BY created_at DESC LIMIT ?5",
            )?;
            let rows = statement
                .query_map(
                    params![
                        project_id.to_string(),
                        skill_id,
                        task_id.as_str(),
                        label.map(LabelId::as_str),
                        limit,
                    ],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub fn list_project_corrections(
        &self,
        project_id: ProjectId,
        limit: usize,
    ) -> Result<Vec<CorrectionRecord>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 500)).unwrap_or(500);
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT record_json FROM correction_records
                 WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?;
            let rows = statement
                .query_map(params![project_id.to_string(), limit], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
                .collect()
        })
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        operation(&connection)
    }
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn history_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRun> {
    let id = row.get::<_, String>(0)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let status_text = row.get::<_, String>(6)?;
    let status =
        serde_json::from_value(serde_json::Value::String(status_text)).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(HistoryRun {
        id,
        project_id: row
            .get::<_, Option<String>>(1)?
            .map(|value| value.parse())
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        project_name: row.get(2)?,
        skill_id: row.get(3)?,
        provider: row.get(4)?,
        model: row.get(5)?,
        status,
        project_schema_json: row.get(7)?,
        workflow_snapshot_json: row.get(8)?,
        terminal_reason: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn stored_project_image_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredProjectImage> {
    let image_id = row.get::<_, String>(0)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let project_id = row.get::<_, String>(1)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(StoredProjectImage {
        image_id,
        project_id,
        relative_path: row.get(2)?,
        sha256: row.get(3)?,
        metadata_json: row.get(4)?,
        imported_at: row.get(5)?,
    })
}

fn append_json_references(
    connection: &Connection,
    sql: &str,
    registry_id: &str,
    kind: &str,
    references: &mut Vec<RegistryReference>,
) -> Result<(), StorageError> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    references.extend(
        rows.into_iter()
            .filter(|(_, json)| json.contains(registry_id))
            .map(|(location, _)| RegistryReference {
                kind: kind.to_owned(),
                location,
            }),
    );
    Ok(())
}

type ProviderProbeUsageRow = (
    String,
    String,
    String,
    i64,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    i64,
    bool,
    String,
    String,
);

fn provider_probe_usage_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderProbeUsageRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
    ))
}

fn parse_provider_probe_usage(
    row: ProviderProbeUsageRow,
) -> Result<ProviderProbeUsage, StorageError> {
    let (
        id,
        provider_id,
        model_profile_id,
        model_profile_revision,
        request_id,
        input_tokens,
        output_tokens,
        total_tokens,
        cost,
        currency,
        duration_ms,
        succeeded,
        safe_message,
        created_at,
    ) = row;
    let parse_u64 = |name: &str, value: i64| {
        u64::try_from(value)
            .map_err(|_| StorageError::InvalidEnum(format!("invalid {name}: {value}")))
    };
    Ok(ProviderProbeUsage {
        id: id.parse().map_err(|error| {
            StorageError::InvalidEnum(format!("invalid probe usage id: {error}"))
        })?,
        provider_id: provider_id
            .parse()
            .map_err(|error| StorageError::InvalidEnum(format!("invalid provider id: {error}")))?,
        model_profile_id: model_profile_id.parse().map_err(|error| {
            StorageError::InvalidEnum(format!("invalid model profile id: {error}"))
        })?,
        model_profile_revision: parse_u64("model profile revision", model_profile_revision)?,
        request_id,
        input_tokens: input_tokens
            .map(|value| parse_u64("input tokens", value))
            .transpose()?,
        output_tokens: output_tokens
            .map(|value| parse_u64("output tokens", value))
            .transpose()?,
        total_tokens: total_tokens
            .map(|value| parse_u64("total tokens", value))
            .transpose()?,
        cost,
        currency,
        duration_ms: parse_u64("probe duration", duration_ms)?,
        succeeded,
        safe_message,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map_err(|error| {
                StorageError::InvalidEnum(format!("invalid probe timestamp: {error}"))
            })?
            .with_timezone(&Utc),
    })
}

fn query_json_rows<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    run_id: RunId,
) -> Result<Vec<T>, StorageError> {
    let mut statement = connection.prepare(sql)?;
    statement
        .query_map([run_id.to_string()], |row| row.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect()
}

fn enum_string<T: serde::Serialize>(value: T) -> Result<String, StorageError> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| StorageError::InvalidEnum("enum did not serialize as a string".to_owned()))
}

fn binding_match_value(binding: &ProjectModelBinding) -> Result<String, StorageError> {
    match binding.match_kind {
        ModelBindingMatch::Capability => enum_string(binding.capability),
        ModelBindingMatch::Role => enum_string(binding.role),
    }
}

fn parse_run_id(value: &str) -> Result<RunId, StorageError> {
    value
        .parse()
        .map_err(|error| StorageError::InvalidEnum(format!("invalid run id: {error}")))
}

fn parse_run_status(value: &str) -> Result<RunStatus, StorageError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(StorageError::from)
}

fn insert_history_run(
    transaction: &rusqlite::Transaction<'_>,
    run: &HistoryRun,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO runs
         (id, project_id, project_name, skill_id, provider, model, status, project_schema_json,
          workflow_snapshot_json, terminal_reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            run.id.to_string(),
            run.project_id.map(|id| id.to_string()),
            run.project_name,
            run.skill_id,
            run.provider,
            run.model,
            enum_string(run.status)?,
            run.project_schema_json,
            run.workflow_snapshot_json,
            run.terminal_reason,
            run.created_at,
            run.updated_at,
        ],
    )?;
    Ok(())
}

fn remap_copy_ids<T: Copy + Ord>(ids: &mut [T], replacements: &BTreeMap<T, T>) {
    for id in ids {
        if let Some(replacement) = replacements.get(id) {
            *id = *replacement;
        }
    }
}

fn remap_relations(
    relations: &mut [annotagent_core::RelationValue],
    annotation_ids: &BTreeMap<annotagent_core::AnnotationId, annotagent_core::AnnotationId>,
    artifact_ids: &BTreeMap<ArtifactId, ArtifactId>,
) {
    for relation in relations {
        for endpoint in [&mut relation.source, &mut relation.target] {
            match endpoint {
                RelationEndpoint::Annotation(id) => {
                    if let Some(replacement) = annotation_ids.get(id) {
                        *id = *replacement;
                    }
                }
                RelationEndpoint::Artifact(id) => {
                    if let Some(replacement) = artifact_ids.get(id) {
                        *id = *replacement;
                    }
                }
            }
        }
    }
}

fn remap_annotation_value(
    value: &mut AnnotationValue,
    annotation_ids: &BTreeMap<annotagent_core::AnnotationId, annotagent_core::AnnotationId>,
    artifact_ids: &BTreeMap<ArtifactId, ArtifactId>,
) {
    match value {
        AnnotationValue::Relation { source, target, .. } => {
            if let Some(replacement) = annotation_ids.get(source) {
                *source = *replacement;
            }
            if let Some(replacement) = annotation_ids.get(target) {
                *target = *replacement;
            }
        }
        AnnotationValue::Relations { relations } => {
            remap_relations(relations, annotation_ids, artifact_ids);
        }
        _ => {}
    }
}

fn remap_json_strings(value: &mut serde_json::Value, replacements: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::String(string) => {
            if let Some(replacement) = replacements.get(string) {
                string.clone_from(replacement);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remap_json_strings(value, replacements);
            }
        }
        serde_json::Value::Object(object) => {
            for value in object.values_mut() {
                remap_json_strings(value, replacements);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn sanitize_trace_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("authorization")
                        || lower.contains("api_key")
                        || lower.contains("secret")
                    {
                        (
                            key.clone(),
                            serde_json::Value::String("[REDACTED]".to_owned()),
                        )
                    } else if lower.contains("base64") || lower == "data_url" {
                        (
                            key.clone(),
                            serde_json::Value::String("[BINARY OMITTED]".to_owned()),
                        )
                    } else {
                        (key.clone(), sanitize_trace_value(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(sanitize_trace_value).collect())
        }
        _ => value.clone(),
    }
}

#[async_trait]
impl RuntimeStore for SqliteStore {
    async fn create_run(&self, run: &RunRecord) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let status = serde_json::to_value(run.status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("pending")
            .to_owned();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO runs
                 (id, project_id, project_name, skill_id, provider, model, status, project_schema_json, workflow_snapshot_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    run.id.to_string(),
                    run.project_id.to_string(),
                    run.project_name,
                    run.skill_id,
                    run.provider,
                    run.model,
                    status,
                    run.project_schema_json,
                    run.workflow_snapshot_json,
                    now
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_run_status(
        &self,
        run_id: RunId,
        status: RunStatus,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let status = serde_json::to_value(status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("failed")
            .to_owned();
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "UPDATE runs SET status = ?2, terminal_reason = ?3, updated_at = ?4 WHERE id = ?1",
                params![run_id.to_string(), status, reason, Utc::now().to_rfc3339()],
            )?;
            transaction.execute(
                "UPDATE run_images SET status = ?2 WHERE run_id = ?1",
                params![run_id.to_string(), status],
            )?;
            if matches!(
                status.as_str(),
                "pending" | "running" | "paused" | "awaiting_review"
            ) {
                transaction.execute(
                    "UPDATE active_project_runs SET status = ?2, updated_at = ?3 WHERE run_id = ?1",
                    params![run_id.to_string(), status, Utc::now().to_rfc3339()],
                )?;
            } else {
                transaction.execute(
                    "DELETE FROM active_project_runs WHERE run_id = ?1",
                    [run_id.to_string()],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_task_run_status(
        &self,
        run_id: RunId,
        image_id: ImageId,
        task_id: &TaskId,
        status: TaskRunStatus,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let status = serde_json::to_value(status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("failed")
            .to_owned();
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO run_images (run_id, image_id, status) VALUES (?1, ?2, ?3)
                 ON CONFLICT(run_id, image_id) DO UPDATE SET status = excluded.status",
                params![run_id.to_string(), image_id.to_string(), status],
            )?;
            transaction.execute(
                "INSERT INTO task_runs (run_id, image_id, task_id, status, reason, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(run_id, image_id, task_id) DO UPDATE SET
                   status = excluded.status,
                   reason = excluded.reason,
                   updated_at = excluded.updated_at",
                params![
                    run_id.to_string(),
                    image_id.to_string(),
                    task_id.as_str(),
                    status,
                    reason,
                    Utc::now().to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_event(&self, event: &RunEvent) -> Result<(), String> {
        let event_json = json(event)?;
        let kind = serde_json::to_value(event.kind)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO run_events (event_id, run_id, event_kind, event_json, occurred_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event.event_id.to_string(),
                    event.run_id.to_string(),
                    kind,
                    event_json,
                    event.occurred_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_usage(&self, run_id: RunId, usage: &UsageRecord) -> Result<(), String> {
        let usage_json = json(usage)?;
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO model_calls
                 (run_id, provider, model, endpoint_summary, request_id, success, retry_count,
                  started_at, completed_at, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run_id.to_string(),
                    usage.provider,
                    usage.model,
                    usage.endpoint_summary,
                    usage.request_id,
                    usage.success,
                    usage.retry_count,
                    usage.started_at.to_rfc3339(),
                    usage.completed_at.to_rfc3339(),
                    sqlite_u64(usage.duration_ms)
                ],
            )?;
            transaction.execute(
                "INSERT INTO usage_records
                 (run_id, usage_json, input_tokens, output_tokens, total_tokens, cost, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run_id.to_string(),
                    usage_json,
                    usage.tokens.input_tokens.map(sqlite_u64),
                    usage.tokens.output_tokens.map(sqlite_u64),
                    usage.tokens.total_tokens.map(sqlite_u64),
                    usage.cost.total.to_string(),
                    usage.completed_at.to_rfc3339()
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_model_message(
        &self,
        run_id: RunId,
        image_id: Option<ImageId>,
        task_id: Option<&TaskId>,
        message: &ModelMessage,
    ) -> Result<(), String> {
        let message_json = json(message)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO model_messages
                 (run_id, image_id, task_id, message_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    run_id.to_string(),
                    image_id.map(|id| id.to_string()),
                    task_id.map(TaskId::as_str),
                    message_json,
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_tool_call(
        &self,
        run_id: RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: &serde_json::Value,
        result: Option<&ToolResult>,
        error: Option<&str>,
    ) -> Result<(), String> {
        let arguments = json(arguments)?;
        let result = result.map(json).transpose()?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR REPLACE INTO tool_calls
                 (call_id, run_id, name, arguments_json, result_json, error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    call_id.as_str(),
                    run_id.to_string(),
                    name,
                    arguments,
                    result,
                    error,
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn record_artifact(
        &self,
        run_id: RunId,
        artifact: &VisionArtifact,
    ) -> Result<(), String> {
        artifact.validate().map_err(|error| error.to_string())?;
        let state = serde_json::to_value(artifact.validation_state)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("unvalidated")
            .to_owned();
        let artifact_json = json(artifact)?;
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR REPLACE INTO vision_artifacts
                 (artifact_id, run_id, image_id, task_id, source_node, validation_state,
                  artifact_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    artifact.id.to_string(),
                    run_id.to_string(),
                    artifact.image_id.to_string(),
                    artifact.task_id.as_ref().map(TaskId::as_str),
                    artifact.source_node,
                    state,
                    artifact_json,
                    artifact.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn set_artifact_validation_state(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
        state: ArtifactValidationState,
    ) -> Result<(), String> {
        self.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT artifact_json FROM vision_artifacts
                     WHERE artifact_id = ?1 AND run_id = ?2",
                    params![artifact_id.to_string(), run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StorageError::InvalidEnum(format!("artifact {artifact_id} was not found"))
                })?;
            let mut artifact: VisionArtifact = serde_json::from_str(&stored)?;
            artifact.validation_state = state;
            connection.execute(
                "UPDATE vision_artifacts SET validation_state = ?2, artifact_json = ?3
                 WHERE artifact_id = ?1 AND run_id = ?4",
                params![
                    artifact_id.to_string(),
                    enum_string(state)?,
                    serde_json::to_string(&artifact)?,
                    run_id.to_string()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn find_artifact(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
    ) -> Result<Option<VisionArtifact>, String> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT artifact_json FROM vision_artifacts
                     WHERE artifact_id = ?1 AND run_id = ?2",
                    params![artifact_id.to_string(), run_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
                .transpose()
        })
        .map_err(|error| error.to_string())
    }

    async fn record_validation(
        &self,
        run_id: RunId,
        issues: &[ValidationIssue],
    ) -> Result<(), String> {
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            for issue in issues {
                let severity = serde_json::to_value(issue.severity)?
                    .as_str()
                    .unwrap_or("warning")
                    .to_owned();
                transaction.execute(
                    "INSERT INTO validation_issues
                     (run_id, code, severity, issue_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        run_id.to_string(),
                        issue.code,
                        severity,
                        serde_json::to_string(issue)?,
                        Utc::now().to_rfc3339()
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn commit_annotation(
        &self,
        run_id: RunId,
        annotation: &Annotation,
    ) -> Result<(), String> {
        let annotation_json = json(annotation)?;
        let status = serde_json::to_value(annotation.review_status)
            .map_err(|error| error.to_string())?
            .as_str()
            .unwrap_or("draft")
            .to_owned();
        self.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO annotations
                 (id, run_id, image_id, task_id, label, review_status, annotation_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    annotation.id.to_string(),
                    run_id.to_string(),
                    annotation.image_id.to_string(),
                    annotation.task_id.as_str(),
                    annotation
                        .label
                        .as_ref()
                        .map(annotagent_core::LabelId::as_str),
                    status,
                    annotation_json,
                    annotation.created_at.to_rfc3339()
                ],
            )?;
            if annotation.review_status == annotagent_core::ReviewStatus::NeedsReview {
                transaction.execute(
                    "INSERT INTO review_queue
                     (run_id, annotation_id, status, reasons_json, created_at)
                     VALUES (?1, ?2, 'pending', '[]', ?3)",
                    params![
                        run_id.to_string(),
                        annotation.id.to_string(),
                        Utc::now().to_rfc3339()
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }

    async fn correction_risk(
        &self,
        project_id: ProjectId,
        skill_id: &str,
        task_id: &TaskId,
        label: Option<&LabelId>,
    ) -> Result<f32, String> {
        self.with_connection(|connection| {
            let count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM (
                    SELECT id FROM correction_records
                    WHERE project_id = ?1 AND skill_id = ?2 AND task_id = ?3
                      AND (?4 IS NULL OR predicted_label = ?4)
                    ORDER BY created_at DESC LIMIT 20
                 )",
                params![
                    project_id.to_string(),
                    skill_id,
                    task_id.as_str(),
                    label.map(LabelId::as_str)
                ],
                |row| row.get(0),
            )?;
            let bounded = u16::try_from(count.clamp(0, 20)).unwrap_or(20);
            Ok(f32::from(bounded) / 20.0)
        })
        .map_err(|error| error.to_string())
    }

    async fn record_revision(&self, revision: &AnnotationRevision) -> Result<(), String> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO annotation_revisions
                 (revision_id, annotation_id, parent_revision_id, revision_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    revision.revision_id.to_string(),
                    revision.annotation_id.to_string(),
                    revision.parent_revision_id.map(|id| id.to_string()),
                    serde_json::to_string(revision)?,
                    revision.created_at.to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())
    }
}

fn sqlite_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn mock_fixture_identity(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized == "mock" || normalized.starts_with("mock-") || normalized.starts_with("mock_")
}

fn value_contains_mock_fixture(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => mock_fixture_identity(value),
        serde_json::Value::Array(values) => values.iter().any(value_contains_mock_fixture),
        serde_json::Value::Object(values) => values.values().any(value_contains_mock_fixture),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use annotagent_core::{
        AnnotationId, AnnotationProvenance, AnnotationSource, ArtifactProvenance, ArtifactRole,
        AttributeValue, CapabilityDeclarationSource, CorrectionFeatures, CredentialReference,
        CredentialSource, DETECTION_ARTIFACT_SCHEMA_VERSION, GenerationDefaults,
        GeometryCalibrationKey, GeometryCalibrationThresholds, GeometryCorrectionInput,
        GeometryCorrectionReason, GeometrySnapshot, InputModality, IssueSeverity, ModelBindingRole,
        ModelCapability, ModelLimits, ModelPricing, ModelProfileStatus, NodeId, NormalizedRect,
        PipelineArtifact, PricingSource, ProjectGeometryPolicy, ProtocolFeatures,
        ProviderAdapterKind, ProviderConnectionPolicy, ProviderHealthSnapshot,
        ProviderHealthStatus, RunEventKind, RunEventPayload, ScoreSemantics, SuggestedAction,
        ValidationEvidence, VisionArtifactValue, WorkflowDraftNode, WorkflowNodeKind,
    };

    use super::*;

    #[test]
    fn migration_creates_required_tables() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let tables = store.schema_tables().expect("schema tables");
        for required in [
            "schema_migrations",
            "projects",
            "project_snapshots",
            "images",
            "tasks",
            "annotations",
            "annotation_revisions",
            "runs",
            "active_project_runs",
            "run_start_requests",
            "run_images",
            "task_runs",
            "run_steps",
            "run_events",
            "model_calls",
            "model_messages",
            "tool_calls",
            "vision_artifacts",
            "validation_issues",
            "usage_records",
            "correction_records",
            "review_queue",
            "settings_metadata",
            "workflow_drafts",
            "workflow_versions",
            "dataset_batches",
            "batch_images",
            "batch_events",
            "agent_sessions",
            "workflow_sample_tests",
            "provider_profiles",
            "model_profiles",
            "project_model_bindings",
            "global_model_defaults",
            "legacy_registry_imports",
            "geometry_quality_reports",
            "geometry_correction_evidence",
            "project_geometry_policies",
            "geometry_calibration_reports",
            "pipeline_improvement_sessions",
            "plugins",
            "plugin_versions",
            "plugin_installations",
            "plugin_permissions",
            "plugin_models",
            "plugin_weight_sets",
            "plugin_health_checks",
            "plugin_test_runs",
            "plugin_references",
            "plugin_license_acceptances",
            "plugin_events",
            "model_catalogs",
            "model_catalog_entries",
            "model_bundles",
            "model_bundle_files",
            "model_bundle_contracts",
            "model_bundle_installations",
            "model_bundle_verifications",
            "model_bundle_smoke_tests",
            "model_bundle_license_acceptances",
            "model_instances",
            "model_instance_health",
            "model_bundle_references",
            "model_bundle_events",
        ] {
            assert!(
                tables.iter().any(|table| table == required),
                "missing {required}"
            );
        }
    }

    #[test]
    fn project_images_keep_stable_ids_without_collapsing_duplicate_content() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        let first = store
            .ensure_project_image(project_id, "images/a.png", "same-hash", "{}")
            .expect("first image");
        let duplicate_content = store
            .ensure_project_image(project_id, "images/b.png", "same-hash", "{}")
            .expect("second path");
        let changed = store
            .ensure_project_image(project_id, "images/a.png", "new-hash", "{\"revision\":2}")
            .expect("changed content at stable path");

        assert_ne!(first.image_id, duplicate_content.image_id);
        assert_eq!(first.image_id, changed.image_id);
        assert_eq!(changed.sha256, "new-hash");
        assert_eq!(
            store
                .get_project_image(project_id, first.image_id)
                .expect("lookup")
                .expect("stored image"),
            changed
        );
    }

    #[test]
    fn workspace_identity_migration_preserves_every_legacy_image_row() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("history.db");
        let connection = Connection::open(&path).expect("legacy database");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("legacy schema");
        let project_id = ProjectId::new();
        for sha256 in ["old-hash", "new-hash"] {
            connection
                .execute(
                    "INSERT INTO images
                     (id, project_id, relative_path, sha256, metadata_json, imported_at)
                     VALUES (?1, ?2, 'images/replaced.png', ?3, '{}', ?4)",
                    params![
                        ImageId::new().to_string(),
                        project_id.to_string(),
                        sha256,
                        Utc::now().to_rfc3339(),
                    ],
                )
                .expect("legacy image row");
        }
        drop(connection);

        let store = SqliteStore::open(&path).expect("migrated database");
        let count = store
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT COUNT(*) FROM images WHERE project_id = ?1",
                    [project_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .expect("preserved row count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn run_image_ownership_is_registered_independently_of_annotations() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let run_id = RunId::new();
        let project_id = ProjectId::new();
        let image_id = ImageId::new();
        store
            .create_run(&RunRecord {
                id: run_id,
                project_id,
                project_name: "duplicate display name".to_owned(),
                skill_id: "none".to_owned(),
                provider: "core".to_owned(),
                model: "none".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: "{}".to_owned(),
                workflow_snapshot_json: None,
            })
            .await
            .expect("create run");
        store
            .set_task_run_status(
                run_id,
                image_id,
                &TaskId::from("objects"),
                TaskRunStatus::Pending,
                None,
            )
            .await
            .expect("register task image");

        assert_eq!(
            store.run_image_ids(run_id).expect("owned images"),
            BTreeSet::from([image_id])
        );
        store
            .set_run_status(run_id, RunStatus::CompletedWithReview, None)
            .await
            .expect("update Run and image status");
        assert_eq!(
            store.run_images(run_id).expect("Run image summary"),
            vec![(image_id, "completed_with_review".to_owned())]
        );
        assert_eq!(
            store
                .latest_project_image_run_statuses(project_id)
                .expect("latest Project image statuses"),
            BTreeMap::from([(image_id, "completed_with_review".to_owned())])
        );
        assert_eq!(
            store
                .backfill_legacy_run_project_id(ProjectId::new(), "duplicate display name")
                .expect("backfill"),
            0
        );
    }

    #[test]
    fn pipeline_improvement_session_round_trips_and_lists_by_project() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let now = Utc::now();
        let run_id = RunId::new();
        let session = PipelineImprovementSession {
            schema_version: annotagent_core::PIPELINE_IMPROVEMENT_SCHEMA_VERSION,
            id: PipelineImprovementId::new(),
            project_id: "geometry-project".to_owned(),
            baseline_workflow_id: "geometry-workflow".to_owned(),
            baseline_workflow_version: 1,
            target_task_id: TaskId::from("objects"),
            target_label: LabelId::from("ball"),
            diagnosis: annotagent_core::PipelineImprovementDiagnosis {
                primary_failure_class: annotagent_core::AnnotationFailureClass::GeometryError,
                evidence_run_ids: vec![run_id],
                evidence_statements: vec!["one human bbox correction".to_owned()],
                semantic_target_correct_count: 1,
                geometry_correction_count: 1,
                provider_failure_count: 0,
                no_candidate_count: 0,
            },
            evaluation_run_ids: Vec::new(),
            baseline_draft_id: "baseline-draft".to_owned(),
            candidate_draft_id: "candidate-draft".to_owned(),
            diff: annotagent_core::PipelineDraftDiff::default(),
            validation: annotagent_core::WorkflowValidationReport {
                valid: true,
                issues: Vec::new(),
                execution_order: Vec::new(),
            },
            comparison: None,
            status: annotagent_core::PipelineImprovementStatus::DraftCreated,
            setup_requirements: Vec::new(),
            applied_draft_id: None,
            created_at: now,
            updated_at: now,
        };
        store
            .save_pipeline_improvement(&session)
            .expect("save improvement");
        assert_eq!(
            store
                .get_pipeline_improvement(session.id)
                .expect("load improvement"),
            Some(session.clone())
        );
        assert_eq!(
            store
                .list_project_pipeline_improvements("geometry-project")
                .expect("list improvements"),
            vec![session]
        );
    }

    #[test]
    fn legacy_registry_import_is_atomic_idempotent_and_preserves_existing_bindings() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let now = Utc::now();
        let provider_id = ProviderId::new();
        let model_id = ModelProfileId::new();
        let project_id = ProjectId::new();
        let provider = ProviderProfile {
            id: provider_id,
            display_name: "Imported legacy mock".to_owned(),
            preset_id: Some("legacy".to_owned()),
            adapter: ProviderAdapterKind::Mock,
            base_url: url::Url::parse("http://127.0.0.1:8787/v1").expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("Imported from compatibility settings.".to_owned()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let model = ModelProfile {
            id: model_id,
            revision: 1,
            provider_id,
            display_name: "default-vision".to_owned(),
            remote_model_id: "legacy-vision".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::VisionLanguage]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        let binding = ProjectModelBinding {
            id: ModelBindingId::new(),
            project_id,
            capability: ModelCapability::VisionLanguage,
            role: ModelBindingRole::PrimaryInference,
            match_kind: ModelBindingMatch::Role,
            model_profile_id: model_id,
            locked: true,
            created_at: now,
        };
        let import = LegacyRegistryImport {
            fingerprint: "legacy-fixture-v1".to_owned(),
            provider: provider.clone(),
            model: model.clone(),
            project_bindings: vec![binding.clone()],
        };
        let first = store
            .apply_legacy_registry_import(&import)
            .expect("first import");
        assert!(first.provider_created);
        assert!(first.model_created);
        assert_eq!(first.bindings_created, 1);
        assert!(!first.already_applied);

        let repeated = store
            .apply_legacy_registry_import(&import)
            .expect("repeated import");
        assert!(repeated.already_applied);
        assert_eq!(store.list_provider_profiles().expect("providers").len(), 1);
        assert_eq!(
            store.list_model_profiles(None, true).expect("models").len(),
            1
        );
        assert_eq!(
            store
                .list_project_model_bindings(project_id)
                .expect("bindings"),
            vec![binding]
        );

        let mut colliding = import;
        colliding.fingerprint = "legacy-fixture-collision".to_owned();
        colliding.model.remote_model_id = "different-semantics".to_owned();
        assert!(matches!(
            store.apply_legacy_registry_import(&colliding),
            Err(StorageError::InvalidModelRevision(_))
        ));
        store
            .with_connection(|connection| {
                let marker_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM legacy_registry_imports",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(marker_count, 1, "failed collision rolled back");
                Ok(())
            })
            .expect("inspect markers");
    }

    #[test]
    fn provider_profiles_persist_references_but_never_secret_values() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let id = ProviderId::new();
        let now = Utc::now();
        let profile = ProviderProfile {
            id,
            display_name: "Qwen Lab".to_owned(),
            preset_id: Some("dashscope".to_owned()),
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: url::Url::parse("https://dashscope.aliyuncs.com/compatible-mode/v1")
                .expect("URL"),
            organization: None,
            workspace: Some("lab".to_owned()),
            credential_ref: Some(CredentialReference {
                provider_id: id,
                source: CredentialSource::SystemKeyring,
                locator: "provider-account".to_owned(),
            }),
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Configured,
                safe_message: None,
                checked_at: None,
            },
            created_at: now,
            updated_at: now,
        };
        store.save_provider_profile(&profile).expect("save");
        assert_eq!(store.get_provider_profile(id).expect("get"), profile);
        assert_eq!(store.list_provider_profiles().expect("list"), vec![profile]);
        store
            .with_connection(|connection| {
                let stored: String = connection.query_row(
                    "SELECT profile_json FROM provider_profiles WHERE id = ?1",
                    [id.to_string()],
                    |row| row.get(0),
                )?;
                assert!(stored.contains("provider-account"));
                assert!(!stored.contains("test-secret-must-not-be-in-sqlite"));
                Ok(())
            })
            .expect("inspect");
        store.delete_provider_profile(id).expect("delete");
        assert!(matches!(
            store.get_provider_profile(id),
            Err(StorageError::ProviderNotFound(missing)) if missing == id
        ));
    }

    #[test]
    fn purging_mock_registry_removes_active_bindings_defaults_and_fixture_drafts() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let now = Utc::now();
        let provider_id = ProviderId::new();
        let model_id = ModelProfileId::new();
        store
            .save_provider_profile(&ProviderProfile {
                id: provider_id,
                display_name: "Test fixture".to_owned(),
                preset_id: Some("mock".to_owned()),
                adapter: ProviderAdapterKind::Mock,
                base_url: url::Url::parse("http://127.0.0.1").expect("URL"),
                organization: None,
                workspace: None,
                credential_ref: None,
                safe_headers: BTreeMap::new(),
                connection_policy: ProviderConnectionPolicy::default(),
                enabled: true,
                health: ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Available,
                    safe_message: None,
                    checked_at: Some(now),
                },
                created_at: now,
                updated_at: now,
            })
            .expect("provider");
        store
            .save_model_profile(&ModelProfile {
                id: model_id,
                revision: 1,
                provider_id,
                display_name: "Fixture detector".to_owned(),
                remote_model_id: "mock-detector".to_owned(),
                input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
                protocol_features: ProtocolFeatures::default(),
                task_capabilities: BTreeSet::from([
                    ModelCapability::ObjectDetection,
                    ModelCapability::VisionLanguage,
                ]),
                capability_source: CapabilityDeclarationSource::Preset,
                limits: ModelLimits::default(),
                generation_defaults: GenerationDefaults::default(),
                pricing: ModelPricing::default(),
                quality_contracts: Vec::new(),
                status: ModelProfileStatus::Available,
                enabled: true,
                locked: true,
                created_at: now,
                updated_at: now,
            })
            .expect("model");
        let project_id = ProjectId::new();
        store
            .save_project_model_binding(
                &ProjectModelBinding {
                    id: ModelBindingId::new(),
                    project_id,
                    capability: ModelCapability::ObjectDetection,
                    role: ModelBindingRole::Detection,
                    match_kind: ModelBindingMatch::Role,
                    model_profile_id: model_id,
                    locked: true,
                    created_at: now,
                },
                BindingMutationActor::User,
            )
            .expect("binding");
        store
            .save_global_model_defaults(&GlobalModelDefaults {
                vision_language: Some(model_id),
                text_generation: None,
                pipeline_builder: None,
            })
            .expect("defaults");
        let fixture_draft = WorkflowDraft {
            schema_version: annotagent_core::WORKFLOW_SCHEMA_VERSION,
            id: "fixture-draft".to_owned(),
            project_id: "project".to_owned(),
            name: "Fixture Draft".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![WorkflowDraftNode {
                id: "detector".to_owned(),
                node_type: "capability.detect".to_owned(),
                kind: WorkflowNodeKind::VisionModel,
                model_binding: Some("mock-detector".to_owned()),
                ..WorkflowDraftNode::default()
            }],
            edges: Vec::new(),
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            geometry_risk_acceptance: None,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        store
            .save_workflow_draft(&fixture_draft)
            .expect("fixture Draft");
        let mut fixture_session = AgentSession::start(
            annotagent_core::AgentKind::PipelineBuilder,
            annotagent_core::AgentBudget::default(),
        )
        .with_project("project");
        fixture_session
            .record_tool(
                "list_compatible_models",
                serde_json::json!({}),
                serde_json::json!({
                    "models": [{"remote_model_id": "mock-detector"}]
                }),
                true,
            )
            .expect("fixture tool");
        store
            .save_agent_session(&fixture_session)
            .expect("fixture Agent Session");
        let mut real_session = AgentSession::start(
            annotagent_core::AgentKind::PipelineBuilder,
            annotagent_core::AgentBudget::default(),
        )
        .with_project("project");
        real_session
            .record_tool(
                "inspect_project",
                serde_json::json!({}),
                serde_json::json!({"warning": "Mock models are disabled in product mode."}),
                true,
            )
            .expect("real tool");
        store
            .save_agent_session(&real_session)
            .expect("real Agent Session");

        let removed = store
            .purge_provider_adapter(ProviderAdapterKind::Mock)
            .expect("purge");
        assert_eq!(removed, (1, 1, 1));
        assert_eq!(store.purge_mock_agent_sessions().expect("sessions"), 1);
        assert!(
            store
                .list_provider_profiles()
                .expect("providers")
                .is_empty()
        );
        assert!(
            store
                .list_model_profiles(None, false)
                .expect("models")
                .is_empty()
        );
        assert!(
            store
                .list_project_model_bindings(project_id)
                .expect("bindings")
                .is_empty()
        );
        assert!(store.list_workflow_drafts(None).expect("Drafts").is_empty());
        assert_eq!(
            store.get_global_model_defaults().expect("defaults"),
            GlobalModelDefaults::default()
        );
        assert_eq!(
            store
                .list_agent_sessions(Some("project"))
                .expect("Agent Sessions"),
            vec![real_session]
        );
    }

    #[test]
    fn model_revisions_bindings_and_agent_lock_are_persistent_and_fail_closed() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let provider_id = ProviderId::new();
        let now = Utc::now();
        store
            .save_provider_profile(&ProviderProfile {
                id: provider_id,
                display_name: "Mock Lab".to_owned(),
                preset_id: Some("mock".to_owned()),
                adapter: ProviderAdapterKind::Mock,
                base_url: url::Url::parse("http://127.0.0.1:8791/v1").expect("URL"),
                organization: None,
                workspace: None,
                credential_ref: None,
                safe_headers: BTreeMap::new(),
                connection_policy: ProviderConnectionPolicy::default(),
                enabled: true,
                health: ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Available,
                    safe_message: None,
                    checked_at: Some(now),
                },
                created_at: now,
                updated_at: now,
            })
            .expect("provider");
        let model_id = ModelProfileId::new();
        let profile = ModelProfile {
            id: model_id,
            revision: 1,
            provider_id,
            display_name: "Vision Builder".to_owned(),
            remote_model_id: "vision-builder-v1".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                json_schema: true,
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([
                ModelCapability::TextGeneration,
                ModelCapability::VisionLanguage,
                ModelCapability::ImageClassification,
            ]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits {
                context_tokens: Some(32_768),
                maximum_output_tokens: Some(4_096),
                maximum_images_per_request: Some(4),
                maximum_image_pixels: Some(12_000_000),
            },
            generation_defaults: GenerationDefaults {
                temperature: Some(rust_decimal::Decimal::new(1, 1)),
                ..GenerationDefaults::default()
            },
            pricing: ModelPricing {
                currency: "USD".to_owned(),
                per_request: Some(rust_decimal::Decimal::new(1, 3)),
                source: PricingSource::UserConfigured,
                updated_at: Some(now),
                ..ModelPricing::default()
            },
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        store.save_model_profile(&profile).expect("revision 1");

        let mut price_update = profile.clone();
        price_update.pricing.per_request = Some(rust_decimal::Decimal::new(2, 3));
        store
            .save_model_profile(&price_update)
            .expect("price update keeps revision");
        let mut invalid_same_revision = price_update.clone();
        invalid_same_revision.remote_model_id = "vision-builder-v2".to_owned();
        assert!(matches!(
            store.save_model_profile(&invalid_same_revision),
            Err(StorageError::ModelSemanticChangeRequiresRevision)
        ));
        invalid_same_revision.revision = 2;
        store
            .save_model_profile(&invalid_same_revision)
            .expect("semantic revision 2");
        let mut unnecessary_revision = invalid_same_revision.clone();
        unnecessary_revision.revision = 3;
        assert!(matches!(
            store.save_model_profile(&unnecessary_revision),
            Err(StorageError::ModelRevisionRequiresSemanticChange)
        ));
        assert_eq!(
            store
                .get_model_profile(model_id, None)
                .expect("latest")
                .revision,
            2
        );
        assert_eq!(
            store
                .list_model_profiles(Some(provider_id), true)
                .expect("history")
                .len(),
            2
        );

        let project_id = ProjectId::new();
        let binding = ProjectModelBinding {
            id: ModelBindingId::new(),
            project_id,
            capability: ModelCapability::VisionLanguage,
            role: ModelBindingRole::PipelineBuilder,
            match_kind: ModelBindingMatch::Role,
            model_profile_id: model_id,
            locked: true,
            created_at: now,
        };
        store
            .save_project_model_binding(&binding, BindingMutationActor::User)
            .expect("binding");
        let mut agent_replacement = binding.clone();
        agent_replacement.model_profile_id = ModelProfileId::new();
        assert!(matches!(
            store.save_project_model_binding(&agent_replacement, BindingMutationActor::Agent),
            Err(StorageError::ModelBindingLocked)
        ));
        assert!(matches!(
            store.delete_project_model_binding(binding.id, BindingMutationActor::Agent),
            Err(StorageError::ModelBindingLocked)
        ));
        assert_eq!(
            store
                .list_project_model_bindings(project_id)
                .expect("bindings"),
            vec![binding]
        );

        let defaults = GlobalModelDefaults {
            pipeline_builder: Some(model_id),
            vision_language: Some(model_id),
            text_generation: None,
        };
        store
            .save_global_model_defaults(&defaults)
            .expect("defaults");
        assert_eq!(
            store.get_global_model_defaults().expect("defaults"),
            defaults
        );
    }

    #[test]
    fn workflow_sample_test_is_persisted_per_draft() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let completed_at = Utc::now();
        let sample_test = WorkflowSampleTest {
            draft_id: "draft-1".to_owned(),
            project_id: "project-1".to_owned(),
            report: WorkflowDryRunReport {
                sandbox: true,
                validation: annotagent_core::WorkflowValidationReport {
                    valid: true,
                    issues: Vec::new(),
                    execution_order: vec!["image".to_owned()],
                },
                samples: Vec::new(),
                summary: annotagent_core::WorkflowDryRunSummary {
                    image_count: 3,
                    ..annotagent_core::WorkflowDryRunSummary::default()
                },
                total_latency_ms: 12,
                estimated_cost: "0".to_owned(),
            },
            completed_at,
        };
        store
            .save_workflow_sample_test(&sample_test)
            .expect("save sample test");
        let restored = store
            .get_workflow_sample_test("draft-1")
            .expect("load sample test")
            .expect("sample test exists");
        assert_eq!(restored, sample_test);
    }

    #[test]
    fn pending_review_count_uses_the_status_column() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        store
            .with_connection(|connection| {
                for (id, status) in [
                    ("review-a", "needs_review"),
                    ("review-b", "needs_review"),
                    ("accepted", "accepted"),
                ] {
                    connection.execute(
                        "INSERT INTO annotations
                         (id, run_id, image_id, task_id, label, review_status, annotation_json, created_at)
                         VALUES (?1, 'run', 'image', 'objects', 'ball', ?2, '{}', '2026-01-01T00:00:00Z')",
                        params![id, status],
                    )?;
                }
                Ok(())
            })
            .expect("fixture annotations");

        assert_eq!(store.pending_review_count().expect("pending reviews"), 2);
    }

    #[test]
    fn project_persists_multiple_workflow_drafts_and_published_versions() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let now = Utc::now();
        let make_draft = |id: &str| WorkflowDraft {
            schema_version: 2,
            id: id.to_owned(),
            project_id: "multi-workflow-project".to_owned(),
            name: id.to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![WorkflowDraftNode {
                id: "commit".to_owned(),
                node_type: "commit".to_owned(),
                kind: WorkflowNodeKind::Commit,
                ..WorkflowDraftNode::default()
            }],
            edges: Vec::new(),
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: true,
            geometry_risk_acceptance: None,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        for id in ["workflow-a", "workflow-b"] {
            let draft = make_draft(id);
            store.save_workflow_draft(&draft).expect("draft");
            let snapshot = WorkflowSnapshot {
                schema_version: 2,
                draft: Some(draft.clone()),
                ..WorkflowSnapshot::default()
            };
            store
                .publish_workflow_draft(&draft, format!("hash-{id}"), snapshot)
                .expect("published version");
        }
        assert_eq!(
            store
                .list_workflow_drafts(Some("multi-workflow-project"))
                .expect("drafts")
                .len(),
            2
        );
        let versions = store
            .list_published_workflow_versions(Some("multi-workflow-project"))
            .expect("versions");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].workflow_id, "workflow-a");
        assert_eq!(versions[1].workflow_id, "workflow-b");
    }

    #[test]
    fn legacy_terminal_awaiting_review_is_migrated_once() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE runs (
                    id TEXT PRIMARY KEY,
                    project_id TEXT,
                    project_name TEXT NOT NULL,
                    skill_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    status TEXT NOT NULL,
                    project_schema_json TEXT NOT NULL,
                    terminal_reason TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE active_project_runs (
                    project_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL UNIQUE,
                    status TEXT NOT NULL,
                    idempotency_key TEXT,
                    updated_at TEXT NOT NULL
                );
                INSERT INTO runs VALUES (
                    'legacy', NULL, 'legacy', 'legacy', 'mock', 'mock',
                    'awaiting_review', '{}', NULL, '2026-01-01T00:00:00Z',
                    '2026-01-01T00:00:00Z'
                );
                INSERT INTO active_project_runs VALUES (
                    'legacy-project', 'legacy', 'awaiting_review', NULL,
                    '2026-01-01T00:00:00Z'
                );",
            )
            .expect("legacy schema");
        let store = SqliteStore {
            connection: Mutex::new(connection),
        };
        store.migrate().expect("upgrade legacy schema");
        let migrated = store
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT status FROM runs WHERE id = 'legacy'",
                    [],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .expect("migrated status");
        assert_eq!(migrated, "completed_with_review");
        let workflow_snapshot_column = store
            .with_connection(|connection| {
                let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
                let columns = statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(columns
                    .into_iter()
                    .any(|name| name == "workflow_snapshot_json"))
            })
            .expect("run columns");
        assert!(workflow_snapshot_column);
        let active_count =
            store
                .with_connection(|connection| {
                    Ok(connection.query_row(
                        "SELECT COUNT(*) FROM active_project_runs",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?)
                })
                .expect("active rows");
        assert_eq!(active_count, 0);

        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO runs
                     (id, project_id, project_name, skill_id, provider, model, status,
                      project_schema_json, workflow_snapshot_json, terminal_reason, created_at, updated_at)
                     VALUES (
                        'suspended', NULL, 'new', 'generic', 'mock', 'mock',
                        'awaiting_review', '{}', NULL, NULL, '2026-01-02T00:00:00Z',
                        '2026-01-02T00:00:00Z'
                    )",
                    [],
                )?;
                Ok(())
            })
            .expect("new suspended run");
        store.migrate().expect("idempotent migration");
        let suspended = store
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT status FROM runs WHERE id = 'suspended'",
                    [],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .expect("suspended status");
        assert_eq!(suspended, "awaiting_review");
    }

    #[tokio::test]
    async fn event_history_round_trips() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let run_id = RunId::new();
        let frozen_workflow = r#"{"workflow_id":"workflow-a","version":1,"name":"before edit"}"#;
        store
            .create_run(&RunRecord {
                id: run_id,
                project_id: ProjectId::new(),
                project_name: "test".to_owned(),
                skill_id: "dummy".to_owned(),
                provider: "mock".to_owned(),
                model: "mock".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: "{}".to_owned(),
                workflow_snapshot_json: Some(frozen_workflow.to_owned()),
            })
            .await
            .expect("create run");
        let event = RunEvent::new(
            run_id,
            RunEventKind::RunCreated,
            RunEventPayload::State {
                from: None,
                to: RunStatus::Pending,
                reason: None,
            },
        );
        store.record_event(&event).await.expect("record event");
        assert_eq!(store.list_events(run_id).expect("events"), vec![event]);
        let issue = ValidationIssue {
            code: "skill_specific_risk".to_owned(),
            severity: IssueSeverity::Warning,
            annotation_ids: Vec::new(),
            message: "The enabled Skill found a domain-specific risk.".to_owned(),
            suggested_action: SuggestedAction::HumanReview,
            evidence: ValidationEvidence::Rule {
                facts: BTreeMap::new(),
            },
        };
        store
            .record_validation(run_id, std::slice::from_ref(&issue))
            .await
            .expect("record validation issue");
        assert_eq!(
            store
                .list_validation_issues(run_id)
                .expect("validation issues"),
            vec![issue]
        );
        let history = store.history(run_id).expect("history");
        assert_eq!(
            history.run.workflow_snapshot_json.as_deref(),
            Some(frozen_workflow)
        );

        let artifact = VisionArtifact {
            id: ArtifactId::new(),
            image_id: ImageId::new(),
            task_id: Some(TaskId::from("attributes")),
            label: None,
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::Attributes {
                values: BTreeMap::from([("verified".to_owned(), AttributeValue::Boolean(true))]),
            },
            source_node: "generic.attributes".to_owned(),
            confidence: Some(0.9),
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance::default(),
            revision: 1,
            replaces_artifact_id: None,
            created_at: Utc::now(),
        };
        store
            .record_artifact(run_id, &artifact)
            .await
            .expect("record artifact");
        assert_eq!(
            store.list_artifacts(run_id).expect("artifacts"),
            vec![artifact]
        );
    }

    #[tokio::test]
    async fn prior_run_annotations_are_scoped_to_the_exact_project_and_run() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_a = ProjectId::new();
        let project_b = ProjectId::new();
        let run_a = RunId::new();
        let run_b = RunId::new();
        for (run_id, project_id, project_name) in [
            (run_a, project_a, "project-a"),
            (run_b, project_b, "project-b"),
        ] {
            store
                .create_run(&RunRecord {
                    id: run_id,
                    project_id,
                    project_name: project_name.to_owned(),
                    skill_id: "none".to_owned(),
                    provider: "core".to_owned(),
                    model: "vision-model".to_owned(),
                    status: RunStatus::Completed,
                    project_schema_json: "{}".to_owned(),
                    workflow_snapshot_json: None,
                })
                .await
                .expect("create run");
        }
        let annotation = Annotation {
            id: AnnotationId::new(),
            image_id: ImageId::new(),
            task_id: TaskId::from("objects"),
            label: Some(LabelId::from("dog")),
            value: AnnotationValue::BoundingBox {
                rect: NormalizedRect::new(0.1, 0.2, 0.3, 0.4).expect("normalized bbox"),
            },
            attributes: BTreeMap::new(),
            confidence: Some(0.8),
            source: AnnotationSource::Human,
            review_status: ReviewStatus::HumanAccepted,
            provenance: AnnotationProvenance::default(),
            created_at: Utc::now(),
        };
        store
            .commit_annotation(run_a, &annotation)
            .await
            .expect("commit source annotation");

        assert_eq!(
            store
                .list_project_annotations_for_run(project_a, run_a)
                .expect("same Project and Run"),
            vec![annotation]
        );
        assert!(
            store
                .list_project_annotations_for_run(project_b, run_a)
                .expect("different Project")
                .is_empty()
        );
        assert!(
            store
                .list_project_annotations_for_run(project_a, run_b)
                .expect("different Run")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn persisted_legacy_detection_artifact_migrates_on_read() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let run_id = RunId::new();
        let snapshot = serde_json::json!({
            "checkpoint": {
                "pipeline_artifact": {
                    "kind": "detection_set",
                    "artifact": {
                        "reference": {
                            "artifact_id": "legacy-set",
                            "source_node": "legacy-detector",
                            "port": "detections",
                            "artifact_type": "detection_set",
                            "item_id": null
                        },
                        "image_id": ImageId::new(),
                        "model_binding": "legacy-model",
                        "detections": [{
                            "id": "legacy-detection",
                            "class_id": "football",
                            "rect": [0.1, 0.2, 0.3, 0.4],
                            "confidence": 0.75
                        }]
                    }
                }
            }
        });
        store
            .create_run(&RunRecord {
                id: run_id,
                project_id: ProjectId::new(),
                project_name: "migration".to_owned(),
                skill_id: "generic".to_owned(),
                provider: "mock".to_owned(),
                model: "legacy-model".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: "{}".to_owned(),
                workflow_snapshot_json: Some(snapshot.to_string()),
            })
            .await
            .expect("persist legacy snapshot");

        let stored = store.history(run_id).expect("stored history");
        let stored_snapshot: serde_json::Value = serde_json::from_str(
            stored
                .run
                .workflow_snapshot_json
                .as_deref()
                .expect("workflow snapshot"),
        )
        .expect("snapshot JSON");
        let artifact: PipelineArtifact =
            serde_json::from_value(stored_snapshot["checkpoint"]["pipeline_artifact"].clone())
                .expect("migrated Pipeline Artifact");
        let PipelineArtifact::DetectionSet(set) = artifact else {
            panic!("DetectionSet")
        };
        set.validate().expect("migrated DetectionSet validates");
        assert_eq!(set.schema_version, DETECTION_ARTIFACT_SCHEMA_VERSION);
        assert_eq!(set.detections[0].source_model_id, "legacy-model");
        assert_eq!(set.detections[0].score.semantics, ScoreSemantics::Unknown);
        assert_eq!(set.detections[0].evidence.len(), 1);
    }

    #[test]
    fn project_run_reservation_is_transactional_and_idempotent() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        let run_id = RunId::new();
        assert_eq!(
            store
                .reserve_project_run(project_id, run_id, Some("request-1"))
                .expect("reservation"),
            RunStartReservation::Reserved
        );
        assert_eq!(
            store
                .reserve_project_run(project_id, RunId::new(), Some("request-1"))
                .expect("idempotent replay"),
            RunStartReservation::Idempotent {
                run_id,
                status: RunStatus::Pending,
            }
        );
        assert_eq!(
            store
                .reserve_project_run(project_id, RunId::new(), Some("request-2"))
                .expect("conflict"),
            RunStartReservation::Conflict {
                run_id,
                status: RunStatus::Pending,
            }
        );
    }

    #[test]
    fn correction_memory_isolated_by_project_skill_task_and_label() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_a = ProjectId::new();
        let project_b = ProjectId::new();
        let record = |project_id, skill_id: &str, task_id: &str, label: &str| CorrectionRecord {
            id: Uuid::new_v4(),
            project_id,
            skill_id: skill_id.to_owned(),
            task_id: TaskId::from(task_id),
            predicted_label: Some(LabelId::from(label)),
            corrected_label: None,
            reason_code: "fixture".to_owned(),
            original_annotation: None,
            corrected_annotation: None,
            note: None,
            image_features: CorrectionFeatures {
                geometry: BTreeMap::new(),
                colors: BTreeMap::new(),
            },
            created_at: Utc::now(),
        };
        for item in [
            record(project_a, "domain.target", "objects", "target"),
            record(project_b, "domain.target", "objects", "target"),
            record(project_a, "domain.other", "objects", "target"),
            record(project_a, "domain.target", "attributes", "target"),
            record(project_a, "domain.target", "objects", "other"),
        ] {
            store.save_correction(&item).expect("correction");
        }
        let matches = store
            .query_corrections(
                project_a,
                "domain.target",
                &TaskId::from("objects"),
                Some(&LabelId::from("target")),
                20,
            )
            .expect("isolated query");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].project_id, project_a);
    }

    #[test]
    fn structured_geometry_correction_round_trips_with_model_revision_and_metrics() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        let run_id = RunId::new();
        let image_id = ImageId::new();
        let model_id = ModelProfileId::new();
        let (report, evidence) =
            annotagent_core::build_geometry_correction_evidence(GeometryCorrectionInput {
                project_id,
                run_id,
                image_id,
                annotation_id: annotagent_core::AnnotationId::new(),
                source_node_id: NodeId::from("detector"),
                source_model_profile_id: Some(model_id),
                source_model_revision: Some(4),
                candidate_artifact_id: ArtifactId::new(),
                reference_artifact_id: ArtifactId::new(),
                original_geometry: GeometrySnapshot {
                    rect: NormalizedRect::new(0.1, 0.1, 0.3, 0.3).expect("original"),
                    image_width: 640,
                    image_height: 480,
                },
                corrected_geometry: GeometrySnapshot {
                    rect: NormalizedRect::new(0.15, 0.15, 0.15, 0.15).expect("corrected"),
                    image_width: 640,
                    image_height: 480,
                },
                reason: GeometryCorrectionReason::TooLoose,
                created_at: Utc::now(),
            });
        store
            .save_geometry_correction(&report, &evidence)
            .expect("geometry evidence");
        let project_records = store
            .list_project_geometry_corrections(project_id, 20)
            .expect("project records");
        assert_eq!(project_records, vec![(report.clone(), evidence.clone())]);
        assert_eq!(
            store
                .list_run_geometry_corrections(run_id, 20)
                .expect("Run records"),
            vec![(report, evidence)]
        );
    }

    #[test]
    fn project_policy_and_immutable_calibration_report_round_trip() {
        let store = SqliteStore::open_in_memory().expect("in-memory database");
        let project_id = ProjectId::new();
        let mut policy = ProjectGeometryPolicy::conservative_default(
            project_id,
            annotagent_core::TaskKind::BoundingBox,
        );
        policy.calibration_thresholds.minimum_sample_count = 5;
        store
            .save_project_geometry_policy(&policy)
            .expect("save policy");
        assert_eq!(
            store
                .get_project_geometry_policy(project_id, annotagent_core::TaskKind::BoundingBox)
                .expect("get policy"),
            Some(policy.clone())
        );
        assert_eq!(
            store
                .list_project_geometry_policies(project_id)
                .expect("list policies"),
            vec![policy.clone()]
        );

        let report = annotagent_core::evaluate_geometry_calibration(
            GeometryCalibrationKey {
                project_id,
                task_id: TaskId::from("objects"),
                label_id: Some(LabelId::from("ball")),
                model_profile_id: ModelProfileId::new(),
                model_profile_revision: 1,
                node_definition_id: "detector".to_owned(),
                node_config_hash: "node-hash".to_owned(),
                prompt_version: None,
                preprocessing_hash: "preprocess-hash".to_owned(),
                dataset_profile_revision: "dataset-v1".to_owned(),
                label_schema_hash: "labels-v1".to_owned(),
                refinement_hash: "refiners-v1".to_owned(),
            },
            GeometryCalibrationThresholds::default(),
            &[],
            0,
            Utc::now(),
        );
        store
            .save_geometry_calibration(&report)
            .expect("save calibration");
        assert_eq!(
            store
                .get_geometry_calibration(report.id)
                .expect("get calibration"),
            Some(report.clone())
        );
        assert_eq!(
            store
                .list_project_geometry_calibrations(project_id)
                .expect("list calibrations"),
            vec![report.clone()]
        );
        assert!(store.save_geometry_calibration(&report).is_err());
    }
}
