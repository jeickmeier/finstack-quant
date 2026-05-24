//! Bucketed DV01 metric tests for InflationSwap.

use crate::inflation_swap::fixtures::*;
use finstack_core::dates::{Date, DayCount};
use finstack_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_bucketed_dv01_computed() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BDV01-1".into())
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
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check that bucketed_dv01 is present
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "Bucketed DV01 should be computed"
    );

    let bucketed_dv01 = *result.measures.get("bucketed_dv01").unwrap();

    eprintln!(
        "test_bucketed_dv01_computed: bucketed_dv01 = {}",
        bucketed_dv01
    );

    // Bucketed DV01 should be finite
    assert!(bucketed_dv01.is_finite(), "Bucketed DV01 should be finite");
}

#[test]
fn test_bucketed_dv01_reasonable_magnitude() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BDV01-MAG".into())
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
            &[MetricId::BucketedDv01, MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let bucketed_dv01_raw = *result.measures.get("bucketed_dv01").unwrap();
    let dv01_raw = *result.measures.get("dv01").unwrap();

    let bucketed_dv01 = bucketed_dv01_raw.abs();
    let dv01 = dv01_raw.abs();

    // Note: Bucketed DV01 for inflation swaps currently returns 0 because the inflation swap
    // pricer doesn't respond to discount curve bumps in the current implementation.
    // The analytical DV01 works because it uses a different calculation method.
    // This is a known limitation and doesn't affect other instrument types.
    // TODO: Fix inflation swap bucketed DV01 to properly respond to discount curve bumps

    // For now, just verify DV01 is positive
    assert!(
        dv01 > 0.0,
        "Analytical DV01 should be positive, got {}",
        dv01_raw
    );

    // Bucketed DV01 should now work for inflation swaps with unified calculator
    assert!(
        bucketed_dv01.abs() > 0.0,
        "Bucketed DV01 should be non-zero for inflation swaps with unified calculator, got {}",
        bucketed_dv01
    );

    // Bucketed DV01 should be reasonable in magnitude
    assert!(
        bucketed_dv01.abs() < 10.0,
        "Bucketed DV01 magnitude should be reasonable, got {}",
        bucketed_dv01
    );
}

#[test]
fn test_bucketed_dv01_zero_for_matured() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2020, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-BDV01-MAT0".into())
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
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let bucketed_dv01 = result.measures.get("bucketed_dv01").unwrap().abs();

    // Matured swap should have negligible bucketed DV01
    assert!(
        bucketed_dv01 < 10.0,
        "Matured swap should have negligible bucketed DV01: {}",
        bucketed_dv01
    );
}
