//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_attribution::attribute_return_contribution;
use serde_json::{json, Value};

#[test]
fn return_contribution_groups_factors_and_brinson_reconcile() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "gross",
        "positions": [
            {
                "id": "AAPL.XNAS",
                "market_value": 9000.0,
                "return": 0.012,
                "groups": {"sector": "tech", "strategy": "value:1"},
                "benchmark_weight": 0.85,
                "benchmark_return": 0.010
            },
            {
                "id": "XOM.XNYS",
                "market_value": 1000.0,
                "return": -0.004,
                "groups": {"sector": "energy"},
                "benchmark_weight": 0.15,
                "benchmark_return": -0.002
            }
        ],
        "factors": [
            {"factor": "value", "exposure": 0.10, "factor_return": 0.02}
        ]
    });

    let out = attribute_return_contribution(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    assert!(
        (result["portfolio_return"]
            .as_f64()
            .expect("portfolio_return")
            - 0.0104)
            .abs()
            < 1e-12
    );
    assert_eq!(result["instrument_contribution"][0]["id"], "AAPL.XNAS");
    assert!(
        (result["instrument_contribution"][0]["weight"]
            .as_f64()
            .expect("weight")
            - 0.9)
            .abs()
            < 1e-12
    );
    let sector_rows = result["group_contribution"]["sector"]
        .as_array()
        .expect("sector rows");
    let tech = sector_rows
        .iter()
        .find(|row| row["key"] == "tech")
        .expect("tech sector");
    assert!((tech["contribution"].as_f64().expect("tech contribution") - 0.0108).abs() < 1e-12);
    let strategy_rows = result["group_contribution"]["strategy"]
        .as_array()
        .expect("strategy rows");
    assert!(strategy_rows.iter().any(|row| row["key"] == "unknown"));
    assert!(
        (result["factor_contribution"][0]["contribution"]
            .as_f64()
            .expect("factor contribution")
            - 0.002)
            .abs()
            < 1e-12
    );

    let relative = &result["benchmark_relative"];
    assert!(!relative.is_null());
    let active = relative["active_return"].as_f64().expect("active_return");
    let reconstructed = relative["allocation_effect"].as_f64().expect("allocation")
        + relative["selection_effect"].as_f64().expect("selection")
        + relative["interaction_effect"]
            .as_f64()
            .expect("interaction");
    assert!((active - reconstructed).abs() < 1e-12);
    assert!(relative["residual"].as_f64().expect("residual").abs() < 1e-12);
}

#[test]
fn return_contribution_rejects_mixed_benchmark_fields() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "gross",
        "positions": [
            {"id": "A", "weight": 0.5, "return": 0.01, "benchmark_weight": 0.5, "benchmark_return": 0.01},
            {"id": "B", "weight": 0.5, "return": 0.02}
        ]
    });

    let err = attribute_return_contribution(&spec.to_string()).expect_err("mixed benchmark fields");
    assert!(err.to_string().contains("benchmark"));
}

#[test]
fn return_contribution_rejects_zero_portfolio_weight_for_benchmark_relative() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "gross",
        "positions": [
            {"id": "A", "market_value": 0.0, "return": 0.01, "benchmark_weight": 1.0, "benchmark_return": 0.01}
        ]
    });

    let err = attribute_return_contribution(&spec.to_string())
        .expect_err("benchmark-relative attribution requires normalized portfolio weights");
    assert!(err.to_string().contains("portfolio weights"));
}
