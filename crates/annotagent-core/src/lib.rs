//! Stable, domain-neutral data types and extension contracts for `AnnotAgent`.

pub mod agent;
pub mod annotation;
pub mod artifact;
pub mod artifact_conversion;
pub mod batch;
pub mod error;
pub mod evaluation;
pub mod event;
pub mod expert_model;
pub mod geometry;
pub mod ids;
pub mod label_pipeline;
pub mod model_profile;
pub mod node_catalog;
pub mod pipeline_builder;
pub mod pipeline_improvement;
pub mod product_pipeline;
pub mod project;
pub mod provider_registry;
pub mod quality_contract;
pub mod skill;
pub mod traits;
pub mod usage;
pub mod vision_backend;
pub mod workflow;

pub use agent::*;
pub use annotation::*;
pub use artifact::*;
pub use artifact_conversion::*;
pub use batch::*;
pub use error::*;
pub use evaluation::*;
pub use event::*;
pub use expert_model::*;
pub use geometry::*;
pub use ids::*;
pub use label_pipeline::*;
pub use model_profile::*;
pub use node_catalog::*;
pub use pipeline_builder::*;
pub use pipeline_improvement::*;
pub use product_pipeline::*;
pub use project::*;
pub use provider_registry::*;
pub use quality_contract::*;
pub use skill::*;
pub use traits::*;
pub use usage::*;
pub use vision_backend::*;
pub use workflow::*;

/// Version used by exported run-history documents.
pub const HISTORY_SCHEMA_VERSION: u32 = 1;
