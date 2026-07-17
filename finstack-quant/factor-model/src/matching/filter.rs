//! Configuration and metadata filters for dependency-scoped factor matching.
//!
use crate::primitives::dependency::{CurveType, DependencyType, MarketDependency};
use finstack_quant_core::types::Attributes;
use serde::{Deserialize, Serialize};

/// Filters on instrument metadata.
///
/// All configured conditions are combined with logical AND.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeFilter {
    /// Tags that must all be present on the instrument.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Metadata key/value pairs that must all match exactly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meta: Vec<(String, String)>,
}

impl AttributeFilter {
    /// Returns whether the attributes satisfy this filter.
    #[must_use]
    pub fn matches(&self, attrs: &Attributes) -> bool {
        let tags_match = self.tags.iter().all(|tag| attrs.has_tag(tag));
        let meta_match = self
            .meta
            .iter()
            .all(|(key, value)| attrs.get_meta(key) == Some(value.as_str()));
        tags_match && meta_match
    }
}

/// Filters on an individual market dependency.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyFilter {
    /// Dependency classification that the dependency must match, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_type: Option<DependencyType>,
    /// Specific curve role that the dependency must match, when present.
    ///
    /// This is only evaluated for curve-like dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_type: Option<CurveType>,
    /// Exact dependency identifier that must match, when present.
    ///
    /// FX pairs use the canonical `BASE/QUOTE` form, for example `USD/EUR`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl DependencyFilter {
    /// Returns whether the dependency satisfies this filter.
    #[must_use]
    pub fn matches(&self, dep: &MarketDependency) -> bool {
        let dependency_type_matches = match self.dependency_type {
            Some(dependency_type) => dep.matches_dependency_type(dependency_type),
            None => true,
        };
        let curve_type_matches = match self.curve_type {
            Some(curve_type) => dep.matches_curve_type(curve_type),
            None => true,
        };
        let id_matches = match self.id.as_deref() {
            Some(expected_id) => dep.matches_id(expected_id),
            None => true,
        };

        dependency_type_matches && curve_type_matches && id_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CurveType;
    use finstack_quant_core::types::CurveId;

    #[test]
    fn test_attribute_filter_empty_matches_all() {
        let filter = AttributeFilter::default();
        let attrs = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA");
        assert!(filter.matches(&attrs));
    }

    #[test]
    fn test_attribute_filter_tag_match() {
        let filter = AttributeFilter {
            tags: vec!["energy".into()],
            meta: vec![],
        };
        let matching = Attributes::default().with_tag("energy");
        let not_matching = Attributes::default().with_tag("financials");
        assert!(filter.matches(&matching));
        assert!(!filter.matches(&not_matching));
    }

    #[test]
    fn test_attribute_filter_meta_match() {
        let filter = AttributeFilter {
            tags: vec![],
            meta: vec![("region".into(), "NA".into())],
        };
        let matching = Attributes::default().with_meta("region", "NA");
        let not_matching = Attributes::default().with_meta("region", "EU");
        assert!(filter.matches(&matching));
        assert!(!filter.matches(&not_matching));
    }

    #[test]
    fn test_attribute_filter_conjunction() {
        let filter = AttributeFilter {
            tags: vec!["energy".into()],
            meta: vec![
                ("region".into(), "NA".into()),
                ("rating".into(), "CCC".into()),
            ],
        };
        let full_match = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA")
            .with_meta("rating", "CCC");
        let partial = Attributes::default()
            .with_tag("energy")
            .with_meta("region", "NA");
        assert!(filter.matches(&full_match));
        assert!(!filter.matches(&partial));
    }

    #[test]
    fn test_dependency_filter_by_type() {
        let filter = DependencyFilter {
            dependency_type: Some(DependencyType::Credit),
            curve_type: None,
            id: None,
        };
        let credit = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };
        let rate = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        assert!(filter.matches(&credit));
        assert!(!filter.matches(&rate));
    }

    #[test]
    fn test_dependency_filter_by_id() {
        let filter = DependencyFilter {
            dependency_type: None,
            curve_type: None,
            id: Some("USD-OIS".into()),
        };
        let matching = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        let not_matching = MarketDependency::Curve {
            id: CurveId::new("EUR-OIS"),
            curve_type: CurveType::Discount,
        };
        assert!(filter.matches(&matching));
        assert!(!filter.matches(&not_matching));
    }

    #[test]
    fn test_dependency_filter_empty_matches_all() {
        let filter = DependencyFilter::default();
        let dep = MarketDependency::Spot { id: "AAPL".into() };
        assert!(filter.matches(&dep));
    }

    #[test]
    fn test_filters_serde_roundtrip() {
        let filter = AttributeFilter {
            tags: vec!["energy".into()],
            meta: vec![("region".into(), "NA".into())],
        };
        let json_result = serde_json::to_string(&filter);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let roundtrip_result: Result<AttributeFilter, _> = serde_json::from_str(&json);
        assert!(roundtrip_result.is_ok());
        let Ok(roundtrip) = roundtrip_result else {
            return;
        };

        assert_eq!(filter, roundtrip);
    }

    #[test]
    fn test_dependency_filter_serde_roundtrip() {
        let filter = DependencyFilter {
            dependency_type: Some(DependencyType::Credit),
            curve_type: Some(CurveType::Hazard),
            id: Some("ACME-HAZARD".into()),
        };
        let json_result = serde_json::to_string(&filter);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let roundtrip_result: Result<DependencyFilter, _> = serde_json::from_str(&json);
        assert!(roundtrip_result.is_ok());
        let Ok(roundtrip) = roundtrip_result else {
            return;
        };

        assert_eq!(filter, roundtrip);
    }

    #[test]
    fn test_dependency_filter_id_matches_fx_pair() {
        let filter = DependencyFilter {
            dependency_type: Some(DependencyType::Fx),
            curve_type: None,
            id: Some("USD/EUR".into()),
        };
        let dep = MarketDependency::FxPair {
            base: finstack_quant_core::currency::Currency::USD,
            quote: finstack_quant_core::currency::Currency::EUR,
        };

        assert!(filter.matches(&dep));
    }

    #[test]
    fn test_dependency_filter_by_curve_type() {
        let filter = DependencyFilter {
            dependency_type: None,
            curve_type: Some(CurveType::Inflation),
            id: None,
        };
        let inflation = MarketDependency::Curve {
            id: CurveId::new("US-CPI"),
            curve_type: CurveType::Inflation,
        };
        let discount = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };

        assert!(filter.matches(&inflation));
        assert!(!filter.matches(&discount));
    }
}
