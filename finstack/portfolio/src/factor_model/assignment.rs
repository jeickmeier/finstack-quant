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
///
/// Every factor matched for a dependency is recorded — hierarchical matchers
/// (e.g. the credit matcher) emit one `(dependency, factor_id, beta)` triple
/// per level, so a single dependency may appear in several mappings.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PositionAssignment {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Matched `(dependency, factor_id, beta)` triples for this position.
    pub mappings: Vec<(MarketDependency, FactorId, f64)>,
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
///
/// Records every matched `(dependency, factor_id, beta)` triple — not just
/// the deepest hierarchy level — so the assignment report fully represents
/// multi-factor (e.g. credit hierarchical) assignments.
///
/// # Errors
///
/// Propagates [`finstack_factor_model::matching::FactorMatchError`] (as a
/// validation error) when the matcher recognises a dependency but cannot
/// produce a deterministic answer (e.g. a required issuer tag is missing),
/// matching the fail-loud behavior of the sensitivity path.
pub(crate) fn assign_position_factors(
    position_id: &PositionId,
    dependencies: &[MarketDependency],
    attributes: &Attributes,
    matcher: &dyn FactorMatcher,
) -> crate::error::Result<(PositionAssignment, Vec<UnmatchedEntry>)> {
    let mut mappings = Vec::new();
    let mut unmatched = Vec::new();

    for dependency in dependencies {
        let entries = matcher
            .match_factor_with_betas(dependency, attributes)
            .map_err(|e| {
                crate::error::Error::invalid_input(format!(
                    "factor assignment failed for position '{position_id}': {e}"
                ))
            })?;
        match entries {
            Some(entries) if !entries.is_empty() => {
                for entry in entries {
                    mappings.push((dependency.clone(), entry.factor_id, entry.beta));
                }
            }
            _ => unmatched.push(UnmatchedEntry {
                position_id: position_id.clone(),
                dependency: dependency.clone(),
            }),
        }
    }

    Ok((
        PositionAssignment {
            position_id: position_id.clone(),
            mappings,
        },
        unmatched,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::types::{Attributes, CurveId};
    use finstack_factor_model::matching::{
        AttributeFilter, DependencyFilter, FactorMatchEntry, FactorMatchError, MappingRule,
        MappingTableMatcher,
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
        )
        .expect("assignment should succeed");

        assert_eq!(assignment.position_id, PositionId::new("pos-1"));
        assert_eq!(assignment.mappings.len(), 1);
        assert_eq!(assignment.mappings[0].1, FactorId::new("Rates"));
        assert!((assignment.mappings[0].2 - 1.0).abs() < 1e-12);
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].position_id, PositionId::new("pos-1"));
        assert_eq!(
            unmatched[0].dependency,
            MarketDependency::Spot { id: "AAPL".into() }
        );
    }

    #[derive(Debug)]
    enum StubBehavior {
        MultiEntry,
        Error,
    }

    #[derive(Debug)]
    struct StubMatcher {
        behavior: StubBehavior,
    }

    impl FactorMatcher for StubMatcher {
        fn match_factor_with_betas(
            &self,
            _dependency: &MarketDependency,
            _attributes: &Attributes,
        ) -> Result<Option<Vec<FactorMatchEntry>>, FactorMatchError> {
            match self.behavior {
                StubBehavior::MultiEntry => Ok(Some(vec![
                    FactorMatchEntry {
                        factor_id: FactorId::new("Credit-PC"),
                        beta: 0.8,
                    },
                    FactorMatchEntry {
                        factor_id: FactorId::new("Credit-Sector"),
                        beta: 0.5,
                    },
                ])),
                StubBehavior::Error => Err(FactorMatchError::MissingRequiredTag {
                    dimension: "sector".to_string(),
                }),
            }
        }
    }

    #[test]
    fn test_assign_position_factors_records_all_matched_factors_with_betas() {
        let matcher = StubMatcher {
            behavior: StubBehavior::MultiEntry,
        };
        let dependencies = vec![MarketDependency::Spot {
            id: "ISSUER".into(),
        }];

        let (assignment, unmatched) = assign_position_factors(
            &PositionId::new("pos-1"),
            &dependencies,
            &Attributes::default(),
            &matcher,
        )
        .expect("assignment should succeed");

        assert!(unmatched.is_empty());
        assert_eq!(assignment.mappings.len(), 2);
        assert_eq!(assignment.mappings[0].1, FactorId::new("Credit-PC"));
        assert!((assignment.mappings[0].2 - 0.8).abs() < 1e-12);
        assert_eq!(assignment.mappings[1].1, FactorId::new("Credit-Sector"));
        assert!((assignment.mappings[1].2 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_assign_position_factors_propagates_factor_match_error() {
        let matcher = StubMatcher {
            behavior: StubBehavior::Error,
        };
        let dependencies = vec![MarketDependency::Spot {
            id: "ISSUER".into(),
        }];

        let result = assign_position_factors(
            &PositionId::new("pos-1"),
            &dependencies,
            &Attributes::default(),
            &matcher,
        );

        let Err(error) = result else {
            panic!("FactorMatchError must propagate, not be swallowed as unmatched");
        };
        let message = error.to_string();
        assert!(message.contains("pos-1"), "error names position: {message}");
        assert!(
            message.contains("sector"),
            "error names dimension: {message}"
        );
    }
}
