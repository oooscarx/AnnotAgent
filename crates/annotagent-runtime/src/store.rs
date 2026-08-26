use annotagent_core::{
    Annotation, RunEvent, RunId, RunStatus, ToolCallId, ToolResult, UsageRecord, ValidationIssue,
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
    async fn record_event(&self, event: &RunEvent) -> Result<(), String>;
    async fn record_usage(&self, run_id: RunId, usage: &UsageRecord) -> Result<(), String>;
    async fn record_tool_call(
        &self,
        run_id: RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: &serde_json::Value,
        result: Option<&ToolResult>,
        error: Option<&str>,
    ) -> Result<(), String>;
    async fn record_validation(
        &self,
        run_id: RunId,
        issues: &[ValidationIssue],
    ) -> Result<(), String>;
    async fn commit_annotation(&self, run_id: RunId, annotation: &Annotation)
    -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: RunId,
    pub project_name: String,
    pub skill_id: String,
    pub provider: String,
    pub model: String,
    pub status: RunStatus,
    pub project_schema_json: String,
}

#[derive(Default)]
pub struct MemoryRuntimeStore {
    events: std::sync::Mutex<Vec<RunEvent>>,
    annotations: std::sync::Mutex<Vec<Annotation>>,
    usage: std::sync::Mutex<Vec<UsageRecord>>,
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
}
