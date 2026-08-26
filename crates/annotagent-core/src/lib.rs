//! Stable, domain-neutral data types and extension contracts for `AnnotAgent`.

pub mod annotation;
pub mod error;
pub mod event;
pub mod geometry;
pub mod ids;
pub mod project;
pub mod skill;
pub mod traits;
pub mod usage;

pub use annotation::*;
pub use error::*;
pub use event::*;
pub use geometry::*;
pub use ids::*;
pub use project::*;
pub use skill::*;
pub use traits::*;
pub use usage::*;

/// Version used by exported run-history documents.
pub const HISTORY_SCHEMA_VERSION: u32 = 1;
