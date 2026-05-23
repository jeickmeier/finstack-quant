use crate::types::PositionId;
use finstack_core::types::Attributes;
use finstack_factor_model::matching::FactorMatcher;
use finstack_factor_model::{FactorId, MarketDependency};

/// Assignment results for a portfolio-level factor mapping pass.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FactorAssignmentReport {
    /// Per-position matched dependencies and factor identifiers.
    pub assignments: Vec<PositionAssignment>,
    /// Dependencies that did not match any configured factor.
    pub unmatched: Vec<UnmatchedEntry>,
}

/// Matched factor assignments for a single portfolio position.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PositionAssignment {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Matched `(dependency, factor_id)` pairs for this position.
    pub mappings: Vec<(MarketDependency, FactorId)>,
}

/// Single unmatched dependency surfaced during assignment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnmatchedEntry {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Dependency that could not be matched.
    pub dependency: MarketDependency,
}

/// Assign factor identifiers to a single position's market dependencies.
pub(crate) fn assign_position_factors(
    position_id: &PositionId,
    dependencies: &[MarketDependency],
    attributes: &Attributes,
    matcher: &dyn FactorMatcher,
) -> (PositionAssignment, Vec<UnmatchedEntry>) {
    let mut mappings = Vec::new();
    let mut unmatched = Vec::new();

    for dependency in dependencies {
        let matched_factor = matcher
            .match_factor_with_betas(dependency, attributes)
            .ok()
            .flatten()
            .and_then(|entries| entries.into_iter().next_back())
            .map(|entry| entry.factor_id);
        if let Some(factor_id) = matched_factor {
            mappings.push((dependency.clone(), factor_id));
        } else {
            unmatched.push(UnmatchedEntry {
                position_id: position_id.clone(),
                dependency: dependency.clone(),
            });
        }
    }

    (
        PositionAssignment {
            position_id: position_id.clone(),
            mappings,
        },
        unmatched,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::types::{Attributes, CurveId};
    use finstack_factor_model::matching::{
        AttributeFilter, DependencyFilter, MappingRule, MappingTableMatcher,
    };
    use finstack_factor_model::{CurveType, DependencyType, FactorId, MarketDependency};

    #[test]
    fn test_assign_position_factors_reports_matches_and_unmatched() {
        let matcher = MappingTableMatcher::new(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Discount),
                curve_type: Some(CurveType::Discount),
                id: None,
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("Rates"),
        }]);
        let dependencies = vec![
            MarketDependency::Curve {
                id: CurveId::new("USD-OIS"),
                curve_type: CurveType::Discount,
            },
            MarketDependency::Spot { id: "AAPL".into() },
        ];

        let (assignment, unmatched) = assign_position_factors(
            &PositionId::new("pos-1"),
            &dependencies,
            &Attributes::default(),
            &matcher,
        );

        assert_eq!(assignment.position_id, PositionId::new("pos-1"));
        assert_eq!(assignment.mappings.len(), 1);
        assert_eq!(assignment.mappings[0].1, FactorId::new("Rates"));
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].position_id, PositionId::new("pos-1"));
        assert_eq!(
            unmatched[0].dependency,
            MarketDependency::Spot { id: "AAPL".into() }
        );
    }
}
