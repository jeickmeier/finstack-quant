//! wasm-bindgen-test suite for `api::portfolio`.
//!
//! Every portfolio computation result is a plain structured JavaScript object.
//! These tests are the only place that contract can be exercised: `JsValue`
//! cannot be constructed off `wasm32`, so the in-crate `#[cfg(test)]` modules
//! keep only the `String` / `f64` / `Option<f64>` exports.
//!
//! Each object-returning assertion goes through [`as_json`], which reads the
//! value back via `JSON.stringify`. That is the ES-`Map` regression gate: a
//! `serde_wasm_bindgen` default serializer emits ES2015 `Map`s, whose property
//! reads silently yield `undefined` and which `JSON.stringify` renders as `{}`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::portfolio::sensitivity::decompose_factor_risk;
use finstack_quant_wasm::api::portfolio::*;
use finstack_quant_wasm::api::scenarios::build_scenario_spec;
use finstack_quant_wasm::utils::{contract_to_js_error, materialization_to_js_error};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn portfolio_spec_json() -> String {
    r#"{"id":"test_portfolio","name":"Test","base_currency":"USD","as_of":"2024-01-15","entities":{},"positions":[]}"#.to_string()
}

fn empty_market_json() -> String {
    let ctx = finstack_quant_core::market_data::context::MarketContext::new();
    serde_json::to_string(&ctx).unwrap()
}

/// Assert a binding result is a real structured JS value and return it as
/// `serde_json::Value`.
///
/// `JSON.stringify` is the load-bearing step: an ES2015 `Map` stringifies to
/// `{}` and would fail the non-empty check below.
fn as_json(value: &JsValue) -> serde_json::Value {
    let text = js_sys::JSON::stringify(value)
        .expect("result must be JSON.stringify-able")
        .as_string()
        .expect("JSON.stringify yields a string");
    assert!(
        text.len() > 2,
        "JSON.stringify round-trip must not be empty (ES Map regression): {text}"
    );
    serde_json::from_str(&text).expect("stringified result must be valid JSON")
}

/// Read one own property directly off the JS object, proving it is not a `Map`.
fn get_f64(value: &JsValue, key: &str) -> f64 {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .unwrap_or_else(|_| panic!("property read for {key}"))
        .as_f64()
        .unwrap_or_else(|| panic!("property {key} must be directly readable as a number"))
}

#[wasm_bindgen_test]
fn portfolio_result_get_metric_returns_undefined_for_missing() {
    let spec = portfolio_spec_json();
    let market = empty_market_json();
    let valuation = value_portfolio(&spec, &market, Some(false), None).unwrap();
    let result = finstack_quant_portfolio::results::PortfolioResult::new(
        serde_wasm_bindgen::from_value(valuation).unwrap(),
        Default::default(),
        Default::default(),
    );
    let result_json = serde_json::to_string(&result).unwrap();
    let v = portfolio_result_get_metric(&result_json, "nonexistent").unwrap();
    assert_eq!(v, None);
}

#[wasm_bindgen_test]
fn value_portfolio_returns_a_structured_object() {
    let spec = portfolio_spec_json();
    let market = empty_market_json();
    let valuation = value_portfolio(&spec, &market, Some(false), None).unwrap();
    // Direct property reads: a `Map` would give `undefined` for each of these.
    for key in ["as_of", "position_values", "total_base_currency"] {
        assert!(
            !js_sys::Reflect::get(&valuation, &JsValue::from_str(key))
                .unwrap()
                .is_undefined(),
            "valuation.{key} must be directly readable"
        );
    }
    let parsed = as_json(&valuation);
    assert_eq!(parsed["as_of"], "2024-01-15");
    assert_eq!(parsed["total_base_currency"]["currency"], "USD");
}

#[wasm_bindgen_test]
fn aggregate_full_cashflows_returns_a_structured_object_for_an_empty_portfolio() {
    let spec = portfolio_spec_json();
    let market = empty_market_json();
    let result = aggregate_full_cashflows(&spec, &market, None).unwrap();
    let parsed = as_json(&result);

    assert_eq!(parsed["events"], serde_json::json!([]));
    assert_eq!(parsed["by_position"], serde_json::json!({}));
    assert_eq!(parsed["by_date"], serde_json::json!({}));
    assert_eq!(parsed["position_summaries"], serde_json::json!({}));
    assert_eq!(parsed["issues"], serde_json::json!([]));
}

#[wasm_bindgen_test]
fn aggregate_full_cashflows_built_matches_the_spec_path() {
    let spec_json = portfolio_spec_json();
    let handle = JsPortfolio::from_spec(&spec_json).unwrap();
    let market = empty_market_json();

    let via_built = aggregate_full_cashflows_built(&handle, &market, None).unwrap();
    let via_spec = aggregate_full_cashflows(&spec_json, &market, None).unwrap();
    assert_eq!(as_json(&via_built), as_json(&via_spec));
}

#[wasm_bindgen_test]
fn aggregate_metrics_returns_a_structured_object() {
    let spec = portfolio_spec_json();
    let market = empty_market_json();
    let valuation = value_portfolio(&spec, &market, Some(false), None).unwrap();
    let valuation_json = js_sys::JSON::stringify(&valuation)
        .unwrap()
        .as_string()
        .unwrap();
    let metrics = aggregate_metrics(&valuation_json, "USD", &market, "2024-01-15").unwrap();
    assert!(as_json(&metrics).is_object());
}

#[wasm_bindgen_test]
fn brinson_fachler_reconstructs_active_return() {
    let sectors = serde_json::json!([
        {
            "sector": "A",
            "portfolio_weight": 0.60,
            "benchmark_weight": 0.40,
            "portfolio_return": 0.08,
            "benchmark_return": 0.06
        },
        {
            "sector": "B",
            "portfolio_weight": 0.40,
            "benchmark_weight": 0.60,
            "portfolio_return": 0.01,
            "benchmark_return": 0.03
        }
    ]);
    let result = brinson_fachler(&sectors.to_string()).unwrap();
    let reconstructed = get_f64(&result, "total_allocation")
        + get_f64(&result, "total_selection")
        + get_f64(&result, "total_interaction");
    assert!((reconstructed - get_f64(&result, "total_excess_return")).abs() < 1e-12);
    assert!(as_json(&result).is_object());
}

#[wasm_bindgen_test]
fn carino_link_reconstructs_compounded_active_return() {
    let periods = serde_json::json!([
        [
            {
                "sector": "A",
                "portfolio_weight": 0.70,
                "benchmark_weight": 0.50,
                "portfolio_return": 0.10,
                "benchmark_return": 0.06
            },
            {
                "sector": "B",
                "portfolio_weight": 0.30,
                "benchmark_weight": 0.50,
                "portfolio_return": 0.04,
                "benchmark_return": 0.05
            }
        ],
        [
            {
                "sector": "A",
                "portfolio_weight": 0.60,
                "benchmark_weight": 0.50,
                "portfolio_return": 0.02,
                "benchmark_return": 0.03
            },
            {
                "sector": "B",
                "portfolio_weight": 0.40,
                "benchmark_weight": 0.50,
                "portfolio_return": -0.01,
                "benchmark_return": 0.00
            }
        ]
    ]);
    let result = carino_link(&periods.to_string()).unwrap();
    let geometric_active = get_f64(&result, "portfolio_return_compounded")
        - get_f64(&result, "benchmark_return_compounded");
    let reconstructed = get_f64(&result, "linked_allocation")
        + get_f64(&result, "linked_selection")
        + get_f64(&result, "linked_interaction");

    assert!((reconstructed - geometric_active).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn twrr_linked_geometrically_links_returns() {
    let result = twrr_linked(&serde_json::json!([0.05, 0.03]).to_string(), 1.0).unwrap();
    assert!((get_f64(&result, "cumulative") - 0.0815).abs() < 1e-12);
    assert!((get_f64(&result, "annualised") - 0.0815).abs() < 1e-12);
    assert_eq!(as_json(&result)["num_periods"], serde_json::json!(2));
}

#[wasm_bindgen_test]
fn replay_portfolio_returns_a_structured_object() {
    let market_val: serde_json::Value = serde_json::from_str(&empty_market_json()).unwrap();
    let snapshots_json = serde_json::json!([
        {"date": "2024-01-15", "market": market_val},
        {"date": "2024-01-16", "market": market_val}
    ])
    .to_string();
    let config_json =
        serde_json::json!({"mode": "pv_only", "attribution_method": "parallel"}).to_string();

    let result = replay_portfolio(&portfolio_spec_json(), &snapshots_json, &config_json).unwrap();
    let parsed = as_json(&result);
    assert!(parsed["steps"].is_array());
    assert_eq!(parsed["steps"].as_array().unwrap().len(), 2);
}

#[wasm_bindgen_test]
fn mo25_26_decompose_factor_risk_accepts_zero_factors_with_canonical_measure() {
    let sensitivities = serde_json::json!({
        "base_currency": "USD",
        "position_ids": [],
        "factor_ids": [],
        "data": []
    })
    .to_string();
    let covariance =
        finstack_quant_models::factor::FactorCovarianceMatrix::new(Vec::new(), Vec::new())
            .expect("empty covariance should build");
    let covariance_json = serde_json::to_string(&covariance).expect("serialize covariance");

    let output = decompose_factor_risk(&sensitivities, &covariance_json, None)
        .expect("MO-26: zero-factor decomposition should be accepted");
    let value = as_json(&output);
    assert_eq!(value["total_risk"], 0.0);
    assert_eq!(
        value["measure"], "variance",
        "MO-25: measure should use canonical serde form, not Debug"
    );
}

// Campisi fixed-income attribution.
//
// Mirrors the hand-worked golden fixture in
// `finstack-quant/portfolio/src/fi_attribution.rs`.

#[allow(clippy::too_many_arguments)]
fn campisi_snap(
    sector: &str,
    weight: f64,
    total_return: f64,
    yield_annual: f64,
    modified_duration: f64,
    spread_duration: f64,
    spread: f64,
    delta_treasury_yield: f64,
    delta_spread: f64,
) -> serde_json::Value {
    serde_json::json!({
        "sector": sector,
        "weight": weight,
        "total_return": total_return,
        "yield_annual": yield_annual,
        "modified_duration": modified_duration,
        "spread_duration": spread_duration,
        "spread": spread,
        "delta_treasury_yield": delta_treasury_yield,
        "delta_spread": delta_spread,
    })
}

fn campisi_golden_portfolio() -> serde_json::Value {
    serde_json::json!([
        campisi_snap("GOVT", 0.30, 0.0155, 0.040, 5.0, 0.0, 0.0, -0.0010, 0.0),
        campisi_snap("GOVT", 0.20, 0.0190, 0.045, 8.0, 0.0, 0.0, -0.0010, 0.0),
        campisi_snap("CORP", 0.30, 0.0120, 0.060, 4.0, 3.8, 0.0150, -0.0010, 0.0020),
        campisi_snap("CORP", 0.20, 0.0118, 0.070, 6.0, 5.5, 0.0250, -0.0010, 0.0020),
    ])
}

fn campisi_golden_benchmark() -> serde_json::Value {
    serde_json::json!([
        campisi_snap("GOVT", 0.45, 0.0155, 0.038, 6.0, 0.0, 0.0, -0.0010, 0.0),
        campisi_snap("GOVT", 0.15, 0.0195, 0.042, 9.0, 0.0, 0.0, -0.0010, 0.0),
        campisi_snap("CORP", 0.25, 0.0090, 0.055, 5.0, 4.8, 0.0120, -0.0010, 0.0020),
        campisi_snap("CORP", 0.15, 0.0100, 0.065, 7.0, 6.5, 0.0200, -0.0010, 0.0020),
    ])
}

fn campisi_config(period_years: f64) -> String {
    serde_json::json!({ "period_years": period_years }).to_string()
}

/// Re-serialize a structured attribution result so it can be fed back into the
/// `*Json`-argument linkers — the JS-side `JSON.stringify(result)` step.
fn stringify(value: &JsValue) -> String {
    js_sys::JSON::stringify(value)
        .expect("stringify")
        .as_string()
        .expect("string")
}

/// Pins the canonical Rust golden numbers through the binding and checks the
/// five effects telescope to the active return.
#[wasm_bindgen_test]
fn campisi_attribution_matches_rust_golden_and_reconciles() {
    let result = campisi_attribution(
        &campisi_golden_portfolio().to_string(),
        &campisi_golden_benchmark().to_string(),
        &campisi_config(0.25),
    )
    .expect("campisi attribution");

    let get = |key: &str| get_f64(&result, key);
    // Argument-order guard: swapping portfolio/benchmark flips these signs.
    assert!((get("portfolio_return") - 0.01441).abs() < 1e-12);
    assert!((get("benchmark_return") - 0.01365).abs() < 1e-12);
    assert!((get("active_return") - 0.00076).abs() < 1e-12);
    assert!((get("total_allocation") - -0.0007125).abs() < 1e-12);
    assert!((get("total_active_carry") - 0.00103125).abs() < 1e-12);
    assert!((get("total_active_treasury") - -0.00075).abs() < 1e-12);
    assert!((get("total_active_spread") - 0.0009575).abs() < 1e-12);
    assert!((get("total_selection") - 0.00023375).abs() < 1e-12);

    let reconstructed = get("total_allocation")
        + get("total_active_carry")
        + get("total_active_treasury")
        + get("total_active_spread")
        + get("total_selection");
    assert!((reconstructed - get("active_return")).abs() < 1e-12);

    let parsed = as_json(&result);
    assert!(
        parsed.get("spread_mode").is_none(),
        "the removed spread_mode stamp must not reappear in the result"
    );
    // Sector ordering is portfolio-first-seen.
    let sectors = parsed["sectors"].as_array().expect("sectors");
    assert_eq!(sectors[0]["sector"], "GOVT");
    assert_eq!(sectors[1]["sector"], "CORP");
}

/// `period_years` is the config's only field and has no default; the binding
/// must fail closed when it is omitted and when the retired `spread_mode` key
/// is still supplied.
#[wasm_bindgen_test]
fn campisi_attribution_rejects_unknown_and_missing_config_fields() {
    let portfolio = campisi_golden_portfolio().to_string();
    let benchmark = campisi_golden_benchmark().to_string();

    assert!(campisi_attribution(&portfolio, &benchmark, &campisi_config(0.25)).is_ok());
    assert!(campisi_attribution(&portfolio, &benchmark, "{}").is_err());
    assert!(campisi_attribution(
        &portfolio,
        &benchmark,
        r#"{"period_years": 0.25, "spread_mode": "dts"}"#
    )
    .is_err());
    assert!(campisi_attribution("[]", &benchmark, &campisi_config(0.25)).is_err());
}

/// `campisiCarinoLink` binds Rust `campisi_carino_link`, which links
/// *precomputed* results and therefore carries no shared `period_years`.
/// Feeding it two periods computed with *different* period lengths must
/// succeed — exactly what the snapshot-based entry point cannot express.
#[wasm_bindgen_test]
fn campisi_carino_link_accepts_periods_of_different_lengths() {
    let portfolio = campisi_golden_portfolio().to_string();
    let benchmark = campisi_golden_benchmark().to_string();

    // 31/365 and 28/365 — a real act/365 monthly pair.
    let jan = campisi_attribution(&portfolio, &benchmark, &campisi_config(31.0 / 365.0))
        .expect("january");
    let feb = campisi_attribution(&portfolio, &benchmark, &campisi_config(28.0 / 365.0))
        .expect("february");

    let periods = format!("[{},{}]", stringify(&jan), stringify(&feb));
    let linked = campisi_carino_link(&periods).expect("carino link over unequal periods");

    let get = |key: &str| get_f64(&linked, key);
    let geometric = get("portfolio_return_compounded") - get("benchmark_return_compounded");
    let reconstructed = get("linked_allocation")
        + get("linked_active_carry")
        + get("linked_active_treasury")
        + get("linked_active_spread")
        + get("linked_selection");
    assert!((reconstructed - geometric).abs() < 1e-10);

    // The two periods carry different carry.
    assert!(
        (get_f64(&jan, "total_active_carry") - get_f64(&feb, "total_active_carry")).abs() > 1e-9
    );

    let parsed = as_json(&linked);
    assert_eq!(parsed["periods"].as_array().expect("periods").len(), 2);
    let names: Vec<&str> = parsed["linked_sectors"]
        .as_array()
        .expect("linked_sectors")
        .iter()
        .map(|s| s["sector"].as_str().expect("sector"))
        .collect();
    assert_eq!(names, ["GOVT", "CORP"]);
}

#[wasm_bindgen_test]
fn campisi_carino_link_rejects_empty_and_inconsistent_periods() {
    assert!(campisi_carino_link("[]").is_err());

    let result = campisi_attribution(
        &campisi_golden_portfolio().to_string(),
        &campisi_golden_benchmark().to_string(),
        &campisi_config(0.25),
    )
    .expect("period");
    let canonical = stringify(&result);
    let mut other: serde_json::Value = serde_json::from_str(&canonical).expect("json");
    other["sectors"][0]["sector"] = serde_json::json!("DIFFERENT");
    let periods = format!("[{canonical},{other}]");
    assert!(campisi_carino_link(&periods).is_err());
}

/// `campisiCarinoLinkFromSnapshots` binds Rust
/// `campisi_carino_link_from_snapshots`: raw period snapshots plus one shared
/// config.
#[wasm_bindgen_test]
fn campisi_carino_link_from_snapshots_reconstructs_compounded_active_return() {
    let period = serde_json::json!({
        "portfolio": campisi_golden_portfolio(),
        "benchmark": campisi_golden_benchmark(),
    });
    let periods = serde_json::json!([period, period]);

    let linked = campisi_carino_link_from_snapshots(&periods.to_string(), &campisi_config(0.25))
        .expect("carino link from snapshots");

    let get = |key: &str| get_f64(&linked, key);
    // Hand-worked compounded returns: 1.01441^2 − 1 and 1.01365^2 − 1.
    let rp = 1.01441_f64.powi(2) - 1.0;
    let rb = 1.01365_f64.powi(2) - 1.0;
    assert!((get("portfolio_return_compounded") - rp).abs() < 1e-12);
    assert!((get("benchmark_return_compounded") - rb).abs() < 1e-12);

    let geometric = rp - rb;
    let reconstructed = get("linked_allocation")
        + get("linked_active_carry")
        + get("linked_active_treasury")
        + get("linked_active_spread")
        + get("linked_selection");
    assert!((reconstructed - geometric).abs() < 1e-10);

    // Carino smoothing is not a no-op here.
    let arithmetic = 2.0 * 0.00076;
    assert!((arithmetic - geometric).abs() > 1e-7);
    let scale = geometric / arithmetic;
    assert!((get("linked_active_spread") - 2.0 * 0.0009575 * scale).abs() < 1e-12);
    assert!((get("linked_allocation") - 2.0 * -0.0007125 * scale).abs() < 1e-12);
}

/// The snapshot entry point takes `FiPeriodInput` objects, not
/// `FiAttributionResult`s — feeding it the other function's input must fail.
#[wasm_bindgen_test]
fn campisi_carino_link_entry_points_are_not_interchangeable() {
    let portfolio = campisi_golden_portfolio().to_string();
    let benchmark = campisi_golden_benchmark().to_string();
    let config = campisi_config(0.25);
    let result = campisi_attribution(&portfolio, &benchmark, &config).expect("period");

    // Results JSON is not FiPeriodInput.
    let periods = format!("[{}]", stringify(&result));
    assert!(campisi_carino_link_from_snapshots(&periods, &config).is_err());

    // Snapshot period JSON is not FiAttributionResult.
    let period = serde_json::json!({
        "portfolio": campisi_golden_portfolio(),
        "benchmark": campisi_golden_benchmark(),
    });
    assert!(campisi_carino_link(&serde_json::json!([period]).to_string()).is_err());
}

/// The reconciliation gate must be reachable through the binding, must honour
/// the supplied tolerance, and must fail closed on a result payload carrying an
/// unknown field.
#[wasm_bindgen_test]
fn campisi_reconciliation_check_honours_tolerance_and_denies_unknown_fields() {
    let config = campisi_config(0.25);
    let result = campisi_attribution(
        &campisi_golden_portfolio().to_string(),
        &campisi_golden_benchmark().to_string(),
        &config,
    )
    .expect("period");
    let canonical = stringify(&result);

    let report = campisi_reconciliation_check(&canonical, 1e-10).expect("report");
    assert_eq!(as_json(&report)["is_reconciled"], serde_json::json!(true));
    assert!(get_f64(&report, "total_residual").abs() <= 1e-10);

    // Tolerance bites: tamper with active_return and the identity breaks.
    let mut tampered: serde_json::Value = serde_json::from_str(&canonical).expect("parse");
    let active = tampered["active_return"].as_f64().expect("active_return");
    tampered["active_return"] = serde_json::json!(active + 0.01);
    let tampered = tampered.to_string();
    let strict = campisi_reconciliation_check(&tampered, 1e-10).expect("report");
    assert_eq!(as_json(&strict)["is_reconciled"], serde_json::json!(false));
    let loose = campisi_reconciliation_check(&tampered, 1.0).expect("report");
    assert_eq!(as_json(&loose)["is_reconciled"], serde_json::json!(true));

    // Unknown fields fail closed on both result-consuming entry points.
    let mut bogus: serde_json::Value = serde_json::from_str(&canonical).expect("parse");
    bogus["bogus_field"] = serde_json::json!(1.0);
    let bogus = bogus.to_string();
    assert!(campisi_reconciliation_check(&bogus, 1e-10).is_err());
    assert!(campisi_carino_link(&format!("[{bogus}]")).is_err());
}

fn empty_scenario_json() -> String {
    let operations =
        serde_wasm_bindgen::to_value(&Vec::<finstack_quant_scenarios::OperationSpec>::new())
            .expect("operations");
    let value =
        build_scenario_spec("stress", operations, None, None, None, None, None).expect("scenario");
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_wasm_bindgen::from_value(value).expect("typed scenario");
    serde_json::to_string(&spec).expect("scenario json")
}

#[wasm_bindgen_test]
fn apply_scenario_and_revalue_empty_portfolio() {
    let spec = portfolio_spec_json();
    let scenario = empty_scenario_json();
    let market = empty_market_json();
    let result = apply_scenario_and_revalue(&spec, &scenario, &market).unwrap();
    let obj = as_json(&result);
    assert!(obj["valuation"].is_object());
    assert!(obj["report"].is_object());
}

#[wasm_bindgen_test]
fn apply_scenario_and_revalue_built_empty_portfolio() {
    let spec = portfolio_spec_json();
    let portfolio = JsPortfolio::from_spec(&spec).unwrap();
    let scenario = empty_scenario_json();
    let market = empty_market_json();
    let result = apply_scenario_and_revalue_built(&portfolio, &scenario, &market).unwrap();
    let obj = as_json(&result);
    assert!(obj["valuation"].is_object());
    assert!(obj["report"].is_object());
}

#[wasm_bindgen_test]
fn contract_error_js_kinds_follow_variants_not_messages() {
    use finstack_quant_core::contract::ContractError;

    let cases = [
        (
            ContractError::UnsupportedVersion {
                contract: "missing curve".to_string(),
                found: 3,
                min: 1,
                max: 2,
            },
            "unsupported_version",
        ),
        (
            ContractError::MissingVersion {
                contract: "malformed validation".to_string(),
            },
            "missing_version",
        ),
        (
            ContractError::MalformedSchema {
                value: "missing curve".to_string(),
                expected: "not found".to_string(),
            },
            "malformed_schema",
        ),
        (
            ContractError::LimitExceeded {
                what: "malformed validation",
                found: 2,
                limit: 1,
            },
            "limit_exceeded",
        ),
        (ContractError::Report(Box::default()), "report"),
        (
            ContractError::Core(finstack_quant_core::Error::Internal(
                "missing curve malformed validation".to_string(),
            )),
            "core",
        ),
    ];

    for (error, expected) in cases {
        let js_error = contract_to_js_error(error);
        let kind = js_sys::Reflect::get(&js_error, &JsValue::from_str("kind"))
            .unwrap()
            .as_string()
            .unwrap();
        assert_eq!(kind, expected);
    }
}

#[wasm_bindgen_test]
fn materialization_limit_and_invalid_input_have_distinct_js_kinds() {
    let cases = [
        (
            finstack_quant_portfolio::Error::ContractLimitExceeded {
                what: "bytes".to_string(),
                found: 2,
                limit: 1,
            },
            "limit_exceeded",
        ),
        (
            finstack_quant_portfolio::Error::InvalidInput(
                "unrelated validation failure".to_string(),
            ),
            "invalid_input",
        ),
    ];

    for (error, expected) in cases {
        let js_error = materialization_to_js_error(error);
        let kind = js_sys::Reflect::get(&js_error, &JsValue::from_str("kind"))
            .unwrap()
            .as_string()
            .unwrap();
        assert_eq!(kind, expected);
    }
}
