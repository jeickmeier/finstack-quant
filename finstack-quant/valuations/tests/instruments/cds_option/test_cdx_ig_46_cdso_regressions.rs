//! Public-surface regression for the `cdx_ig_46_payer_atm_jun26` Bloomberg
//! CDSO golden: supplied-curve NPV and a zero-bump hazard rebootstrap.

#![allow(clippy::expect_used)]

use super::common::cdx_ig_46_fixture;
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::CdsValuationConvention;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::recalibration::QuoteBump;
use time::macros::date;

/// Internal regression on the single-maturity quote. The external comparison
/// runs separately in the golden runner with the captured USD 6 tolerance.
#[test]
fn cdx_ig_46_npv_on_supplied_curve() {
    let (option, market) = cdx_ig_46_fixture();
    let pv = option
        .value(&market, date!(2026 - 05 - 07))
        .expect("price option")
        .amount();
    assert!((pv - 118_354.377_111_254_22).abs() < 1e-4, "CDSO NPV {pv}");
}

#[test]
fn cdx_ig_46_zero_rebootstrap_preserves_supplied_curve_npv() {
    let as_of = date!(2026 - 05 - 07);
    let (option, market) = cdx_ig_46_fixture();
    let supplied_pv = option
        .value(&market, as_of)
        .expect("supplied market npv")
        .amount();

    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let zero_hazard = bump_hazard_spreads(
        hazard.as_ref(),
        &market,
        &QuoteBump::ParallelBp(0.0),
        &option.discount_curve_id,
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("zero-bump hazard rebootstrap");
    let zero_market = market.try_insert(zero_hazard).expect("insert market curve");
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
