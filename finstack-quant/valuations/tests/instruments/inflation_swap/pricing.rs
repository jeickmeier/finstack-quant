//! Core pricing methodology tests for InflationSwap.
//!
//! Tests the mathematical correctness of:
//! - Fixed leg PV calculation
//! - Inflation leg PV calculation
//! - Par rate (breakeven) computation
//! - Net PV calculation with correct sign conventions

use crate::inflation_swap::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwapBuilder;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_par_rate_gives_zero_pv() {
    // At-market swap (fixed rate = par rate) should have PV ≈ 0
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    // First, compute par rate
    let temp_swap = InflationSwapBuilder::new()
        .id("ZCINF-PAR".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal")) // doesn't matter for par_rate calculation
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let par_rate = temp_swap.par_rate(&ctx, as_of).unwrap();
    assert!(
        par_rate > 0.0 && par_rate < 0.1,
        "Par rate should be reasonable"
    );

    // Now price swap with par rate
    let par_swap = InflationSwapBuilder::new()
        .id("ZCINF-PAR2".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(par_rate).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv = par_swap.value(&ctx, as_of).unwrap();
    assert!(
        pv.amount().abs() < pv_tolerance(standard_notional()),
        "PV should be near zero for par swap, got: {}",
        pv.amount()
    );
}

#[test]
fn test_fixed_leg_pv_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-FL1".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let swap2 = InflationSwapBuilder::new()
        .id("ZCINF-FL2".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv_fixed1 = swap1.pv_fixed_leg(&ctx, as_of).unwrap().amount();
    let pv_fixed2 = swap2.pv_fixed_leg(&ctx, as_of).unwrap().amount();

    let ratio = pv_fixed2 / pv_fixed1;
    assert!(
        (ratio - 10.0).abs() < 1e-6,
        "Fixed leg PV should scale linearly with notional, ratio: {}",
        ratio
    );
}

#[test]
fn test_inflation_leg_pv_scales_with_notional() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap1 = InflationSwapBuilder::new()
        .id("ZCINF-IL1".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let swap2 = InflationSwapBuilder::new()
        .id("ZCINF-IL2".into())
        .notional(Money::new(5_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv_infl1 = swap1.pv_inflation_leg(&ctx, as_of).unwrap().amount();
    let pv_infl2 = swap2.pv_inflation_leg(&ctx, as_of).unwrap().amount();

    let ratio = pv_infl2 / pv_infl1;
    assert!(
        (ratio - 5.0).abs() < 1e-6,
        "Inflation leg PV should scale linearly with notional, ratio: {}",
        ratio
    );
}

#[test]
fn test_pv_sign_convention_pay_fixed() {
    // Pay: receive inflation leg, pay fixed leg
    // If inflation > fixed: PV should be positive
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.03, 0.04); // 3% inflation

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-SIGN".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.01).expect("valid decimal")) // 1% real rate, below 3% inflation
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv = swap.value(&ctx, as_of).unwrap().amount();
    assert!(pv > 0.0, "Pay with low fixed rate should have positive PV");
}

#[test]
fn test_pv_sign_convention_receive_fixed() {
    // Receive: pay inflation leg, receive fixed leg
    // If fixed > inflation: PV should be positive
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.01, 0.04); // 1% inflation

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-SIGN2".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.03).expect("valid decimal")) // 3% real rate, above 1% inflation
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv = swap.value(&ctx, as_of).unwrap().amount();
    assert!(
        pv > 0.0,
        "Receive with high fixed rate should have positive PV"
    );
}

#[test]
fn test_par_rate_increases_with_inflation_expectations() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx_low_infl = standard_market(as_of, 0.01, 0.04);
    let ctx_high_infl = standard_market(as_of, 0.03, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-PAR3".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let par_low = swap.par_rate(&ctx_low_infl, as_of).unwrap();
    let par_high = swap.par_rate(&ctx_high_infl, as_of).unwrap();

    assert!(
        par_high > par_low,
        "Par rate should increase with inflation expectations: {} vs {}",
        par_high,
        par_low
    );
}

#[test]
fn test_fixed_leg_increases_with_maturity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut pvs = Vec::new();
    for years in &[1, 2, 5, 10] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-MAT".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        let pv_fixed = swap.pv_fixed_leg(&ctx, as_of).unwrap().amount();
        pvs.push(pv_fixed);
    }

    // Fixed leg PV should increase with maturity (compounding effect)
    for i in 1..pvs.len() {
        assert!(
            pvs[i] > pvs[i - 1],
            "Fixed leg PV should increase with maturity"
        );
    }
}

#[test]
fn test_inflation_leg_increases_with_maturity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut pvs = Vec::new();
    for years in &[1, 2, 5, 10] {
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-MAT2".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        let pv_infl = swap.pv_inflation_leg(&ctx, as_of).unwrap().amount();
        pvs.push(pv_infl);
    }

    // Inflation leg PV should increase with maturity
    for i in 1..pvs.len() {
        assert!(
            pvs[i] > pvs[i - 1],
            "Inflation leg PV should increase with maturity"
        );
    }
}

#[test]
fn test_par_rate_formula_consistency() {
    // Verify: K_par = (E[I(T)/I(0)])^(1/τ) - 1
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.025, 0.04); // 2.5% inflation

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-FORM".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let par_rate = swap.par_rate(&ctx, as_of).unwrap();

    // For flat 2.5% inflation, par should be close to 2.5%
    // (exact match depends on lag and interpolation)
    assert!(
        (par_rate - 0.025).abs() < 0.01,
        "Par rate should be close to inflation rate: {}",
        par_rate
    );
}

#[test]
fn test_npv_equals_leg_difference() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-NPV".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.015).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let npv = swap.value(&ctx, as_of).unwrap().amount();
    let pv_fixed = swap.pv_fixed_leg(&ctx, as_of).unwrap().amount();
    let pv_infl = swap.pv_inflation_leg(&ctx, as_of).unwrap().amount();

    // Pay: NPV = PV(infl) - PV(fixed)
    let expected_npv = pv_infl - pv_fixed;
    assert!(
        (npv - expected_npv).abs() < 1e-6,
        "NPV should equal leg difference: {} vs {}",
        npv,
        expected_npv
    );
}

#[test]
fn test_realistic_market_pricing() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2035, Month::January, 1).unwrap();

    let ctx = realistic_market(as_of);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-REAL".into())
        .notional(large_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal")) // 2% real rate
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv = swap.value(&ctx, as_of).unwrap();

    // Should produce finite, reasonable PV
    assert!(pv.amount().is_finite());
    assert!(pv.amount().abs() < large_notional().amount());
}

#[test]
fn test_day_count_impact_on_fixed_leg() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let mut pvs = finstack_quant_core::HashMap::default();
    for dc in &[DayCount::Act360, DayCount::Act365F, DayCount::Thirty360] {
        let swap = InflationSwapBuilder::new()
            .id("ZCINF-DC".into())
            .notional(standard_notional())
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id("US-CPI-U".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(*dc)
            .side(PayReceive::Pay)
            .attributes(Attributes::new())
            .build()
            .unwrap();

        let pv_fixed = swap.pv_fixed_leg(&ctx, as_of).unwrap().amount();
        pvs.insert(format!("{:?}", dc), pv_fixed);
    }

    // Different day count conventions should yield different PVs
    let pv_360 = pvs.get("Act360").unwrap();
    let pv_365 = pvs.get("Act365F").unwrap();
    let pv_30360 = pvs.get("Thirty360").unwrap();

    assert_ne!(pv_360, pv_365, "Act360 and Act365F should differ");
    assert_ne!(pv_365, pv_30360, "Act365F and 30/360 should differ");
}

#[test]
fn test_zero_fixed_rate() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let ctx = standard_market(as_of, 0.02, 0.04);

    let swap = InflationSwapBuilder::new()
        .id("ZCINF-ZERO".into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.0).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv_fixed = swap.pv_fixed_leg(&ctx, as_of).unwrap().amount();

    // With 0% fixed rate: (1 + 0)^τ - 1 = 0, so PV(fixed) should be ~0
    assert!(
        pv_fixed.abs() < 1e-6,
        "Zero fixed rate should give zero fixed leg PV"
    );
}
