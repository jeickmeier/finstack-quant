//! Linked CDSW spot trade on the same market as the CDX IG 46 CDSO golden.

use super::common::cdx_ig_46_fixture;
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use finstack_quant_valuations::recalibration::QuoteBump;
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn cdx_ig_46_linked_spot_cds_reconciliation() {
    let as_of = date!(2026 - 05 - 07);
    let settlement = date!(2026 - 05 - 12);
    let (option, market) = cdx_ig_46_fixture();
    let cds = synthetic_underlying_cds(&option, as_of).expect("linked CDS");
    let discount = market
        .get_discount(&option.discount_curve_id)
        .expect("S531");
    let df = discount
        .df_between_dates(as_of, settlement)
        .expect("spot settlement DF");
    let clean_pv = cds.value_raw(&market, as_of).expect("spot CDS clean PV");
    let accrual = cds.clean_quote_accrual(as_of).expect("clean accrued");
    let accrued_cash =
        -cds.notional.amount() * cds.premium_leg.coupon_bp.as_f64() / 10_000.0 * accrual;
    let dirty_pv = clean_pv + accrued_cash;
    let invoice = dirty_pv / df;
    let principal = invoice - accrued_cash;
    eprintln!(
        "May 12 buyer cash: principal {principal:.2}, accrued {accrued_cash:.2}, invoice {invoice:.2}; CDSW shows -2137736, -136111, -2273847"
    );
    assert_eq!(accrual, 49.0 / 360.0);
    assert_eq!(accrued_cash.round(), -136_111.0);

    let hazard = market
        .get_hazard(&option.credit_curve_id)
        .expect("index hazard");
    let pd = 1.0
        - hazard
            .sp_on_date(cds.premium_leg.end)
            .expect("maturity survival");
    assert_eq!((pd * 10_000.0).round(), 452.0, "CDSW displays 0.0452");
    let bumped = bump_hazard_spreads(
        hazard.as_ref(),
        &market,
        &QuoteBump::ParallelBp(1.0),
        &option.discount_curve_id,
        None,
        None,
    )
    .expect("replay index quote");
    let bumped_market = market
        .clone()
        .try_insert(bumped)
        .expect("insert bumped hazard");
    let spread_dv01 = cds.value_raw(&bumped_market, as_of).expect("bumped CDS PV") - clean_pv;
    assert!(
        (spread_dv01 / 46_963.21 - 1.0).abs() < 0.01,
        "CDSW spread DV01 {spread_dv01}"
    );

    // Clean par quote yields zero clean principal at the quoted breakeven spread.
    let mut par_cds = cds;
    par_cds.premium_leg.coupon_bp = Decimal::new(536_264, 4);
    let par_pv = par_cds.value_raw(&market, as_of).expect("par CDS PV");
    assert!(
        par_pv.abs() < 0.01,
        "clean par PV should be near zero at 53.6264 bp, got {par_pv}"
    );
}
