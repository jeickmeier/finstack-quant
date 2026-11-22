//! Phase 2 strict persisted factor-definition contract regressions.

use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::types::CurveId;
use finstack_quant_factor::credit::calibration::{
    BetaShrinkage, CovarianceStrategy, VolModelChoice,
};
use finstack_quant_factor::credit::decomposition::{LevelsAtDate, PeriodDecomposition};
use finstack_quant_factor::credit::hierarchy::{
    AdderVolSource, FactorHistories, IssuerBetaPolicy, VolModel, VolState,
};
use finstack_quant_factor::risk::{
    FactorContribution, PositionFactorContribution, PositionResidualContribution,
    PositionRiskDecomposition, ResidualContributionSource, RiskBudgetResult, StressAttribution,
};
use finstack_quant_factor::{FactorDefinition, FactorId, FactorType, MarketMapping, RiskMeasure};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn assert_rejects_unknown_at<T: DeserializeOwned>(mut value: Value, pointer: &str) {
    let target = if pointer.is_empty() {
        value.as_object_mut()
    } else {
        value.pointer_mut(pointer).and_then(Value::as_object_mut)
    };
    let Some(target) = target else {
        panic!("expected JSON object at {pointer:?} in {value}");
    };
    target.insert("__nonexistent__".into(), json!(true));
    let error = serde_json::from_value::<T>(value)
        .err()
        .unwrap_or_else(|| panic!("unknown field at {pointer:?} must fail"));
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn factor_definition_rejects_unknown_fields() {
    let definition = FactorDefinition {
        id: FactorId::new("USD-Rates"),
        factor_type: FactorType::Rates,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS")],
            units: BumpUnits::RateBp,
        },
        description: None,
    };
    let value = serde_json::to_value(definition).expect("definition serializes");
    assert_rejects_unknown_at::<FactorDefinition>(value, "");
}

#[test]
fn closed_factor_enums_and_credit_wires_reject_unknown_fields() {
    assert_rejects_unknown_at::<MarketMapping>(
        json!({"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}}),
        "/curve_parallel",
    );
    assert_rejects_unknown_at::<RiskMeasure>(json!({"var": {"confidence": 0.99}}), "/var");
    assert_rejects_unknown_at::<VolModelChoice>(json!({"ewma": {"lambda": 0.94}}), "/ewma");
    assert_rejects_unknown_at::<CovarianceStrategy>(json!({"ridge": {"alpha": 1.0}}), "/ridge");
    assert_rejects_unknown_at::<BetaShrinkage>(
        json!({"toward_one": {"alpha": 0.5}}),
        "/toward_one",
    );
    assert_rejects_unknown_at::<IssuerBetaPolicy>(
        json!({"dynamic": {"min_history": 12, "overrides": {}}}),
        "/dynamic",
    );
    assert_rejects_unknown_at::<AdderVolSource>(
        json!({"bucket_peer_proxy": {"peer_bucket": "IG.EU"}}),
        "/bucket_peer_proxy",
    );
    assert_rejects_unknown_at::<VolModel>(json!({"sample": {"variance": 0.04}}), "/sample");
    assert_rejects_unknown_at::<VolModel>(
        json!({"ewma": {"lambda": 0.94, "variance": 0.04}}),
        "/ewma",
    );
    assert_rejects_unknown_at::<VolState>(json!({"factors": {}, "idiosyncratic": {}}), "");
    assert_rejects_unknown_at::<FactorHistories>(json!({"dates": [], "values": {}}), "");

    let levels = json!({
        "date": "2024-03-29",
        "generic": 100.0,
        "by_level": [{"level_index": 0, "dimension": "rating", "values": {}}],
        "adder": {}
    });
    assert_rejects_unknown_at::<LevelsAtDate>(levels.clone(), "");
    assert_rejects_unknown_at::<LevelsAtDate>(levels, "/by_level/0");

    let period = json!({
        "from": "2024-03-28",
        "to": "2024-03-29",
        "d_generic": 1.0,
        "by_level": [{"level_index": 0, "dimension": "rating", "deltas": {}}],
        "d_adder": {}
    });
    assert_rejects_unknown_at::<PeriodDecomposition>(period.clone(), "");
    assert_rejects_unknown_at::<PeriodDecomposition>(period, "/by_level/0");
}

#[test]
fn closed_risk_wires_reject_unknown_fields() {
    let budget = json!({
        "positions": [{
            "position_id": "A",
            "actual_component_var": -1.0,
            "target_component_var": 1.0,
            "utilization": 1.0,
            "excess": 0.0
        }],
        "total_overbudget": 0.0,
        "has_breach": false
    });
    assert_rejects_unknown_at::<RiskBudgetResult>(budget.clone(), "");
    assert_rejects_unknown_at::<RiskBudgetResult>(budget, "/positions/0");

    let pos_decomp = json!({
        "portfolio_var": -1.0,
        "portfolio_es": -1.2,
        "confidence": 0.95,
        "method": "parametric",
        "var_contributions": [{
            "position_id": "A",
            "component_var": -1.0,
            "relative_var": 1.0,
            "marginal_var": null,
            "incremental_var": null
        }],
        "es_contributions": [{
            "position_id": "A",
            "component_es": -1.2,
            "relative_es": 1.0,
            "marginal_es": null
        }],
        "n_positions": 1,
        "euler_residual": 0.0
    });
    assert_rejects_unknown_at::<PositionRiskDecomposition>(pos_decomp.clone(), "");
    assert_rejects_unknown_at::<PositionRiskDecomposition>(
        pos_decomp.clone(),
        "/var_contributions/0",
    );
    assert_rejects_unknown_at::<PositionRiskDecomposition>(pos_decomp, "/es_contributions/0");

    let stress = json!({
        "var_threshold": -1.0,
        "n_tail_scenarios": 1,
        "position_ids": ["A"],
        "position_contributions": [{
            "position_id": "A",
            "avg_tail_pnl": -1.0,
            "pct_of_tail_loss": 1.0,
            "worst_scenario_pnl": -1.0
        }],
        "tail_scenarios": [{
            "scenario_index": 0,
            "portfolio_pnl": -1.0,
            "position_pnls": [-1.0]
        }]
    });
    assert_rejects_unknown_at::<StressAttribution>(stress.clone(), "");
    assert_rejects_unknown_at::<StressAttribution>(stress.clone(), "/position_contributions/0");
    assert_rejects_unknown_at::<StressAttribution>(stress, "/tail_scenarios/0");

    assert_rejects_unknown_at::<ResidualContributionSource>(
        json!({"kind": "from_credit_model", "issuer_id": "ACME"}),
        "",
    );
    assert_rejects_unknown_at::<PositionResidualContribution>(
        json!({"position_id": "A", "residual_variance": 0.01, "source": {"kind": "other"}}),
        "",
    );
    assert_rejects_unknown_at::<FactorContribution>(
        json!({"factor_id": "RATES", "absolute_risk": 0.04, "relative_risk": 1.0, "marginal_risk": 0.04}),
        "",
    );
    assert_rejects_unknown_at::<PositionFactorContribution>(
        json!({"position_id": "A", "factor_id": "RATES", "risk_contribution": 0.04}),
        "",
    );
}
