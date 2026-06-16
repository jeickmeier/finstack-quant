//! Numerical stability tests for IRS pricing.
//!
//! Tests verify:
//! - Kahan summation produces stable results for long swaps
//! - ANNUITY_EPSILON guards against division by zero
//! - Input validation catches invalid parameters
//! - Edge cases are handled gracefully
//!
//! References:
//! - Kahan, W. (1965). "Further Remarks on Reducing Truncation Errors."
//! - ISDA stress testing guidelines

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, InterestRateSwap, PayReceive,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ]);

    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate), (30.0, rate)])
        .build()
        .unwrap()
}

// ============================================================================
// Input Validation Tests
// ============================================================================

#[test]
fn test_validation_rejects_end_before_start() {
    let result = test_utils::usd_irs_swap(
        InstrumentId::new("INVALID_DATES"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2029 - 01 - 01), // Start after end
        date!(2024 - 01 - 01), // End before start
        PayReceive::Pay,
    );

    assert!(result.is_err(), "Should reject end <= start");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("date range") || err.contains("end") && err.contains("start"),
        "Error should mention date range: {}",
        err
    );
}

#[test]
fn test_validation_rejects_zero_notional() {
    let result = test_utils::usd_irs_swap(
        InstrumentId::new("ZERO_NOTIONAL"),
        Money::new(0.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    );

    assert!(result.is_err(), "Should reject zero notional");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("notional") || err.contains("positive"),
        "Error should mention notional: {}",
        err
    );
}

#[test]
fn test_validation_rejects_negative_notional() {
    let result = test_utils::usd_irs_swap(
        InstrumentId::new("NEGATIVE_NOTIONAL"),
        Money::new(-1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    );

    assert!(result.is_err(), "Should reject negative notional");
}

#[test]
fn test_validation_rejects_extreme_rate() {
    // 15000% rate should be rejected as non-physical
    let result = test_utils::usd_irs_swap(
        InstrumentId::new("EXTREME_RATE"),
        Money::new(1_000_000.0, Currency::USD),
        150.0, // 15000% rate - well above MAX_RATE_MAGNITUDE (100.0)
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    );

    assert!(result.is_err(), "Should reject extreme rate");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("rate") || err.contains("magnitude"),
        "Error should mention rate: {}",
        err
    );
}

#[test]
fn test_validation_accepts_negative_rate() {
    // -0.5% rate should be accepted (realistic in EUR/JPY markets)
    let result = test_utils::usd_irs_swap(
        InstrumentId::new("NEGATIVE_RATE"),
        Money::new(1_000_000.0, Currency::USD),
        -0.005, // -0.5% rate
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    );

    assert!(result.is_ok(), "Should accept small negative rate");
}

#[test]
fn test_validation_accepts_valid_swap() {
    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("VALID_SWAP"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Pay,
    )
    .expect("Valid swap should construct");

    // Explicit validation should also pass
    assert!(swap.validate().is_ok());
}

// ============================================================================
// Long-Dated Swap Stability Tests (Kahan Summation)
// ============================================================================

#[test]
fn test_30y_swap_numerical_stability() {
    // 30-year swap with quarterly payments = 120 periods
    // Tests Kahan summation effectiveness
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2054 - 01 - 01); // 30 years

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("SWAP_30Y"),
        Money::new(100_000_000.0, Currency::USD), // Large notional
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    // Should price without numerical issues
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity, MetricId::ParRate, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("30Y swap should price successfully");

    let annuity = result.measures["annuity"];
    let par_rate = result.measures["par_rate"];

    // Annuity for 30Y swap at 5% should be roughly 15-17 years
    assert!(
        annuity > 14.0 && annuity < 18.0,
        "30Y annuity {} should be in reasonable range",
        annuity
    );

    // Par rate should be close to flat forward rate
    // Note: ~1% tolerance due to day-count convention mismatch between
    // fixed leg (30/360) and floating leg (ACT/360) which is market standard
    assert!(
        (par_rate - 0.05).abs() < 0.005,
        "30Y par rate {} should be close to 5% (within 50bp)",
        par_rate
    );

    // NPV should be defined and finite
    assert!(
        result.value.amount().is_finite(),
        "NPV should be finite for 30Y swap"
    );
}

#[test]
fn test_annuity_deterministic_across_runs() {
    // Same swap should produce identical annuity across multiple runs
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2044 - 01 - 01); // 20 years

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("DETERMINISM_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let mut annuities = Vec::with_capacity(10);
    for _ in 0..10 {
        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Annuity],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        annuities.push(result.measures["annuity"]);
    }

    // All annuities should be bit-identical
    for (i, &a) in annuities.iter().enumerate().skip(1) {
        assert_eq!(
            annuities[0].to_bits(),
            a.to_bits(),
            "Annuity run {} ({}) differs from run 0 ({})",
            i,
            a,
            annuities[0]
        );
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_expired_swap_handling() {
    // Swap that has already expired should have zero NPV and handle gracefully
    let start = date!(2020 - 01 - 01);
    let end = date!(2023 - 01 - 01);
    let as_of = date!(2024 - 01 - 01); // After end

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    // Flat forward curve based at the swap start: seasoned resets strictly
    // before the curve base now error under the default FloatingRateFallback.
    let fwd_curve = build_flat_forward_curve(0.05, start, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = InterestRateSwap {
        id: InstrumentId::new("EXPIRED_SWAP"),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        },
        float: FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        },
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Expired swap should have zero NPV (no future cashflows)
    let npv = swap
        .value(&market, as_of)
        .expect("Expired swap should price");
    assert!(
        npv.amount().abs() < 1.0,
        "Expired swap NPV should be ~0, got {}",
        npv.amount()
    );

    // Annuity should be zero for expired swap
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Expired swap should price");
    let annuity = result.measures["annuity"];
    assert!(
        annuity.abs() < 1e-10,
        "Expired swap annuity should be ~0, got {}",
        annuity
    );
}

#[test]
fn test_very_short_swap_1_month() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2024 - 02 - 01); // 1 month

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Use manual construction for very short swap since quarterly freq won't work
    let swap = InterestRateSwap {
        id: InstrumentId::new("SHORT_SWAP"),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        float: FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Should price without issues
    let npv = swap.value(&market, as_of);
    assert!(npv.is_ok(), "1-month swap should price: {:?}", npv);
}

#[test]
fn test_extreme_rate_environment_stress() {
    // Test with extreme but valid rate scenarios (ISDA stress testing)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // High rate scenario (15%)
    let disc_high = build_flat_discount_curve(0.15, as_of, "USD-OIS");
    let fwd_high = build_flat_forward_curve(0.15, as_of, "USD-SOFR-3M");

    let market_high = MarketContext::new().insert(disc_high).insert(fwd_high);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("STRESS_HIGH"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv_high = swap.value(&market_high, as_of);
    assert!(
        npv_high.is_ok(),
        "Should price in high rate environment: {:?}",
        npv_high
    );

    // Low rate scenario (0.1%)
    let disc_low = build_flat_discount_curve(0.001, as_of, "USD-OIS");
    let fwd_low = build_flat_forward_curve(0.001, as_of, "USD-SOFR-3M");

    let market_low = MarketContext::new().insert(disc_low).insert(fwd_low);

    let npv_low = swap.value(&market_low, as_of);
    assert!(
        npv_low.is_ok(),
        "Should price in low rate environment: {:?}",
        npv_low
    );

    // Verify inverse relationship still holds
    let npv_high_val = npv_high.unwrap().amount();
    let npv_low_val = npv_low.unwrap().amount();

    assert!(
        npv_low_val > npv_high_val,
        "Receive fixed should be worth more in low rate env: low={}, high={}",
        npv_low_val,
        npv_high_val
    );
}

// ============================================================================
// Golden Value Tests
// ============================================================================

#[test]
fn test_golden_5y_usd_swap_par_npv() {
    // Golden test: 5Y USD swap at par should have NPV ≈ 0
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // First compute actual par rate for this swap
    let temp_swap = test_utils::usd_irs_swap(
        InstrumentId::new("TEMP_PAR"),
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let par_rate = temp_swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];

    // Now create a swap at the computed par rate
    let swap_at_par = test_utils::usd_irs_swap(
        InstrumentId::new("GOLDEN_PAR"),
        Money::new(10_000_000.0, Currency::USD),
        par_rate, // Use computed par rate
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let npv = swap_at_par.value(&market, as_of).unwrap();

    // NPV at par should be very close to zero
    // Tolerance: $100 on $10MM notional = 0.001% = 0.1bp (1bp tolerance)
    assert!(
        npv.amount().abs() < 100.0,
        "5Y USD par swap NPV should be ~0 (within 1bp), got {} ({:.2}bp, par rate: {:.6})",
        npv.amount(),
        npv.amount() / 1000.0, // Per $10MM notional, divide by 1000 to get bp
        par_rate
    );
}

#[test]
fn test_golden_annuity_5y_at_5pct() {
    // Golden test: 5Y semi-annual fixed leg at 5% flat
    // Annuity ≈ Σ (0.5 × e^(-0.05×t)) for t = 0.5, 1.0, ..., 5.0
    // ≈ 4.32 (analytical approximation)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("GOLDEN_ANNUITY"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = result.measures["annuity"];

    // Expected: ~4.32 years (analytical), allow 1% tolerance for day-count conventions
    assert!(
        (annuity - 4.32).abs() < 0.05,
        "5Y annuity at 5% should be ~4.32, got {} (diff: {:.4})",
        annuity,
        (annuity - 4.32).abs()
    );
}

#[test]
fn test_golden_dv01_approximation() {
    // Golden test: DV01 ≈ Annuity × Notional × 0.0001
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let notional = 10_000_000.0;

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("GOLDEN_DV01"),
        Money::new(notional, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = result.measures["annuity"];
    let dv01 = result.measures["dv01"];

    // DV01 approximation
    let dv01_approx = annuity * notional * 0.0001;

    // Actual DV01 (from parallel bump) should be within 3% of approximation
    // The difference is higher than bond DV01 because IRS DV01 includes
    // floating leg forward rate changes on the parallel bump
    assert!(
        (dv01.abs() - dv01_approx).abs() / dv01_approx < 0.03,
        "DV01 ({:.2}) should be ~{:.2} (within 3%)",
        dv01,
        dv01_approx
    );
}

#[test]
fn test_missing_curve_produces_clear_error() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // Create market with only discount curve (no forward curve)
    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);
    // Note: No forward curve added

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("MISSING_CURVE_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Pay,
    )
    .unwrap();

    // Pricing should fail with clear error about missing forward curve
    let result = swap.value(&market, as_of);
    assert!(result.is_err(), "Should fail when forward curve is missing");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("curve") || err_msg.contains("forward") || err_msg.contains("not found"),
        "Error should mention missing curve: {}",
        err_msg
    );
}
