//! Matching primitives for mapping market dependencies to factor identifiers.

mod config;
mod credit;
mod filter;
mod matchers;

pub use crate::credit::hierarchy::dimension_key;
pub use config::{HierarchicalConfig, MatchingConfig};
pub use credit::{
    bucket_factor_id, CreditHierarchicalConfig, CreditHierarchicalMatcher,
    CREDIT_GENERIC_FACTOR_ID, ISSUER_ID_META_KEY,
};
pub use filter::{AttributeFilter, DependencyFilter};
pub use matchers::{
    CascadeMatcher, FactorMatchEntry, FactorMatchError, FactorMatchResult, FactorMatcher,
    FactorNode, HierarchicalMatcher, MappingRule, MappingTableMatcher,
};
