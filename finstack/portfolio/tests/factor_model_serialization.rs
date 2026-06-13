//! Factor model serialization tests for portfolio.

use finstack_core::types::CurveId;
use finstack_factor_model::{CurveType, FactorId, MarketDependency, RiskMeasure};
use finstack_portfolio::factor_model::RiskDecomposition;
use finstack_portfolio::factor_model::{
    FactorAssignmentReport, FactorContribution, FactorContributionDelta, PositionAssignment,
    PositionFactorContribution, StressResult, UnmatchedEntry, WhatIfResult,
};
use finstack_portfolio::types::PositionId;

fn roundtrip_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialization should succeed");
    serde_json::from_str(&json).expect("deserialization should succeed")
}

fn assert_roundtrip_value<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let restored = roundtrip_json(value);
    assert_eq!(
        serde_json::to_value(value).expect("value serialization should succeed"),
        serde_json::to_value(&restored).expect("value reserialization should succeed")
    );
}

fn sample_decomposition() -> RiskDecomposition {
    RiskDecomposition {
        total_risk: 100.0,
        measure: RiskMeasure::Variance,
        factor_contributions: vec![FactorContribution {
            factor_id: FactorId::new("Rates"),
            absolute_risk: 60.0,
            relative_risk: 0.6,
            marginal_risk: 0.3,
        }],
        residual_risk: 40.0,
        position_factor_contributions: vec![PositionFactorContribution {
            position_id: PositionId::new("POS_1"),
            factor_id: FactorId::new("Rates"),
            risk_contribution: 60.0,
        }],
        position_residual_contributions: vec![],
    }
}

#[test]
fn test_factor_model_report_types_roundtrip() {
    let dependency = MarketDependency::Curve {
        id: CurveId::new("USD-OIS"),
        curve_type: CurveType::Discount,
    };

    assert_roundtrip_value(&FactorAssignmentReport {
        assignments: vec![PositionAssignment {
            position_id: PositionId::new("POS_1"),
            mappings: vec![(dependency, FactorId::new("Rates"), 1.0)],
        }],
        unmatched: vec![UnmatchedEntry {
            position_id: PositionId::new("POS_2"),
            dependency: MarketDependency::Spot { id: "AAPL".into() },
        }],
    });

    assert_roundtrip_value(&FactorContributionDelta {
        factor_id: FactorId::new("Credit"),
        absolute_change: -12.5,
        relative_change: -0.2,
    });

    assert_roundtrip_value(&WhatIfResult {
        before: sample_decomposition(),
        after: sample_decomposition(),
        delta: vec![FactorContributionDelta {
            factor_id: FactorId::new("Rates"),
            absolute_change: 5.0,
            relative_change: 0.1,
        }],
    });

    assert_roundtrip_value(&StressResult {
        total_pnl: -1_250.0,
        position_pnl: vec![(PositionId::new("POS_1"), -750.0)],
        stressed_decomposition: sample_decomposition(),
    });
}

// ---------------------------------------------------------------------------
// PR-12 backward compat: pre-PR-6 RiskDecomposition JSON round-trip
// ---------------------------------------------------------------------------

/// Old `RiskDecomposition` JSON that lacks the `position_residual_contributions`
/// field (added in PR-6) must still deserialize and the field defaults to empty.
#[test]
fn pre_pr6_risk_decomposition_json_deserializes_residual_defaults_empty() {
    let legacy_json = r#"{
        "total_risk": 0.025,
        "measure": "volatility",
        "factor_contributions": [
            {
                "factor_id": "credit.generic",
                "absolute_risk": 0.020,
                "marginal_risk": 0.018,
                "relative_risk": 0.80
            }
        ],
        "residual_risk": 0.005,
        "position_factor_contributions": []
    }"#;

    let decomp: RiskDecomposition = serde_json::from_str(legacy_json)
        .expect("pre-PR-6 RiskDecomposition JSON should deserialize");
    assert!(
        decomp.position_residual_contributions.is_empty(),
        "position_residual_contributions should default to empty for old payloads"
    );
    assert!((decomp.total_risk - 0.025).abs() < 1e-12);
    assert!((decomp.residual_risk - 0.005).abs() < 1e-12);
    assert_eq!(decomp.factor_contributions.len(), 1);
    assert_eq!(
        decomp.factor_contributions[0].factor_id,
        FactorId::new("credit.generic")
    );
}
