//! Shared structured errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("invalid project configuration: {0}")]
    InvalidProject(String),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("skill error: {0}")]
    Skill(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("refinement error: {0}")]
    Refinement(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("model request failed: {0:?}")]
    ModelFailure(ModelFailure),
    #[error("export error: {0}")]
    Export(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

/// Safe machine-readable diagnostics. Never contains remote text, URLs or credentials.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelFailure {
    pub stage: ModelFailureStage,
    pub category: ModelFailureCategory,
    pub http_status: Option<u16>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFailureStage {
    PrepareRequest,
    ProviderRequest,
    ResponseBody,
    ResponseDecode,
    StructuredOutput,
    Handler,
    Recovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFailureCategory {
    Configuration,
    Cancelled,
    Timeout,
    Connection,
    Transport,
    HttpStatus,
    InvalidResponse,
    InvalidStructuredOutput,
    ProviderError,
    LocalError,
    Interrupted,
}
