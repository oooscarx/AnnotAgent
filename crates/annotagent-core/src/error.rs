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
    #[error("export error: {0}")]
    Export(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
