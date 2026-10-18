//! Generic factor identifiers, definitions, and market dependencies.

/// Factor definitions and market-data bump mappings.
pub mod definition;
/// Market dependency descriptors used by factor matchers.
pub mod dependency;
/// Factor identifiers and asset-class type tags.
pub mod factor_types;

pub use definition::{FactorDefinition, MarketMapping};
pub use dependency::{CurveType, DependencyType, MarketDependency};
pub use factor_types::{FactorId, FactorType};
