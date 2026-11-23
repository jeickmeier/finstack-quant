//! Matching primitives for mapping market dependencies to factor identifiers.

pub(crate) mod config;
pub(crate) mod credit;
pub(crate) mod filter;
pub(crate) mod matchers;

pub(crate) use config::MatchingConfig;
pub(crate) use credit::{
    CREDIT_GENERIC_FACTOR_ID, CreditHierarchicalConfig, bucket_factor_id_from_path,
};
#[cfg(test)]
pub(crate) use filter::{AttributeFilter, DependencyFilter};
pub(crate) use matchers::MappingRule;

#[cfg(test)]
pub(super) use matchers::deepest_match;
