//! Theta runs at 1D, 1M and 6M for every pricing golden and every registry
//! example (UI pricing case), one test per fixture family and horizon.
//!
//! The families follow the fixture directories, except that the
//! structured-credit and CDS-tranche goldens, which dominate the run time, are
//! carved out of `regression_goldens` into families of their own.
//!
//! Theta holds the market fixed and rolls the valuation date, so the rolled
//! repricing crosses every index fixing inside the horizon. A fixture whose
//! base valuation prices must also produce a finite theta at each horizon; the
//! only exceptions are listed in [`EXPECTED_FAILURES`] with their reason.
//!
//! Set `THETA_SURVEY_OUT=<prefix>` to write each test's survey as
//! `<prefix>.<family>.<horizon>`, one tab-separated line per run.

use super::pricing_common::resolve_market;
use super::schema::GoldenFixture;
use super::walk::collect_fixture_paths_under;
use crate::instruments::loan_facility_wire_keys::{pricing_case_inputs, pricing_cases};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};
use std::path::PathBuf;
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
    // Market-held observations the rolled repricing does not yet receive.
    (
        "registry/inflation_linked_bond",
        &["6M"],
        "the registry market deliberately has no CPI observations; the 6M roll crosses publication dates and theta does not synthesize published CPI history",
    ),
    (
        "golden/regression_goldens/inflation_linked_bond/inflation_linked_bond_5y",
        &["1D", "1M", "6M"],
        "theta does not synthesize monthly CPI publications; the rolled linker requires observations beyond the fixture's published history",
    ),
    (
        "golden/bloomberg/cap_floor/usd_cap_5y_atm_black",
        &["1M", "6M"],
        CAP_FIXINGS,
    ),
    (
        "golden/regression_goldens/structured_credit/abs_credit_card_senior",
        &["1M", "6M"],
        SC_FIXINGS,
    ),
    (
        "golden/regression_goldens/structured_credit/clo_mezzanine_base_case",
        &["1M", "6M"],
        SC_FIXINGS,
    ),
    ("registry/variance_swap", &["1M", "6M"], PRICE_SERIES),
    ("registry/fx_variance_swap", &["1M", "6M"], PRICE_SERIES),
    // Instrument-held state that `Instrument::theta_observed_state` does not roll.
    (
        "golden/quantlib/asian_option/spx_geometric_asian_call_1y_quantlib",
        &["6M"],
        "the rolled trade is a seasoned geometric Asian, which has no analytical pricer",
    ),
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

const CAP_FIXINGS: &str = "the CapFloor theta calculator reprices without the caplet fixings the horizon crosses; its schedule records no projected fixings";
const SC_FIXINGS: &str = "the structured-credit schedule records no projected fixings, so the SOFR-3M resets the roll crosses are not materialized";
const PRICE_SERIES: &str = "observed price series are not extended over the roll and variance swaps have no theta_observed_state";
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

/// Fixture paths under `relative_dir` of `tests/golden/data`.
fn golden_paths(relative_dir: &str) -> Vec<PathBuf> {
    collect_fixture_paths_under(relative_dir).expect("pricing fixtures")
}

/// The `regression_goldens` fixtures other than the structured-credit and
/// CDS-tranche families, which have tests of their own.
fn other_regression_golden_paths() -> Vec<PathBuf> {
    golden_paths("pricing/regression_goldens")
        .into_iter()
        .filter(|path| {
            let path = path.to_string_lossy();
            !path.contains("/structured_credit/") && !path.contains("/cds_tranche/")
        })
        .collect()
}

fn golden_cases(paths: Vec<PathBuf>) -> Vec<Case> {
    paths
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

/// Theta survey of one fixture family at one horizon.
///
/// Returns `(report, unexpected)`: one tab-separated survey line per run, and
/// the runs whose outcome contradicts [`EXPECTED_FAILURES`] (an unlisted
/// failure, or a listed run that succeeded). A fixture whose base valuation
/// fails is reported as `base-error` and not surveyed.
fn survey(cases: &[Case], (label, period): (&str, &str)) -> (Vec<String>, Vec<String>) {
    let theta = ["theta".to_string()];
    let overrides = format!(r#"{{"theta_period":{period}}}"#);
    let mut report = Vec::new();
    let mut unexpected = Vec::new();
    for case in cases {
        if let Err(error) = price(case, &[], None) {
            report.push(format!("{}\t-\tbase-error\t{error}", case.name));
            continue;
        }
        let outcome = match price(case, &theta, Some(&overrides)) {
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
    (report, unexpected)
}

/// Runs the survey for one family at one horizon, writes the optional survey
/// file and fails on any unexpected outcome.
fn assert_theta_finite(family: &str, cases: &[Case], horizon: (&str, &str)) {
    let (report, unexpected) = survey(cases, horizon);
    if let Ok(prefix) = std::env::var("THETA_SURVEY_OUT") {
        let path = format!("{prefix}.{family}.{}", horizon.0);
        std::fs::write(&path, report.join("\n") + "\n").expect("write theta survey");
    }
    assert!(
        unexpected.is_empty(),
        "{} unexpected theta outcomes:\n{}",
        unexpected.len(),
        unexpected.join("\n")
    );
}

/// The golden families are a partition of every fixture under `pricing/`:
/// a new fixture directory must be added to a family here, not silently
/// skipped.
#[test]
fn golden_families_cover_every_pricing_fixture() {
    let mut families: Vec<PathBuf> = [
        golden_paths("pricing/bloomberg"),
        golden_paths("pricing/quantlib"),
        golden_paths("pricing/regression_goldens/structured_credit"),
        golden_paths("pricing/regression_goldens/cds_tranche"),
        other_regression_golden_paths(),
    ]
    .into_iter()
    .flatten()
    .collect();
    families.sort();
    assert_eq!(
        families,
        golden_paths("pricing"),
        "theta families must cover every pricing fixture exactly once"
    );
}

/// Case sources, one per fixture family; the function name is the family
/// label in test names and survey files.
fn bloomberg() -> Vec<Case> {
    golden_cases(golden_paths("pricing/bloomberg"))
}

fn quantlib() -> Vec<Case> {
    golden_cases(golden_paths("pricing/quantlib"))
}

fn structured_credit() -> Vec<Case> {
    golden_cases(golden_paths("pricing/regression_goldens/structured_credit"))
}

fn cds_tranche() -> Vec<Case> {
    golden_cases(golden_paths("pricing/regression_goldens/cds_tranche"))
}

fn other_regression() -> Vec<Case> {
    golden_cases(other_regression_golden_paths())
}

fn registry() -> Vec<Case> {
    registry_cases()
}

/// One `#[test]` per fixture family and horizon:
/// `name => case source, index into [`HORIZONS`]`.
macro_rules! theta_tests {
    ($($test:ident => $family:ident, $horizon:expr;)*) => {$(
        #[test]
        fn $test() {
            assert_theta_finite(stringify!($family), &$family(), HORIZONS[$horizon]);
        }
    )*};
}

theta_tests! {
    theta_is_finite_1d_bloomberg_goldens => bloomberg, 0;
    theta_is_finite_1m_bloomberg_goldens => bloomberg, 1;
    theta_is_finite_6m_bloomberg_goldens => bloomberg, 2;
    theta_is_finite_1d_quantlib_goldens => quantlib, 0;
    theta_is_finite_1m_quantlib_goldens => quantlib, 1;
    theta_is_finite_6m_quantlib_goldens => quantlib, 2;
    theta_is_finite_1d_structured_credit_goldens => structured_credit, 0;
    theta_is_finite_1m_structured_credit_goldens => structured_credit, 1;
    theta_is_finite_6m_structured_credit_goldens => structured_credit, 2;
    theta_is_finite_1d_cds_tranche_goldens => cds_tranche, 0;
    theta_is_finite_1m_cds_tranche_goldens => cds_tranche, 1;
    theta_is_finite_6m_cds_tranche_goldens => cds_tranche, 2;
    theta_is_finite_1d_other_regression_goldens => other_regression, 0;
    theta_is_finite_1m_other_regression_goldens => other_regression, 1;
    theta_is_finite_6m_other_regression_goldens => other_regression, 2;
    theta_is_finite_1d_registry_examples => registry, 0;
    theta_is_finite_1m_registry_examples => registry, 1;
    theta_is_finite_6m_registry_examples => registry, 2;
}
