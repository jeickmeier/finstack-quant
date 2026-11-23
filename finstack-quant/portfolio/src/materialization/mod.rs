//! Versioned, content-addressed portfolio materialization fast path.
//!
//! [`crate::PortfolioSpec`] remains the portable embedded
//! interchange format. This module is the native bulk-loading path for
//! normalized stores: an allocation-free preflight enforces collection limits,
//! then the outer document is parsed once into its canonical serde type. Unique
//! instrument envelopes are validated before cache lookup, and positions share
//! cached [`std::sync::Arc`] instrument instances.

mod cache;
mod engine;
mod envelope;
mod report;

pub use cache::InstrumentArtifactCache;
pub use envelope::{
    InstrumentArtifact, MaterializedPosition, MaterializerInfo, PortfolioHeader,
    PortfolioMaterializationEnvelope, PortfolioMaterializationSchema,
};
pub use report::{MaterializationPhases, MaterializationReport};
