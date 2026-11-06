//! Matching primitives for mapping market dependencies to factor identifiers.

mod config;
mod credit;
mod filter;
mod matchers;

pub use config::{HierarchicalConfig, MatchingConfig};
pub use credit::{
    CREDIT_GENERIC_FACTOR_ID, CreditHierarchicalConfig, ISSUER_ID_META_KEY, bucket_factor_id,
    parse_bucket_factor_id,
};
pub use filter::{AttributeFilter, DependencyFilter};
pub use matchers::{
    FactorMatchEntry, FactorMatchError, FactorMatcher, FactorNode, MappingRule, MappingTableMatcher,
};
