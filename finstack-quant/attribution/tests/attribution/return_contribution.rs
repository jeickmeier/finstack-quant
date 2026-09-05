//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_attribution::attribute_return_contribution_json;
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

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
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
    // Audit fix: the Brinson output must record which group dimension the
    // decomposition collapsed to (multi-dimensional inputs pick one).
    assert_eq!(
        relative["group_dimension"], "sector",
        "benchmark_relative must record the Brinson group dimension"
    );
    let active = relative["active_return"].as_f64().expect("active_return");
    let reconstructed = relative["allocation_effect"].as_f64().expect("allocation")
        + relative["selection_effect"].as_f64().expect("selection")
        + relative["interaction_effect"]
            .as_f64()
            .expect("interaction");
    assert!((active - reconstructed).abs() < 1e-12);
    assert!(relative["residual"].as_f64().expect("residual").abs() < 1e-12);
}

/// Audit fix (B1): gross weighting is w_i = MV_i / Σ|MV_j| — the numerator
/// keeps its sign; only the denominator is absolute. A short position that
/// gains 5% must contribute negatively. The old code took `.abs()` in the
/// numerator, flipping every short's weight (and contribution) positive.
///
/// `weighting` is omitted so the serde default (`gross`) path is exercised.
#[test]
fn return_contribution_gross_weighting_keeps_short_sign() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {"id": "LONG", "market_value": 100.0, "return": 0.05},
            {"id": "SHORT", "market_value": -100.0, "return": 0.05}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    // +0.5 * 0.05 + (-0.5) * 0.05 == exactly 0.0.
    assert_eq!(
        result["portfolio_return"]
            .as_f64()
            .expect("portfolio_return"),
        0.0,
        "long +5% and short +5% at equal gross MV must net to exactly zero"
    );
    let short = result["instrument_contribution"]
        .as_array()
        .expect("instrument rows")
        .iter()
        .find(|row| row["id"] == "SHORT")
        .expect("SHORT row");
    assert_eq!(
        short["weight"].as_f64().expect("weight"),
        -0.5,
        "short gross weight must keep the sign of its market value"
    );
    assert_eq!(
        short["contribution"].as_f64().expect("contribution"),
        -0.025,
        "short contribution must be negative when the shorted asset rallies"
    );
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

    let err =
        attribute_return_contribution_json(&spec.to_string()).expect_err("mixed benchmark fields");
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

    let err = attribute_return_contribution_json(&spec.to_string())
        .expect_err("benchmark-relative attribution requires normalized portfolio weights");
    assert!(err.to_string().contains("portfolio weights"));
}

/// Audit fix: `net_market_value` weighting on a near-flat long/short book
/// produces highly leveraged weights (tiny denominator). The result must
/// carry a warning so the consumer knows the weights are leveraged rather
/// than discovering it from a 2000× contribution row.
#[test]
fn return_contribution_warns_on_near_zero_net_market_value() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "net_market_value",
        "positions": [
            {"id": "LONG", "market_value": 1000.0, "return": 0.01},
            {"id": "SHORT", "market_value": -999.5, "return": 0.02}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    let warnings = result["warnings"].as_array().expect("warnings array");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().is_some_and(|s| s.contains("net market value"))),
        "near-zero net MV must produce a leveraged-weights warning; got {warnings:?}"
    );
}

/// Audit fix (Mo4): explicit-weight mode without a benchmark never checked
/// Σw. A weight sum away from 1.0 is legitimate (leveraged or partially
/// invested books), so it must warn — not error — so consumers know the
/// contributions are on a non-unit weight basis.
#[test]
fn return_contribution_warns_on_explicit_weights_not_summing_to_one() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {"id": "A", "weight": 0.5, "return": 0.01},
            {"id": "B", "weight": 0.3, "return": 0.02}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    let warnings = result["warnings"].as_array().expect("warnings array");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().is_some_and(|s| s.contains("weights"))),
        "explicit weights summing to 0.8 without a benchmark must warn; got {warnings:?}"
    );
}

/// The Σw warning must NOT fire for a fully-invested explicit-weight book.
#[test]
fn return_contribution_no_warning_for_unit_sum_explicit_weights() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {"id": "A", "weight": 0.5, "return": 0.01},
            {"id": "B", "weight": 0.5, "return": 0.02}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");
    assert!(
        result.get("warnings").is_none(),
        "unit-sum explicit weights must not warn; got {:?}",
        result["warnings"]
    );
}

/// Audit fix (Mo2): when factor rows are supplied, the result must carry the
/// idiosyncratic residual `specific_return = portfolio_return − Σ factor
/// contributions`, so factor attributions reconcile to the total.
#[test]
fn return_contribution_reports_specific_return_with_factors() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {"id": "A", "weight": 1.0, "return": 0.015}
        ],
        "factors": [
            {"factor": "value", "exposure": 0.5, "factor_return": 0.02}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    let specific = result["specific_return"]
        .as_f64()
        .expect("specific_return must be present when factors are supplied");
    // 0.015 - 0.5 * 0.02 = 0.005
    assert!((specific - 0.005).abs() < 1e-12, "specific: {specific}");
}

/// `specific_return` is meaningless without factor rows: the field must be
/// omitted from the JSON entirely (additive schema).
#[test]
fn return_contribution_omits_specific_return_without_factors() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {"id": "A", "weight": 1.0, "return": 0.015}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");
    assert!(
        result.get("specific_return").is_none(),
        "specific_return must be omitted when no factors are supplied; got {:?}",
        result["specific_return"]
    );
}

/// Audit fix (M3): Brinson-Fachler groups absent from one side must use the
/// standard degenerate-group conventions (Bacon, *Practical Portfolio
/// Performance Measurement and Attribution*, 2e, Ch. 5): when `w_b = 0` the
/// benchmark group return defaults to the total benchmark return; when
/// `w_p = 0` the portfolio group return defaults to the benchmark group
/// return. The old code forced both to 0.0, which preserved the active total
/// but mislabelled allocation vs selection vs interaction.
///
/// Portfolio: tech 60% @ +10%, cashlike 40% @ 0%.
/// Benchmark: tech 80% @ +10%, energy 20% @ +10% (R_b = +10%).
/// Under the conventions every group's benchmark-relative return spread and
/// allocation spread is zero except cashlike's, so the whole −4% active
/// return is pure interaction (underweighting nothing, holding cash the
/// benchmark does not).
#[test]
fn return_contribution_brinson_degenerate_groups_use_standard_conventions() {
    let spec = json!({
        "as_of": "2026-01-02",
        "positions": [
            {
                "id": "TECH",
                "weight": 0.6,
                "return": 0.10,
                "groups": {"sector": "tech"},
                "benchmark_weight": 0.8,
                "benchmark_return": 0.10
            },
            {
                "id": "CASH",
                "weight": 0.4,
                "return": 0.0,
                "groups": {"sector": "cashlike"},
                "benchmark_weight": 0.0,
                "benchmark_return": 0.0
            },
            {
                "id": "ENERGY",
                "weight": 0.0,
                "return": 0.0,
                "groups": {"sector": "energy"},
                "benchmark_weight": 0.2,
                "benchmark_return": 0.10
            }
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");

    let relative = &result["benchmark_relative"];
    let active = relative["active_return"].as_f64().expect("active_return");
    let allocation = relative["allocation_effect"].as_f64().expect("allocation");
    let selection = relative["selection_effect"].as_f64().expect("selection");
    let interaction = relative["interaction_effect"]
        .as_f64()
        .expect("interaction");
    let residual = relative["residual"].as_f64().expect("residual");

    assert!((active - (-0.04)).abs() < 1e-12, "active: {active}");
    assert!(allocation.abs() < 1e-12, "allocation: {allocation}");
    assert!(selection.abs() < 1e-12, "selection: {selection}");
    assert!(
        (interaction - (-0.04)).abs() < 1e-12,
        "interaction: {interaction}"
    );
    assert!(
        (allocation + selection + interaction + residual - active).abs() == 0.0,
        "effects plus residual must reconstruct active exactly"
    );

    let warnings = result["warnings"].as_array().expect("warnings array");
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .is_some_and(|s| s.contains("benchmark group return"))),
        "degenerate-group substitution must be surfaced as a warning; got {warnings:?}"
    );
}

/// Audit fix (M6): an exactly-flat net-MV book has no defined net weights.
/// The old code silently returned all-zero weights, making a real long/short
/// P&L day indistinguishable from a flat book. It must fail loud instead.
#[test]
fn return_contribution_rejects_exactly_flat_net_market_value_book() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "net_market_value",
        "positions": [
            {"id": "LONG", "market_value": 100.0, "return": 0.05},
            {"id": "SHORT", "market_value": -100.0, "return": -0.03}
        ]
    });

    let err = attribute_return_contribution_json(&spec.to_string())
        .expect_err("exactly-flat net-MV book must be rejected, not zeroed");
    let message = err.to_string();
    assert!(
        message.contains("net market value"),
        "error must name the flat net-market-value condition; got: {message}"
    );
    assert!(
        message.contains("gross"),
        "error must suggest gross weighting; got: {message}"
    );
}

/// The warning must NOT fire for an ordinary net book — the field is then
/// omitted from the JSON entirely (additive schema).
#[test]
fn return_contribution_no_warning_for_ordinary_net_book() {
    let spec = json!({
        "as_of": "2026-01-02",
        "weighting": "net_market_value",
        "positions": [
            {"id": "A", "market_value": 6000.0, "return": 0.01},
            {"id": "B", "market_value": 4000.0, "return": 0.02}
        ]
    });

    let out = attribute_return_contribution_json(&spec.to_string()).expect("valid spec");
    let result: Value = serde_json::from_str(&out).expect("json result");
    assert!(
        result.get("warnings").is_none(),
        "ordinary net book must not carry warnings; got {:?}",
        result["warnings"]
    );
}

#[test]
fn brinson_rejects_offsetting_group_with_nonzero_contribution() {
    for side in ["portfolio", "benchmark"] {
        let mut positions = json!([
            {"id": "LONG", "weight": 0.5, "return": 0.1, "groups": {"sector": "tech"}, "benchmark_weight": 0.5, "benchmark_return": 0.0},
            {"id": "SHORT", "weight": -0.5, "return": 0.0, "groups": {"sector": "tech"}, "benchmark_weight": 0.5, "benchmark_return": 0.0}
        ]);
        if side == "benchmark" {
            positions[1]["weight"] = json!(0.5);
            positions[1]["benchmark_weight"] = json!(-0.5);
            positions[0]["benchmark_return"] = json!(0.1);
        }
        positions.as_array_mut().unwrap().push(json!({"id": "ANCHOR", "weight": if side == "portfolio" { 1.0 } else { 0.0 }, "return": 0.0, "groups": {"sector": "cash"}, "benchmark_weight": if side == "benchmark" { 1.0 } else { 0.0 }, "benchmark_return": 0.0}));
        let spec = json!({"as_of": "2026-01-02", "positions": positions});
        let error = attribute_return_contribution_json(&spec.to_string()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("zero net {side} weight")),
            "{error}"
        );
    }
}
