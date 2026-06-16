//! Tests for inflation index lag policies.

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_lag_override_vs_index_lag() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    // Index with 3M lag
    let disc = flat_discount("USD-OIS", as_of, 0.04).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02).unwrap();
    let index = simple_index(
        "US-CPI-U",
        as_of,
        300.0,
        Currency::USD,
        InflationLag::Months(3),
    );

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index);

    // Swap with no lag override (uses index 3M lag)
    let swap_idx_lag = InflationSwapBuilder::new()
        .id("ZCINF-LAG-IDX".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Swap with 0 lag override
    let swap_no_lag = InflationSwapBuilder::new()
        .id("ZCINF-LAG-0".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag_override(InflationLag::None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let pv_idx_lag = swap_idx_lag.value(&ctx, as_of).unwrap().amount();
    let pv_no_lag = swap_no_lag.value(&ctx, as_of).unwrap().amount();

    // With positive inflation, longer lag reduces projected CPI at maturity
    // For Pay (receiving inflation), lower CPI means lower PV
    assert!(
        pv_no_lag >= pv_idx_lag,
        "No lag should give higher/equal PV than 3M lag for Pay: {} vs {}",
        pv_no_lag,
        pv_idx_lag
    );
}

#[test]
fn test_different_lag_durations() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let disc = flat_discount("USD-OIS", as_of, 0.04).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.03).unwrap();
    let index = simple_index("US-CPI-U", as_of, 300.0, Currency::USD, InflationLag::None);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index);

    let mut pvs = Vec::new();
    for lag_months in &[0, 1, 2, 3, 6] {
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-LAG-VAR".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(if *lag_months == 0 {
                InflationLag::None
            } else {
                InflationLag::Months(*lag_months)
            })
            .attributes(Default::default())
            .build()
            .unwrap();

        let pv = swap.value(&ctx, as_of).unwrap().amount();
        pvs.push(pv);
    }

    // With positive inflation, PV should decrease as lag increases
    for i in 1..pvs.len() {
        assert!(
            pvs[i] <= pvs[i - 1],
            "PV should decrease with longer lag (positive inflation)"
        );
    }
}

#[test]
fn test_lag_in_days_vs_months() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let disc = flat_discount("USD-OIS", as_of, 0.04).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02).unwrap();
    let index = simple_index("US-CPI-U", as_of, 300.0, Currency::USD, InflationLag::None);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index);

    // 3 months ≈ 90 days
    let swap_3m = InflationSwapBuilder::new()
        .id("ZCINF-LAG-3M".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag_override(InflationLag::Months(3))
        .attributes(Default::default())
        .build()
        .unwrap();

    let swap_90d = InflationSwapBuilder::new()
        .id("ZCINF-LAG-90D".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag_override(InflationLag::Days(90))
        .attributes(Default::default())
        .build()
        .unwrap();

    let pv_3m = swap_3m.value(&ctx, as_of).unwrap().amount();
    let pv_90d = swap_90d.value(&ctx, as_of).unwrap().amount();

    // Should be close but not necessarily identical
    let rel_diff = (pv_3m - pv_90d).abs() / pv_3m.abs().max(1.0);
    assert!(
        rel_diff < 0.05,
        "3M and 90D lag should give similar PV: {} vs {}",
        pv_3m,
        pv_90d
    );
}

#[test]
fn test_no_index_fallback_to_curve() {
    // Test that swap works even without inflation index (uses curve directly)
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let disc = flat_discount("USD-OIS", as_of, 0.04).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02).unwrap();

    // Context without inflation index
    let ctx = MarketContext::new().insert(disc).insert(infl_curve);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-NO-IDX".into())
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

    let pv = swap.value(&ctx, as_of).unwrap();

    // Should work and produce reasonable PV
    assert!(pv.amount().is_finite());
}
