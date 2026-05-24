//! Theta (time decay) metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_core::dates::{Date, DayCount};
use finstack_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_theta_finite_difference_validation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-THETA-FD".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.025).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Get analytic theta
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta_analytic = *result.measures.get("theta").unwrap();

    // Compute finite difference theta
    let pv0 = swap.value(&ctx, as_of).unwrap().amount();
    let pv1 = swap
        .value(&ctx, as_of + time::Duration::days(1))
        .unwrap()
        .amount();

    let theta_fd = pv1 - pv0; // 1-day theta

    // Check sign consistency
    assert_eq!(
        theta_analytic.signum(),
        theta_fd.signum(),
        "Theta sign should match FD: analytic={}, FD={}",
        theta_analytic,
        theta_fd
    );
}

#[test]
fn test_theta_reasonable_magnitude() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-THETA-MAG".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta should be finite and reasonably small per day
    assert!(theta.is_finite(), "Theta should be finite");
    assert!(
        theta.abs() < standard_notional().amount() * 0.001,
        "Theta magnitude should be reasonable: {}",
        theta
    );
}

#[test]
fn test_theta_zero_for_matured_swap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2020, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-THETA-MAT0".into())
        .notional(standard_notional())
        .start_date(Date::from_calendar_date(2015, Month::January, 1).unwrap())
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = result.measures.get("theta").unwrap().abs();

    // Matured swap should have near-zero theta
    assert!(
        theta < 1.0,
        "Matured swap should have negligible theta: {}",
        theta
    );
}
