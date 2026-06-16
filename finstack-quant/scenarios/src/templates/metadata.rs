//! Template metadata types for the stress test template library.

use serde::{Deserialize, Serialize};

/// Severity classification for stress scenarios.
///
/// This label is intended for discovery and filtering rather than for pricing
/// logic. Registries and UIs can use it to group historical events by the
/// magnitude of the modeled dislocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Mild stress with limited market dislocation.
    Mild,
    /// Moderate stress with broader but contained market impact.
    Moderate,
    /// Severe systemic stress with large cross-asset dislocations.
    Severe,
}

/// Asset class categories affected by a stress template.
///
/// These values describe the primary risk buckets touched by a historical
/// scenario so registries can expose coarse filtering and search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    /// Interest rates and fixed income.
    Rates,
    /// Credit spreads and default risk.
    Credit,
    /// Equity prices and dividends.
    Equity,
    /// Foreign exchange rates.
    #[serde(rename = "fx")]
    FX,
    /// Implied and realized volatility.
    Volatility,
    /// Commodity prices.
    Commodity,
}

/// Metadata describing a historical stress test template.
///
/// A template registry stores this metadata alongside a builder factory. The
/// fields are intentionally discovery-oriented: they identify the historical
/// event, the affected asset classes, and any component templates that can be
/// built separately from the composite scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateMetadata {
    /// Stable identifier for the template.
    pub id: String,
    /// Human-readable template name.
    pub name: String,
    /// Description of the historical event and modeled effects.
    pub description: String,
    /// Primary date associated with the historical event.
    ///
    /// This is typically the date of the market dislocation rather than the
    /// valuation date used when the template is later executed.
    pub event_date: time::Date,
    /// Asset classes materially affected by the scenario.
    pub asset_classes: Vec<AssetClass>,
    /// Freeform tags used for filtering and discovery.
    pub tags: Vec<String>,
    /// Severity classification for the template.
    pub severity: Severity,
    /// IDs of composable sub-component templates.
    ///
    /// Empty when the template is atomic rather than a composite built from
    /// multiple reusable scenario fragments.
    pub components: Vec<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use std::collections::HashSet;
    use time::macros::date;

    #[test]
    fn test_metadata_construction() {
        let meta = TemplateMetadata {
            id: "gfc_2008".into(),
            name: "Global Financial Crisis 2008".into(),
            description: "Lehman collapse scenario".into(),
            event_date: date!(2008 - 09 - 15),
            asset_classes: vec![AssetClass::Rates, AssetClass::Credit],
            tags: vec!["systemic".into(), "credit".into()],
            severity: Severity::Severe,
            components: vec!["gfc_2008_rates".into(), "gfc_2008_credit".into()],
        };

        assert_eq!(meta.id, "gfc_2008");
        assert_eq!(meta.severity, Severity::Severe);
        assert_eq!(meta.asset_classes.len(), 2);
    }

    #[test]
    fn test_metadata_serde_roundtrip() {
        let meta = TemplateMetadata {
            id: "test".into(),
            name: "Test".into(),
            description: "A test template".into(),
            event_date: date!(2020 - 03 - 16),
            asset_classes: vec![AssetClass::Equity],
            tags: vec!["test".into()],
            severity: Severity::Mild,
            components: vec![],
        };

        let json = serde_json::to_string(&meta).expect("serialize");
        let deser: TemplateMetadata = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(meta.id, deser.id);
        assert_eq!(meta.severity, deser.severity);
        assert_eq!(meta.event_date, deser.event_date);
    }

    #[test]
    fn test_metadata_serde_contract() {
        let meta = TemplateMetadata {
            id: "test".into(),
            name: "Test".into(),
            description: "A test template".into(),
            event_date: date!(2020 - 03 - 16),
            asset_classes: vec![AssetClass::Equity, AssetClass::FX],
            tags: vec!["systemic".into()],
            severity: Severity::Mild,
            components: vec!["equity".into()],
        };

        let json = serde_json::to_value(&meta).expect("serialize");

        assert_eq!(json["severity"], "mild");
        assert_eq!(json["asset_classes"], serde_json::json!(["equity", "fx"]));
        assert_eq!(json["tags"], serde_json::json!(["systemic"]));
        assert_eq!(json["components"], serde_json::json!(["equity"]));
    }

    #[test]
    fn test_metadata_rejects_unknown_fields() {
        let json = serde_json::json!({
            "id": "test",
            "name": "Test",
            "description": "A test template",
            "event_date": "2020-03-16",
            "asset_classes": ["equity"],
            "tags": ["systemic"],
            "severity": "mild",
            "components": [],
            "unexpected": true
        });

        let result = serde_json::from_value::<TemplateMetadata>(json);

        assert!(result.is_err());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Mild < Severity::Moderate);
        assert!(Severity::Moderate < Severity::Severe);
    }

    #[test]
    fn test_asset_class_ordering() {
        assert!(AssetClass::Rates < AssetClass::Credit);
    }

    #[test]
    fn test_asset_class_set_coverage() {
        let classes: HashSet<_> = [
            AssetClass::Rates,
            AssetClass::Credit,
            AssetClass::Equity,
            AssetClass::FX,
            AssetClass::Volatility,
            AssetClass::Commodity,
        ]
        .into_iter()
        .collect();

        assert_eq!(classes.len(), 6);
        assert!(classes.contains(&AssetClass::Rates));
        assert!(classes.contains(&AssetClass::Credit));
        assert!(classes.contains(&AssetClass::Equity));
        assert!(classes.contains(&AssetClass::FX));
        assert!(classes.contains(&AssetClass::Volatility));
        assert!(classes.contains(&AssetClass::Commodity));
    }
}
