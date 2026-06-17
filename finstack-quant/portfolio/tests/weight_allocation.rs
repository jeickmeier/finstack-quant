//! Strategy-level allocation JSON API tests.

use finstack_quant_portfolio::allocate_weights;
use serde_json::{json, Value};

#[test]
fn allocate_weights_supports_equal_fixed_and_inverse_volatility() {
    let equal = json!({
        "scheme": "equal",
        "total_capital": 1000.0,
        "strategies": [{"id": "value:1"}, {"id": "carry:1"}],
        "money_decimal_places": 2
    });
    let out = allocate_weights(&equal.to_string()).expect("equal allocation");
    let result: Value = serde_json::from_str(&out).expect("equal JSON");
    assert_eq!(result["scheme"], "equal");
    assert!((result["allocations"][0]["weight"].as_f64().expect("weight") - 0.5).abs() < 1e-12);
    assert_eq!(result["allocations"][0]["capital"], 500.0);

    let fixed = json!({
        "scheme": "fixed",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "value:1", "fixed_weight": 0.25},
            {"id": "carry:1", "fixed_weight": 0.75}
        ],
        "money_decimal_places": 2
    });
    let out = allocate_weights(&fixed.to_string()).expect("fixed allocation");
    let result: Value = serde_json::from_str(&out).expect("fixed JSON");
    assert!((result["allocations"][1]["weight"].as_f64().expect("weight") - 0.75).abs() < 1e-12);

    let inverse_vol = json!({
        "scheme": "inverse_volatility",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "low_vol", "returns": [0.01, 0.02, 0.01, 0.02]},
            {"id": "high_vol", "returns": [0.05, -0.05, 0.05, -0.05]}
        ],
        "money_decimal_places": 2
    });
    let out = allocate_weights(&inverse_vol.to_string()).expect("inverse vol allocation");
    let result: Value = serde_json::from_str(&out).expect("inverse vol JSON");
    let low_weight = result["allocations"][0]["weight"]
        .as_f64()
        .expect("low weight");
    let high_weight = result["allocations"][1]["weight"]
        .as_f64()
        .expect("high weight");
    assert!(low_weight > high_weight);
    assert!((low_weight + high_weight - 1.0).abs() < 1e-12);
}

#[test]
fn allocate_weights_solves_diagonal_risk_budget() {
    let spec = json!({
        "scheme": "risk_budget",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "value:1", "risk_budget": 0.25},
            {"id": "carry:1", "risk_budget": 0.75}
        ],
        "covariance": [[0.04, 0.0], [0.0, 0.01]],
        "money_decimal_places": 2
    });

    let out = allocate_weights(&spec.to_string()).expect("risk budget allocation");
    let result: Value = serde_json::from_str(&out).expect("risk budget JSON");
    assert_eq!(result["scheme"], "risk_budget");
    let first = result["allocations"][0]["weight"]
        .as_f64()
        .expect("first weight");
    let second = result["allocations"][1]["weight"]
        .as_f64()
        .expect("second weight");
    let expected_first = 2.5 / (2.5 + 0.75_f64.sqrt() / 0.1);
    assert!((first - expected_first).abs() < 1e-8);
    assert!((second - (1.0 - expected_first)).abs() < 1e-8);
    assert!((result["diagnostics"]["weights_sum"].as_f64().expect("sum") - 1.0).abs() < 1e-12);
}

#[test]
fn allocate_weights_accepts_singular_psd_covariance() {
    let spec = json!({
        "scheme": "risk_budget",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "higher_vol", "risk_budget": 0.5},
            {"id": "same_vol", "risk_budget": 0.5}
        ],
        "covariance": [[1.0, 1.0], [1.0, 1.0]],
        "money_decimal_places": 2
    });

    let out = allocate_weights(&spec.to_string()).expect("rank-one PSD covariance should be valid");
    let result: Value = serde_json::from_str(&out).expect("risk budget JSON");
    assert!(
        (result["allocations"][0]["weight"]
            .as_f64()
            .expect("first weight")
            - 0.5)
            .abs()
            < 1e-8
    );
    assert!(
        (result["allocations"][1]["weight"]
            .as_f64()
            .expect("second weight")
            - 0.5)
            .abs()
            < 1e-8
    );
}

#[test]
fn allocate_weights_rejects_invalid_fixed_weights() {
    let spec = json!({
        "scheme": "fixed",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "value:1", "fixed_weight": 0.25},
            {"id": "carry:1", "fixed_weight": 0.50}
        ]
    });

    let err = allocate_weights(&spec.to_string()).expect_err("fixed weights must sum to one");
    assert!(err.to_string().contains("sum"));
}

#[test]
fn single_strategy_inverse_volatility_does_not_require_returns() {
    let spec = json!({
        "scheme": "inverse_volatility",
        "total_capital": 1000.0,
        "strategies": [{"id": "only"}],
        "money_decimal_places": 2
    });

    let out = allocate_weights(&spec.to_string()).expect("single strategy inverse-vol");
    let result: Value = serde_json::from_str(&out).expect("allocation JSON");
    assert_eq!(result["allocations"][0]["weight"], 1.0);
    assert!(result["allocations"][0].get("volatility").is_none());
}

#[test]
fn single_strategy_risk_budget_still_validates_budget() {
    let missing = json!({
        "scheme": "risk_budget",
        "total_capital": 1000.0,
        "strategies": [{"id": "only"}],
        "covariance": [[0.04]]
    });
    let err = allocate_weights(&missing.to_string()).expect_err("missing risk budget");
    assert!(err.to_string().contains("risk_budget"));

    let wrong_sum = json!({
        "scheme": "risk_budget",
        "total_capital": 1000.0,
        "strategies": [{"id": "only", "risk_budget": 0.5}],
        "covariance": [[0.04]]
    });
    let err = allocate_weights(&wrong_sum.to_string()).expect_err("risk budget must sum to one");
    assert!(err.to_string().contains("sum"));
}
