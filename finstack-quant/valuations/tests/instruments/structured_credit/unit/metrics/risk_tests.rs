//! Unit tests for risk metrics (Duration, Z-spread, CS01, YTM).
//!
//! Tests cover:
//! - Duration calculations from dated cashflows
//! - Z-spread solver convergence
//! - CS01 price sensitivity

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_cs01, calculate_tranche_duration, calculate_tranche_z_spread,
};
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn flat_discount_curve(rate: f64) -> DiscountCurve {
    let base = base_date();
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
        ])
        .build()
        .unwrap()
}

fn sample_cashflows() -> Vec<(Date, Money)> {
    vec![
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::new(60_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::new(40_000.0, Currency::USD),
        ),
    ]
}

#[test]
fn test_tranche_duration_is_true_modified_duration() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let day_count = DayCount::Act365F;
    let shift = 1e-4; // 1bp, matching the calculator
    let mut pv = 0.0;
    let mut shifted_pv = 0.0;
    let mut weighted_pv = 0.0;

    for (date, amount) in &flows {
        let t = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .unwrap();
        let df = curve.df_between_dates(as_of, *date).unwrap();
        let flow_pv = amount.amount() * df;
        pv += flow_pv;
        shifted_pv += flow_pv * (-shift * t).exp();
        weighted_pv += flow_pv * t;
    }

    // True modified duration: -(dP/dy)/P from a 1bp continuous-compounding
    // bump. For continuous compounding this is the Macaulay measure only to
    // first order; the calculator must reproduce the bump value exactly.
    let expected_duration = -(shifted_pv - pv) / (pv * shift);
    let macaulay = weighted_pv / pv;
    let duration =
        calculate_tranche_duration(&flows, &curve, as_of, Money::new(pv, Currency::USD)).unwrap();

    assert!(
        (duration - expected_duration).abs() < 1e-10,
        "Duration must equal the 1bp-bump modified duration; got {duration}, expected {expected_duration}"
    );
    // Sanity: for a 1bp continuous bump the modified duration is close to but
    // not identical to Macaulay (second-order convexity effect).
    assert!(
        (duration - macaulay).abs() < 1e-3,
        "modified duration {duration} should be near Macaulay {macaulay}"
    );
}

#[test]
fn test_z_spread_zero_for_curve_pv() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let mut pv = 0.0;
    for (date, amount) in &flows {
        let df = curve.df_between_dates(as_of, *date).unwrap();
        pv += amount.amount() * df;
    }

    let z_spread_bps =
        calculate_tranche_z_spread(&flows, &curve, Money::new(pv, Currency::USD), as_of).unwrap();

    assert!(
        z_spread_bps.abs() < 0.1,
        "Z-spread should be near 0 for curve-implied PV"
    );
}

#[test]
fn test_cs01_negative_for_long_position() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let cs01 = calculate_tranche_cs01(&flows, &curve, 0.0, as_of).unwrap();
    assert!(
        cs01 < 0.0,
        "CS01 should be negative for a long position (wider spreads reduce PV), got {}",
        cs01
    );
}
