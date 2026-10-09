//! Public-surface regression for the `cdx_ig_46_payer_atm_jun26` Bloomberg
//! CDSO golden: supplied-curve NPV and a zero-bump hazard rebootstrap.

#![allow(clippy::expect_used)]

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::recalibration::QuoteBump;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use time::macros::date;

const FIXTURE: &str =
    "tests/golden/data/pricing/bloomberg/cds_option/cdx_ig_46_payer_atm_jun26.json";
const BBG_NPV: f64 = 118_781.76;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn load_fixture_json() -> Value {
    let raw = fs::read_to_string(fixture_path()).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

fn bootstrap_market(fixture: &Value) -> MarketContext {
    let envelope: CalibrationEnvelope =
        serde_json::from_value(fixture["market"]["envelope"].clone()).expect("parse envelope");
    let result = engine::calibrate(&envelope).expect("calibrate");
    MarketContext::try_from(result.result.final_market).expect("rehydrate market")
}

fn load_option(fixture: &Value) -> CdsOption {
    serde_json::from_value(fixture["instrument"]["instrument"]["spec"].clone())
        .expect("parse cds option spec")
}

/// Library NPV on the supplied curve. Bloomberg CDSO shows 118,781.76: an open
/// reconciliation of 431.15 (0.36%), not a tolerance.
///
/// What is established (see `docs/audits/2026-09-09-cdso-reconciliation.md`,
/// 2026-10-08 section): CDSO prices the index on a flat curve at the index
/// quote (53.6264 bp), which the fixture now supplies; the earlier 6,722 gap
/// came from a term structure transcribed from another screen. None of the
/// candidate conventions in the `cdx_ig_46_convention_matrix` diagnostic
/// (variance clock, discount date, first-coupon annuity, separate front-end
/// protection) reproduces NPV, forward, delta, vega and theta together.
/// Closing the rest needs payer and receiver CDSO prices at a strike equal to
/// the coupon and at two other strikes, plus the CDSO curve tab.
const LIBRARY_NPV: f64 = 118_350.607_245_74;

/// Pins the library value so an unintended pricing change is caught, and pins
/// the size of the open Bloomberg difference so closing it is a deliberate,
/// reviewed update of this test rather than a silent drift.
#[test]
fn cdx_ig_46_npv_is_pinned_with_the_open_bloomberg_difference() {
    let fixture = load_fixture_json();
    let market = bootstrap_market(&fixture);
    let option = load_option(&fixture);
    let supplied_pv = option
        .value(&market, date!(2026 - 05 - 07))
        .expect("supplied market npv")
        .amount();

    assert!(
        (supplied_pv - LIBRARY_NPV).abs() < 1e-4,
        "CDX IG 46 CDSO NPV moved: got {supplied_pv}, pinned {LIBRARY_NPV}",
    );
    let open_difference = BBG_NPV - supplied_pv;
    assert!(
        (open_difference - 431.15).abs() < 0.01,
        "open Bloomberg difference changed: {open_difference} (target {BBG_NPV})",
    );
}

#[test]
fn cdx_ig_46_zero_rebootstrap_preserves_supplied_curve_npv() {
    let fixture = load_fixture_json();
    let as_of = date!(2026 - 05 - 07);
    let market = bootstrap_market(&fixture);
    let option = load_option(&fixture);
    let supplied_pv = option
        .value(&market, as_of)
        .expect("supplied market npv")
        .amount();

    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let zero_hazard = bump_hazard_spreads(
        hazard.as_ref(),
        &market,
        &QuoteBump::ParallelBp(0.0),
        Some(&option.discount_curve_id),
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("zero-bump hazard rebootstrap");
    let zero_market = market.insert(zero_hazard);
    let zero_pv = option
        .value(&zero_market, as_of)
        .expect("zero-bump market npv")
        .amount();

    // A zero-bump rebootstrap must preserve the supplied curve.
    assert!(
        (zero_pv - supplied_pv).abs() < 1e-6,
        "zero-bump rebootstrap should reproduce the supplied-curve NPV: supplied={supplied_pv}, zero={zero_pv}",
    );
}
