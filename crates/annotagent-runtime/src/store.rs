use annotagent_core::{
    Annotation, AnnotationRevision, ArtifactId, ArtifactValidationState, ImageId, LabelId,
    ModelMessage, ProjectId, RunEvent, RunId, RunStatus, TaskId, TaskRunStatus, ToolCallId,
    ToolResult, UsageRecord, ValidationIssue, VisionArtifact,
};
use async_trait::async_trait;

#[async_trait]
pub trait RuntimeStore: Send + Sync {
    async fn create_run(&self, run: &RunRecord) -> Result<(), String>;
    async fn set_run_status(
        &self,
        run_id: RunId,
        status: RunStatus,
        reason: Option<&str>,
    ) -> Result<(), String>;
    async fn set_task_run_status(
        &self,
        run_id: RunId,
        image_id: ImageId,
        task_id: &TaskId,
        status: TaskRunStatus,
        reason: Option<&str>,
    ) -> Result<(), String>;
    async fn record_event(&self, event: &RunEvent) -> Result<(), String>;
    async fn record_usage(&self, run_id: RunId, usage: &UsageRecord) -> Result<(), String>;
    async fn record_model_message(
        &self,
        run_id: RunId,
        image_id: Option<ImageId>,
        task_id: Option<&TaskId>,
        message: &ModelMessage,
    ) -> Result<(), String>;
    async fn record_tool_call(
        &self,
        run_id: RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: &serde_json::Value,
        result: Option<&ToolResult>,
        error: Option<&str>,
    ) -> Result<(), String>;
    async fn record_artifact(&self, run_id: RunId, artifact: &VisionArtifact)
    -> Result<(), String>;
    async fn set_artifact_validation_state(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
        state: ArtifactValidationState,
    ) -> Result<(), String>;
    async fn find_artifact(
        &self,
        run_id: RunId,
        artifact_id: ArtifactId,
    ) -> Result<Option<VisionArtifact>, String>;
    async fn record_validation(
        &self,
        run_id: RunId,
        issues: &[ValidationIssue],
    ) -> Result<(), String>;
    async fn commit_annotation(&self, run_id: RunId, annotation: &Annotation)
    -> Result<(), String>;
    async fn record_revision(&self, revision: &AnnotationRevision) -> Result<(), String>;
    async fn correction_risk(
        &self,
        project_id: ProjectId,
        skill_id: &str,
        task_id: &TaskId,
        label: Option<&LabelId>,
    ) -> Result<f32, String>;
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: RunId,
    pub project_id: ProjectId,
    pub project_name: String,
    pub skill_id: String,
    pub provider: String,
    pub model: String,
    pub status: RunStatus,
    pub project_schema_json: String,
    pub workflow_snapshot_json: Option<String>,
}

#[derive(Default)]
pub struct MemoryRuntimeStore {
    events: std::sync::Mutex<Vec<RunEvent>>,
    annotations: std::sync::Mutex<Vec<Annotation>>,
    usage: std::sync::Mutex<Vec<UsageRecord>>,
    model_messages: std::sync::Mutex<Vec<ModelMessage>>,
    artifacts: std::sync::Mutex<Vec<VisionArtifact>>,
}

impl MemoryRuntimeStore {
    pub fn events(&self) -> Result<Vec<RunEvent>, String> {
        self.events
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "event store lock poisoned".to_owned())
    }

    pub fn annotations(&self) -> Result<Vec<Annotation>, String> {
        self.annotations
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "annotation store lock poisoned".to_owned())
    }

    pub fn usage(&self) -> Result<Vec<UsageRecord>, String> {
        self.usage
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "usage store lock poisoned".to_owned())
    }

    pub fn model_messages(&self) -> Result<Vec<ModelMessage>, String> {
        self.model_messages
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "model message store lock poisoned".to_owned())
    }

    pub fn artifacts(&self) -> Result<Vec<VisionArtifact>, String> {
        self.artifacts
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "artifact store lock poisoned".to_owned())
    }
}

#[async_trait]
impl RuntimeStore for MemoryRuntimeStore {
    async fn create_run(&self, _run: &RunRecord) -> Result<(), String> {
        Ok(())
    }

    async fn set_run_status(
        &self,
        _run_id: RunId,
        _status: RunStatus,
        _reason: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn set_task_run_status(
        &self,
        _run_id: RunId,
        _image_id: ImageId,
        _task_id: &TaskId,
        _status: TaskRunStatus,
        _reason: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn record_event(&self, event: &RunEvent) -> Result<(), String> {
        self.events
            .lock()
            .map_err(|_| "event store lock poisoned".to_owned())?
            .push(event.clone());
        Ok(())
    }

    async fn record_usage(&self, _run_id: RunId, usage: &UsageRecord) -> Result<(), String> {
        self.usage
            .lock()
            .map_err(|_| "usage store lock poisoned".to_owned())?
            .push(usage.clone());
        Ok(())
    }

    async fn record_model_message(
        &self,
        _run_id: RunId,
        _image_id: Option<ImageId>,
        _task_id: Option<&TaskId>,
        message: &ModelMessage,
    ) -> Result<(), String> {
        self.model_messages
            .lock()
            .map_err(|_| "model message store lock poisoned".to_owned())?
            .push(message.clone());
        Ok(())
    }

    async fn record_tool_call(
        &self,
        _run_id: RunId,
        _call_id: &ToolCallId,
        _name: &str,
        _arguments: &serde_json::Value,
        _result: Option<&ToolResult>,
        _error: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn record_artifact(
        &self,
        _run_id: RunId,
        artifact: &VisionArtifact,
    ) -> Result<(), String> {
        self.artifacts
            .lock()
            .map_err(|_| "artifact store lock poisoned".to_owned())?
            .push(artifact.clone());
        Ok(())
    }

    async fn set_artifact_validation_state(
        &self,
        _run_id: RunId,
        artifact_id: ArtifactId,
        state: ArtifactValidationState,
    ) -> Result<(), String> {
        let mut artifacts = self
            .artifacts
            .lock()
            .map_err(|_| "artifact store lock poisoned".to_owned())?;
        let artifact = artifacts
            .iter_mut()
            .find(|artifact| artifact.id == artifact_id)
            .ok_or_else(|| format!("artifact {artifact_id} was not found"))?;
        artifact.validation_state = state;
        Ok(())
    }

    async fn find_artifact(
        &self,
        _run_id: RunId,
        artifact_id: ArtifactId,
    ) -> Result<Option<VisionArtifact>, String> {
        Ok(self
            .artifacts
            .lock()
            .map_err(|_| "artifact store lock poisoned".to_owned())?
            .iter()
            .find(|artifact| artifact.id == artifact_id)
            .cloned())
    }

    async fn record_validation(
        &self,
        _run_id: RunId,
        _issues: &[ValidationIssue],
    ) -> Result<(), String> {
        Ok(())
    }

    async fn commit_annotation(
        &self,
        _run_id: RunId,
        annotation: &Annotation,
    ) -> Result<(), String> {
        self.annotations
            .lock()
            .map_err(|_| "annotation store lock poisoned".to_owned())?
            .push(annotation.clone());
        Ok(())
    }

    async fn correction_risk(
        &self,
        _project_id: ProjectId,
        _skill_id: &str,
        _task_id: &TaskId,
        _label: Option<&LabelId>,
    ) -> Result<f32, String> {
        Ok(0.0)
    }

    async fn record_revision(&self, _revision: &AnnotationRevision) -> Result<(), String> {
        Ok(())
    }
}
