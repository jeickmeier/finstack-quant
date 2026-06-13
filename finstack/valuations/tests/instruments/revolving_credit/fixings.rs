//! Tests for historical fixing support in the revolving credit cashflow engine.
//!
//! Verifies that seasoned floating-rate facilities use observed fixing rates
//! for past reset dates instead of projecting from the forward curve.

use finstack_cashflows::builder::FloatingRateSpec;
use finstack_core::currency::Currency;
use finstack_core::dates::{DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use time::macros::date;

/// Build a flat discount curve for testing.
fn build_flat_discount_curve(rate: f64, base_date: time::Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, (-rate).exp()), (5.0, (-rate * 5.0).exp())])
        .build()
        .unwrap()
}

/// Build a flat forward curve for testing.
fn build_flat_forward_curve(
    rate: f64,
    base_date: time::Date,
    curve_id: &str,
    tenor_years: f64,
) -> ForwardCurve {
    ForwardCurve::builder(curve_id, tenor_years)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (1.0, rate), (5.0, rate)])
        .build()
        .unwrap()
}

/// Build a seasoned floating-rate facility.
///
/// commitment_date is in the past relative to as_of, so some reset dates
/// will fall before the valuation date.
fn build_seasoned_floating_facility(
    commitment_date: time::Date,
    maturity_date: time::Date,
) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-FIXING-TEST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Floating(FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"), // +200 bps
            gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: Some(rust_decimal::Decimal::try_from(0.0).expect("valid")), // 0% floor on index
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            dc: DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
            payment_lag_days: 0,
        }))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap()
}

/// When fixings are provided for past reset dates, the cashflow engine should
/// use the observed fixing rate (+ spread + floor) instead of the forward curve.
#[test]
fn test_seasoned_facility_uses_fixings_for_past_resets() {
    // Facility starts 2024-07-01, matures 2025-07-01.
    // Valuation date is 2025-01-15 (6+ months seasoned).
    // Quarterly resets: 2024-07-01, 2024-10-01, 2025-01-01, 2025-04-01
    // Resets on 2024-07-01 and 2024-10-01 are in the past (before as_of).
    let commitment_date = date!(2024 - 07 - 01);
    let maturity_date = date!(2025 - 07 - 01);
    let as_of = date!(2025 - 01 - 15);

    let facility = build_seasoned_floating_facility(commitment_date, maturity_date);

    // Forward curve: flat 4% (this is what would be used without fixings)
    // Flat curve based at commitment_date: seasoned resets strictly before
    // the curve base now error under the default FloatingRateFallback.
    let fwd_curve = build_flat_forward_curve(0.04, commitment_date, "USD-SOFR-3M", 0.25);
    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD-OIS");

    // Fixings: provide historical rates significantly different from the forward curve
    // to verify fixings are actually being used.
    let fixing_series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2024 - 07 - 01), 0.053), // 5.3% fixing (vs 4% forward)
            (date!(2024 - 10 - 01), 0.051), // 5.1% fixing (vs 4% forward)
            (date!(2025 - 01 - 01), 0.049), // 4.9% fixing (this reset is also past)
        ],
        None,
    )
    .unwrap();

    // Build market with fixings
    let market_with_fixings = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve.clone())
        .insert_series(fixing_series);

    // Build market without fixings
    let market_without_fixings = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Generate cashflows WITH fixings
    let engine_with = CashflowEngine::new(
        &facility,
        Some(&market_with_fixings),
        as_of,
        finstack_core::market_data::fixings::get_fixing_series(&market_with_fixings, "USD-SOFR-3M")
            .ok(),
    )
    .unwrap();
    let schedule_with = engine_with.generate_deterministic().unwrap();

    // Generate cashflows WITHOUT fixings (graceful degradation)
    let engine_without =
        CashflowEngine::new(&facility, Some(&market_without_fixings), as_of, None).unwrap();
    let schedule_without = engine_without.generate_deterministic().unwrap();

    // Both should succeed (graceful degradation)
    assert!(
        !schedule_with.schedule.flows.is_empty(),
        "Should generate cashflows with fixings"
    );
    assert!(
        !schedule_without.schedule.flows.is_empty(),
        "Should generate cashflows without fixings"
    );

    // Find the interest cashflows (FloatReset kind) and compare rates.
    // With fixings, past periods should use higher rates (5.3%, 5.1%) vs forward (4%).
    let float_flows_with: Vec<_> = schedule_with
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .collect();

    let float_flows_without: Vec<_> = schedule_without
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows_with.len(),
        float_flows_without.len(),
        "Same number of floating rate cashflows"
    );
    assert!(
        !float_flows_with.is_empty(),
        "Should have at least one floating rate cashflow"
    );

    // The first cashflow should use fixing rates (higher) vs forward rates.
    // Fixings are 5.3%/5.1% + 200bp spread = 7.3%/7.1%
    // Forward is ~4% + 200bp spread = ~6%
    // So the fixing-based interest should be higher.
    let total_interest_with: f64 = float_flows_with.iter().map(|cf| cf.amount.amount()).sum();

    let total_interest_without: f64 = float_flows_without
        .iter()
        .map(|cf| cf.amount.amount())
        .sum();

    // With fixings (higher rates for past periods), total interest should be higher
    assert!(
        total_interest_with > total_interest_without,
        "Fixing-based interest ({:.2}) should exceed forward-projected interest ({:.2}) \
         because past fixings (5.3%/5.1%) are higher than the forward rate (4%)",
        total_interest_with,
        total_interest_without,
    );

    // Verify the rate on a past-reset cashflow is consistent with the fixing.
    // First float cashflow pays on ~2025-01-01 (period: 2024-10-01 to 2025-01-01,
    // reset date 2024-10-01 with fixing 5.1%).
    // Expected all-in rate: max(5.1%, 0%) + 2% = 7.1%
    // The actual rate stored is the time-weighted average for the period.
    if let Some(first_cf) = float_flows_with.first() {
        if let Some(rate) = first_cf.rate {
            // The rate should reflect fixing + spread, not forward + spread.
            // Forward-based would be ~4% + 2% = 6%. Fixing-based should be ~7%.
            assert!(
                rate > 0.065,
                "Rate on past-reset period should reflect fixing ({:.4}%), expected > 6.5%",
                rate * 100.0
            );
        }
    }
}

/// When fixings are not provided for a seasoned facility, the engine should
/// gracefully fall back to forward curve projection (backwards compatibility).
#[test]
fn test_seasoned_facility_without_fixings_uses_forward_projection() {
    let commitment_date = date!(2024 - 07 - 01);
    let maturity_date = date!(2025 - 07 - 01);
    let as_of = date!(2025 - 01 - 15);

    let facility = build_seasoned_floating_facility(commitment_date, maturity_date);

    // Flat curve based at commitment_date: seasoned resets strictly before
    // the curve base now error under the default FloatingRateFallback.
    let fwd_curve = build_flat_forward_curve(0.04, commitment_date, "USD-SOFR-3M", 0.25);
    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // No fixings provided -- should not error, should use forward projection
    let engine = CashflowEngine::new(&facility, Some(&market), as_of, None).unwrap();
    let schedule = engine.generate_deterministic().unwrap();

    let float_flows: Vec<_> = schedule
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should produce floating rate cashflows even without fixings"
    );

    // All rates should be close to forward rate + spread = ~4% + 2% = ~6%
    for cf in &float_flows {
        if let Some(rate) = cf.rate {
            assert!(
                (rate - 0.06).abs() < 0.005,
                "Rate should be near forward + spread (6%), got {:.4}%",
                rate * 100.0
            );
        }
    }
}

/// Fixings with a floor: when the fixing rate is below the floor, the floor
/// should be applied to the index rate before adding spread.
#[test]
fn test_fixings_respect_floor() {
    let commitment_date = date!(2024 - 07 - 01);
    let maturity_date = date!(2025 - 07 - 01);
    let as_of = date!(2025 - 01 - 15);

    // Build a facility with a 3% floor on the index rate
    let facility = RevolvingCredit::builder()
        .id("RC-FIXING-FLOOR".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(10_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Floating(FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"), // +200 bps
            gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: Some(rust_decimal::Decimal::try_from(300.0).expect("valid")), // 3% floor
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            dc: DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
            payment_lag_days: 0,
        }))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let fwd_curve = build_flat_forward_curve(0.04, as_of, "USD-SOFR-3M", 0.25);
    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD-OIS");

    // Fixing at 1% (below the 3% floor)
    let fixing_series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2024 - 07 - 01), 0.01), // 1% (below 3% floor)
            (date!(2024 - 10 - 01), 0.01), // 1% (below 3% floor)
            (date!(2025 - 01 - 01), 0.01), // 1% (below 3% floor)
        ],
        None,
    )
    .unwrap();

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_series(fixing_series);

    let fixings =
        finstack_core::market_data::fixings::get_fixing_series(&market, "USD-SOFR-3M").ok();

    let engine = CashflowEngine::new(&facility, Some(&market), as_of, fixings).unwrap();
    let schedule = engine.generate_deterministic().unwrap();

    let float_flows: Vec<_> = schedule
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .collect();

    assert!(!float_flows.is_empty());

    // For past resets: floor(1%, 3%) = 3%, + 200bp spread = 5%
    // Check the rate is at least 5% (floor is binding)
    if let Some(first_cf) = float_flows.first() {
        if let Some(rate) = first_cf.rate {
            assert!(
                (rate - 0.05).abs() < 0.002,
                "Rate should be near floor (3%) + spread (2%) = 5%, got {:.4}%",
                rate * 100.0
            );
        }
    }
}

/// Non-seasoned facilities (all resets in the future) should behave
/// identically with or without fixings.
#[test]
fn test_non_seasoned_facility_ignores_fixings() {
    let commitment_date = date!(2025 - 01 - 15); // Same as as_of
    let maturity_date = date!(2026 - 01 - 15);
    let as_of = date!(2025 - 01 - 15);

    let facility = build_seasoned_floating_facility(commitment_date, maturity_date);

    let fwd_curve = build_flat_forward_curve(0.04, as_of, "USD-SOFR-3M", 0.25);
    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD-OIS");

    // Provide fixings even though none should be used (all resets are >= as_of)
    let fixing_series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(date!(2025 - 01 - 15), 0.10)], // 10% fixing (very different from forward)
        None,
    )
    .unwrap();

    let market_with = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve.clone())
        .insert_series(fixing_series);

    let market_without = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let fixings =
        finstack_core::market_data::fixings::get_fixing_series(&market_with, "USD-SOFR-3M").ok();

    let engine_with = CashflowEngine::new(&facility, Some(&market_with), as_of, fixings).unwrap();
    let schedule_with = engine_with.generate_deterministic().unwrap();

    let engine_without =
        CashflowEngine::new(&facility, Some(&market_without), as_of, None).unwrap();
    let schedule_without = engine_without.generate_deterministic().unwrap();

    // When all resets are in the future, fixings should not be used.
    // Both should produce identical cashflows.
    let total_with: f64 = schedule_with
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .map(|cf| cf.amount.amount())
        .sum();

    let total_without: f64 = schedule_without
        .schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == finstack_core::cashflow::CFKind::FloatReset)
        .map(|cf| cf.amount.amount())
        .sum();

    assert!(
        (total_with - total_without).abs() < 1e-6,
        "Non-seasoned facility should produce identical cashflows with or without fixings. \
         With: {:.6}, Without: {:.6}",
        total_with,
        total_without,
    );
}

/// Integration test: pricing a seasoned facility through the full pricer
/// with fixings in the market context.
#[test]
fn test_pricer_integration_with_fixings() {
    use finstack_valuations::instruments::Instrument;

    let commitment_date = date!(2024 - 07 - 01);
    let maturity_date = date!(2025 - 07 - 01);
    let as_of = date!(2025 - 01 - 15);

    let facility = build_seasoned_floating_facility(commitment_date, maturity_date);

    // Flat curve based at commitment_date: seasoned resets strictly before
    // the curve base now error under the default FloatingRateFallback.
    let fwd_curve = build_flat_forward_curve(0.04, commitment_date, "USD-SOFR-3M", 0.25);
    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD-OIS");

    // Fixings with higher rates than the forward curve
    let fixing_series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2024 - 07 - 01), 0.053),
            (date!(2024 - 10 - 01), 0.051),
            (date!(2025 - 01 - 01), 0.049),
        ],
        None,
    )
    .unwrap();

    let market_with = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve.clone())
        .insert_series(fixing_series);

    let market_without = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Price with fixings (higher past rates)
    let pv_with = facility.value(&market_with, as_of).unwrap();

    // Price without fixings (using forward for past resets)
    let pv_without = facility.value(&market_without, as_of).unwrap();

    // Both should produce valid PV
    assert!(pv_with.amount().is_finite());
    assert!(pv_without.amount().is_finite());

    // With higher fixing rates, the lender receives more interest, so PV should be higher
    assert!(
        pv_with.amount() > pv_without.amount(),
        "PV with higher fixings ({:.2}) should exceed PV with forward projection ({:.2})",
        pv_with.amount(),
        pv_without.amount(),
    );
}
