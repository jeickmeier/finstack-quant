//! wasm-bindgen-test suite for `finstack_quant_wasm::api::cashflows`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::{cashflows, valuations::pricing::price_instrument};
use wasm_bindgen_test::*;

fn cashflow_spec_json() -> String {
    serde_json::json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None",
        },
        "issue": "2024-08-31",
        "maturity": "2025-08-31",
        "coupon_program": [{
            "kind": "fixed",
            "spec": {
              "coupon_type": "Cash",
              "rate": "0.06",
              "freq": {"count": 12, "unit": "months"},
              "dc": "Thirty360",
              "bdc": "following",
              "calendar_id": "weekends_only",
              "stub": "None",
              "end_of_month": false,
              "payment_lag_days": 0,
            },
        }],
    })
    .to_string()
}

fn market_context_json() -> String {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    let base = time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(disc);
    serde_json::to_string(&ctx).unwrap()
}

fn floating_cashflow_spec_json() -> String {
    serde_json::json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None",
        },
        "issue": "2025-01-15",
        "maturity": "2026-01-15",
        "coupon_program": [{
            "kind": "floating",
            "spec": {
              "rate_spec": {
                "index_id": "USD-SOFR-3M",
                "spread_bp": "150.0",
                "gearing": "1.0",
                "gearing_includes_spread": true,
                "index_floor_bp": null,
                "all_in_floor_bp": null,
                "all_in_cap_bp": null,
                "index_cap_bp": null,
                "reset_freq": {"count": 3, "unit": "months"},
                "reset_lag_days": 0,
                "fixing_calendar_id": null,
                "overnight_compounding": null,
                "overnight_basis": null,
              },
              "coupon_type": "Cash",
              "freq": {"count": 3, "unit": "months"},
              "dc": "Act360",
              "bdc": "following",
              "calendar_id": "weekends_only",
              "stub": "None",
              "end_of_month": false,
              "payment_lag_days": 0,
            },
        }],
    })
    .to_string()
}

fn step_up_cashflow_spec_json() -> String {
    serde_json::json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None",
        },
        "issue": "2024-01-01",
        "maturity": "2026-01-01",
        "coupon_program": [{
            "kind": "step_up",
            "spec": {
                "coupon_type": "Cash",
                "initial_rate": "0.06",
                "step_schedule": [["2025-01-01", "0.07"]],
                "freq": {"count": 12, "unit": "months"},
                "dc": "Thirty360",
                "bdc": "following",
                "calendar_id": "weekends_only",
                "stub": "None",
            },
        }],
        "payment_program": [{
            "kind": "program",
            "steps": [
                {"date": "2025-01-01", "split": "PIK"},
                {"date": "2026-01-01", "split": "Cash"},
            ],
        }],
    })
    .to_string()
}

fn floating_market_context_json() -> String {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

    let base = time::Date::from_calendar_date(2025, time::Month::January, 15).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 0.03), (1.0, 0.04), (5.0, 0.05)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(disc).insert(fwd);
    serde_json::to_string(&ctx).unwrap()
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_builds_accrues_and_prices_custom_bond() {
    let schedule_json = cashflows::build_cashflow_schedule_json(&cashflow_spec_json(), None)
        .expect("schedule should build");
    let validated =
        cashflows::validate_cashflow_schedule_json(&schedule_json).expect("valid schedule");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&schedule_json).unwrap(),
        serde_json::from_str::<serde_json::Value>(&validated).unwrap()
    );
    let flows_json = cashflows::dated_flows_json(&schedule_json).expect("dated flows");
    let flows: Vec<serde_json::Value> = serde_json::from_str(&flows_json).unwrap();
    let schedule: serde_json::Value = serde_json::from_str(&schedule_json).unwrap();
    assert_eq!(flows.len(), schedule["flows"].as_array().unwrap().len());
    assert!(cashflows::accrued_interest_json(&schedule_json, "2025-02-28", None).unwrap() > 0.0);

    let instrument_json =
        cashflows::bond_from_cashflows_json("CUSTOM-CF", &schedule_json, "USD-OIS", Some(99.0))
            .expect("bond JSON");
    let result_json = price_instrument(
        &instrument_json,
        &market_context_json(),
        "2024-09-03",
        "discounting",
    )
    .expect("price custom bond");
    let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(result["instrument_id"], "CUSTOM-CF");
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_builds_floating_schedule_with_market_json() {
    let schedule_json = cashflows::build_cashflow_schedule_json(
        &floating_cashflow_spec_json(),
        Some(floating_market_context_json()),
    )
    .expect("floating schedule should build with market");
    let schedule: serde_json::Value = serde_json::from_str(&schedule_json).unwrap();
    let float_flows: Vec<_> = schedule["flows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|flow| flow["kind"] == "FloatReset")
        .collect();

    assert!(!float_flows.is_empty());
    assert!(float_flows
        .iter()
        .all(|flow| flow["rate"].as_f64().unwrap() > 0.015));
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_builds_step_up_with_payment_program() {
    let schedule_json =
        cashflows::build_cashflow_schedule_json(&step_up_cashflow_spec_json(), None)
            .expect("step-up schedule builds");
    let schedule: serde_json::Value =
        serde_json::from_str(&schedule_json).expect("schedule JSON parses");
    assert!(schedule["flows"]
        .as_array()
        .expect("flows array")
        .iter()
        .any(|flow| flow["kind"] == "PIK"));
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_accepts_config_and_missing_quoted_clean() {
    let schedule_json = cashflows::build_cashflow_schedule_json(&cashflow_spec_json(), None)
        .expect("schedule should build");
    let config_json = serde_json::json!({
        "method": "Linear",
        "include_pik": true,
        "frequency": {"count": 12, "unit": "months"},
    })
    .to_string();

    assert!(
        cashflows::accrued_interest_json(&schedule_json, "2025-02-28", Some(config_json)).unwrap()
            > 0.0
    );

    let instrument_json =
        cashflows::bond_from_cashflows_json("CUSTOM-CF-NO-QUOTE", &schedule_json, "USD-OIS", None)
            .expect("bond JSON");
    let instrument: serde_json::Value = serde_json::from_str(&instrument_json).unwrap();
    assert_eq!(instrument["spec"]["id"], "CUSTOM-CF-NO-QUOTE");
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_rejects_bad_inputs() {
    let schedule_json = cashflows::build_cashflow_schedule_json(&cashflow_spec_json(), None)
        .expect("schedule should build");

    assert!(cashflows::validate_cashflow_schedule_json("{not json").is_err());
    assert!(cashflows::accrued_interest_json(&schedule_json, "2025-02-30", None).is_err());
}

#[wasm_bindgen_test]
fn cashflows_json_bridge_rejects_amortization_over_notional() {
    let schedule_json = cashflows::build_cashflow_schedule_json(&cashflow_spec_json(), None)
        .expect("schedule should build");
    let mut schedule: serde_json::Value = serde_json::from_str(&schedule_json).unwrap();
    schedule["flows"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "date": "2025-03-31",
            "reset_date": null,
            "amount": {"amount": "1000011", "currency": "USD"},
            "kind": "Amortization",
            "accrual_factor": 0.0,
            "rate": null,
        }));

    assert!(cashflows::validate_cashflow_schedule_json(&schedule.to_string()).is_err());
}
