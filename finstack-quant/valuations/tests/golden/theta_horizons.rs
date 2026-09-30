//! Theta runs at 1D, 1M and 6M for every pricing golden and every registry
//! example (UI pricing case).
//!
//! Theta holds the market fixed and rolls the valuation date, so the rolled
//! repricing crosses every index fixing inside the horizon. A fixture whose
//! base valuation prices must also produce a finite theta at each horizon; the
//! only exceptions are listed in [`EXPECTED_FAILURES`] with their reason.

use super::pricing_common::resolve_market;
use super::schema::GoldenFixture;
use super::walk::collect_fixture_paths_under;
use crate::instruments::loan_facility_wire_keys::{pricing_case_inputs, pricing_cases};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};
use std::sync::Arc;

/// Theta horizons as `(label, metric_pricing_overrides.theta_period)`.
const HORIZONS: [(&str, &str); 3] = [
    ("1D", r#"{"count":1,"unit":"days"}"#),
    ("1M", r#"{"count":1,"unit":"months"}"#),
    ("6M", r#"{"count":6,"unit":"months"}"#),
];

/// `(fixture, horizons, reason)` for theta runs that are expected to fail.
/// A listed run that succeeds fails the test, so entries cannot go stale.
const EXPECTED_FAILURES: &[(&str, &[&str], &str)] = &[
    // Instrument-held observation state: not market-held fixings.
    (
        "golden/quantlib/asian_option/spx_arithmetic_asian_call_1y_quantlib",
        &["6M"],
        S16B,
    ),
    (
        "golden/quantlib/asian_option/spx_geometric_asian_call_1y_quantlib",
        &["6M"],
        S16B,
    ),
    (
        "golden/quantlib/fx_barrier_option/eurusd_up_in_call_rebate_at_expiry_3m_quantlib",
        &["1D", "1M"],
        S16B,
    ),
    (
        "golden/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib",
        &["1D", "1M"],
        S16B,
    ),
    (
        "golden/quantlib/fx_barrier_option/eurusd_up_out_call_rebate_at_hit_3m_quantlib",
        &["1D", "1M"],
        S16B,
    ),
    ("registry/asian_option", &["6M"], S16B),
    ("registry/autocallable", &["6M"], S16B),
    ("registry/cliquet_option", &["6M"], S16B),
    ("registry/fx_barrier_option", &["1D", "1M", "6M"], S16B),
    ("registry/fx_touch_option", &["1D", "1M", "6M"], S16B),
    ("registry/trs_equity", &["1M", "6M"], HELD),
    ("registry/trs_fixed_income_index", &["1M", "6M"], HELD),
    ("registry/range_accrual", &["6M"], HELD),
    // Not a fixing: the rolled trade has no schedule, needs lifecycle inputs,
    // or leaves its market grid.
    (
        "golden/regression_goldens/bond_future/ust_ty_10y_front_month",
        &["1D", "1M", "6M"],
        NO_SCHEDULE,
    ),
    ("registry/bond_future", &["1D", "1M", "6M"], NO_SCHEDULE),
    ("registry/agency_tba", &["1D", "1M", "6M"], NO_SCHEDULE),
    (
        "golden/regression_goldens/equity_future/spx_es_3m",
        &["6M"],
        "the 6M roll passes last_trading_date; the rolled future requires a settlement_price",
    ),
    (
        "registry/levered_real_estate_equity",
        &["1D", "1M", "6M"],
        "period cash requires purchase_price, which the registry example omits",
    ),
    (
        "golden/quantlib/swaption/usd_bachelier_1y1y_payer_swaption_quantlib",
        &["6M"],
        SWAPTION_GRID,
    ),
    (
        "golden/quantlib/swaption/usd_black_1y1y_payer_swaption_quantlib",
        &["6M"],
        SWAPTION_GRID,
    ),
    (
        "registry/revolving_credit",
        &["6M"],
        "a scheduled draw/repay event falls before the rolled simulation anchor",
    ),
];

const S16B: &str = "instrument-held observation state (barrier breach, Asian/autocall/cliquet fixings) inside the roll; rolled by Instrument::theta_observed_state (S16b, a1101cd59 on fix/wasm-binding-audit)";
const HELD: &str = "instrument-held period state (TRS period-start level, range-accrual in-range count) is not rolled; needs a theta_observed_state extension";
const NO_SCHEDULE: &str =
    "physically settled forward has no standalone cashflow schedule for period cash";
const SWAPTION_GRID: &str =
    "the rolled expiry (0.4986y) falls below the vol surface's 0.5y expiry grid";

struct Case {
    name: String,
    instrument: String,
    market: MarketContext,
    as_of: String,
    model: String,
}

fn golden_cases() -> Vec<Case> {
    collect_fixture_paths_under("pricing")
        .expect("pricing fixtures")
        .into_iter()
        .map(|path| {
            let raw = std::fs::read_to_string(&path).expect("read fixture");
            let fixture: GoldenFixture = serde_json::from_str(&raw).expect("parse fixture");
            let pricing = fixture.pricing().expect("pricing body");
            let name = path
                .to_string_lossy()
                .split("data/pricing/")
                .last()
                .unwrap_or_default()
                .trim_end_matches(".json")
                .to_string();
            Case {
                market: resolve_market(&pricing.market)
                    .unwrap_or_else(|e| panic!("{name}: market: {e}")),
                instrument: pricing.instrument.to_string(),
                as_of: fixture.metadata.valuation_date.clone(),
                model: pricing.model.clone(),
                name: format!("golden/{name}"),
            }
        })
        .collect()
}

fn registry_cases() -> Vec<Case> {
    let cases = pricing_cases();
    cases["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| {
            let (instrument, market) = pricing_case_inputs(case);
            let request = &case["request"];
            Case {
                name: format!("registry/{}", case["type"].as_str().expect("type")),
                instrument: instrument.to_string(),
                market,
                as_of: request["asOf"].as_str().expect("asOf").to_string(),
                model: request["model"].as_str().expect("model").to_string(),
            }
        })
        .collect()
}

fn price(case: &Case, metrics: &[String], overrides: Option<&str>) -> Result<Option<f64>, String> {
    let instrument = parse_boxed_instrument_from_json(&case.instrument, overrides)
        .map_err(|e| format!("parse: {e}"))?;
    let result = price_instrument(
        &instrument,
        &case.market,
        &case.as_of,
        &case.model,
        metrics,
        None,
        PricingOptions::default()
            .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::new())),
    )
    .map_err(|e| e.to_string())?;
    Ok(result.measures.get("theta").copied())
}

#[test]
fn theta_is_finite_at_1d_1m_6m_for_every_fixture() {
    let theta = ["theta".to_string()];
    let mut report = Vec::new();
    let mut unexpected = Vec::new();
    for case in golden_cases().into_iter().chain(registry_cases()) {
        if let Err(error) = price(&case, &[], None) {
            report.push(format!("{}\t-\tbase-error\t{error}", case.name));
            continue;
        }
        for (label, period) in HORIZONS {
            let overrides = format!(r#"{{"theta_period":{period}}}"#);
            let outcome = match price(&case, &theta, Some(&overrides)) {
                Ok(Some(value)) if value.is_finite() => Ok(value),
                Ok(Some(value)) => Err(format!("non-finite theta {value}")),
                Ok(None) => Err("theta missing from result".to_string()),
                Err(error) => Err(error),
            };
            let expected = EXPECTED_FAILURES
                .iter()
                .any(|(name, horizons, _)| *name == case.name && horizons.contains(&label));
            match &outcome {
                Ok(value) => {
                    report.push(format!("{}\t{label}\tok\t{value}", case.name));
                    if expected {
                        unexpected.push(format!("{} {label}: stale expected failure", case.name));
                    }
                }
                Err(error) => {
                    report.push(format!("{}\t{label}\terror\t{error}", case.name));
                    if !expected {
                        unexpected.push(format!("{} {label}: {error}", case.name));
                    }
                }
            }
        }
    }
    if let Ok(path) = std::env::var("THETA_SURVEY_OUT") {
        std::fs::write(&path, report.join("\n") + "\n").expect("write theta survey");
    }
    assert!(
        unexpected.is_empty(),
        "{} unexpected theta outcomes:\n{}",
        unexpected.len(),
        unexpected.join("\n")
    );
}
