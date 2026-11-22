//! Matching primitives for mapping market dependencies to factor identifiers.

mod config;
mod credit;
mod filter;
mod matchers;

pub use config::{HierarchicalConfig, MatchingConfig};
pub(crate) use credit::bucket_factor_id_from_path;
pub use credit::{
    CREDIT_GENERIC_FACTOR_ID, CreditHierarchicalConfig, ISSUER_ID_META_KEY, bucket_factor_id,
    parse_bucket_factor_id,
};
pub use filter::{AttributeFilter, DependencyFilter};
pub use matchers::{FactorMatchEntry, FactorMatchError, FactorMatcher, FactorNode, MappingRule};

#[cfg(test)]
pub(super) fn deepest_match(
    matcher: &dyn FactorMatcher,
    dependency: &crate::MarketDependency,
    attributes: &finstack_quant_core::types::Attributes,
) -> Option<crate::FactorId> {
    matcher
        .match_factor(dependency, attributes)
        .ok()
        .flatten()
        .and_then(|entries| entries.last().map(|entry| entry.factor_id.clone()))
}
