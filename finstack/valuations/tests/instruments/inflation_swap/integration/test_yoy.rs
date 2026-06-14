//! Integration tests for `YoYInflationSwap`.
//!
//! The rest of the inflation_swap slice covers only the zero-coupon
//! `InflationSwap`; the year-on-year swap previously had no integration
//! coverage. These tests exercise end-to-end pricing, the pay/receive sign
//! mirror, notional linearity, the par-rate roundtrip (par fixed rate ⇒ zero
//! PV), and the Inflation01 sensitivity.

use crate::inflation_swap::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::InflationLag;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::inflation_swap::YoYInflationSwap;
use finstack_valuations::instruments::{Attributes, Instrument, PayReceive};
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn build_yoy(side: PayReceive, fixed_rate: f64, notional: f64) -> YoYInflationSwap {
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 15).unwrap();
    YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-TEST"))
        .notional(Money::new(notional, Currency::USD))
        .start_date(start)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(fixed_rate).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(side)
        .lag_override(InflationLag::Months(3))
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .attributes(Attributes::new())
        .build()
        .expect("valid YoY swap")
}

fn market(as_of: Date) -> MarketContext {
    // 2.5% inflation, 2% nominal discount.
    standard_market(as_of, 0.025, 0.02)
}

#[test]
fn test_yoy_prices_to_finite_value() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let swap = build_yoy(PayReceive::Pay, 0.025, 1_000_000.0);
    let pv = swap
        .value(&market(as_of), as_of)
        .expect("YoY swap should price");

    assert_eq!(pv.currency(), Currency::USD);
    assert!(
        pv.amount().is_finite(),
        "YoY PV should be finite, got {}",
        pv.amount()
    );
}

#[test]
fn test_yoy_pay_receive_sign_symmetry() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = market(as_of);

    let pay = build_yoy(PayReceive::Pay, 0.02, 1_000_000.0)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let receive = build_yoy(PayReceive::Receive, 0.02, 1_000_000.0)
        .value(&market, as_of)
        .unwrap()
        .amount();

    // Pay-fixed and receive-fixed at the same terms are mirror positions.
    assert!(
        (pay + receive).abs() <= 1e-6 + 1e-6 * pay.abs().max(1.0),
        "pay-fixed and receive-fixed YoY PVs should be opposite: pay={pay}, receive={receive}"
    );
    assert!(
        pay.abs() > 1.0,
        "test setup should produce a non-trivial off-par PV, got {pay}"
    );
}

#[test]
fn test_yoy_pv_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = market(as_of);

    let pv_1m = build_yoy(PayReceive::Pay, 0.02, 1_000_000.0)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let pv_2m = build_yoy(PayReceive::Pay, 0.02, 2_000_000.0)
        .value(&market, as_of)
        .unwrap()
        .amount();

    assert!(
        (pv_2m - 2.0 * pv_1m).abs() <= 1e-6 * pv_1m.abs().max(1.0),
        "YoY PV should scale linearly with notional: 1M={pv_1m}, 2M={pv_2m}"
    );
}

#[test]
fn test_yoy_par_rate_zeros_the_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = market(as_of);

    // A vanilla YoY swap's PV is linear in the fixed rate (the fixed leg is
    // rate × annuity), so we can solve the par rate by interpolating two PVs.
    let pv_at = |rate: f64| {
        build_yoy(PayReceive::Pay, rate, 1_000_000.0)
            .value(&market, as_of)
            .unwrap()
            .amount()
    };
    let (r1, r2) = (0.01, 0.04);
    let (pv1, pv2) = (pv_at(r1), pv_at(r2));
    let par_rate = r1 - pv1 * (r2 - r1) / (pv2 - pv1);

    assert!(
        par_rate.is_finite() && par_rate > 0.0,
        "interpolated par rate should be sensible and positive, got {par_rate}"
    );

    // Rebuilt at its par rate, the swap must price to (approximately) zero —
    // which also confirms PV is genuinely linear in the fixed rate.
    let pv_at_par = pv_at(par_rate);
    assert!(
        pv_at_par.abs() < 1.0,
        "YoY swap struck at its par rate ({par_rate}) should have ~zero PV, got {pv_at_par}"
    );
}

#[test]
fn test_yoy_inflation01_is_finite_and_nonzero() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let swap = build_yoy(PayReceive::Pay, 0.02, 1_000_000.0);

    let inflation01 = *swap
        .price_with_metrics(
            &market(as_of),
            as_of,
            &[MetricId::Inflation01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("Inflation01 should compute")
        .measures
        .get("inflation01")
        .expect("inflation01 measure");

    assert!(
        inflation01.is_finite() && inflation01.abs() > 0.0,
        "YoY Inflation01 should be finite and non-zero, got {inflation01}"
    );
}
