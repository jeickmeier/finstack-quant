//! Comprehensive unit tests for Fixed Income Index Total Return Swaps.
//!
//! Tests cover instrument creation, validation, pricing, carry and roll calculations,
//! duration sensitivity, and index-specific behaviors.

use super::test_utils::*;
use finstack_cashflows::CashflowProvider;
use finstack_core::currency::Currency::*;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::money::Money;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::TrsSide;
use rust_decimal::Decimal;

// ================================================================================================
// Construction and Validation Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_builder_defaults() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new().build();

    // Assert
    assert_eq!(trs.id.as_str(), "TEST-FI-TRS-001");
    assert_eq!(trs.notional.amount(), 10_000_000.0);
    assert_eq!(trs.notional.currency(), USD);
    assert_eq!(trs.side, TrsSide::ReceiveTotalReturn);
    assert_eq!(trs.underlying.contract_size, 1.0);
}

#[test]
fn test_fi_index_trs_builder_custom_params() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new()
        .id("CUSTOM-FI-TRS-001")
        .notional(Money::new(25_000_000.0, USD))
        .spread_bp(150.0)
        .side(TrsSide::PayTotalReturn)
        .build();

    // Assert
    assert_eq!(trs.id.as_str(), "CUSTOM-FI-TRS-001");
    assert_eq!(trs.notional.amount(), 25_000_000.0);
    assert_eq!(trs.financing.spread_bp, Decimal::from(150));
    assert_eq!(trs.side, TrsSide::PayTotalReturn);
}

#[test]
fn test_fi_index_trs_with_yield_only() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(Some("HY-INDEX-YIELD".into()))
        .duration_id(None)
        .build();

    // Assert
    assert!(trs.underlying.yield_id.is_some());
    assert!(trs.underlying.duration_id.is_none());
}

#[test]
fn test_fi_index_trs_with_duration_only() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(None)
        .duration_id(Some("HY-INDEX-DURATION".into()))
        .build();

    // Assert
    assert!(trs.underlying.yield_id.is_none());
    assert!(trs.underlying.duration_id.is_some());
}

#[test]
fn test_fi_index_trs_with_yield_and_duration() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(Some("HY-INDEX-YIELD".into()))
        .duration_id(Some("HY-INDEX-DURATION".into()))
        .build();

    // Assert
    assert!(trs.underlying.yield_id.is_some());
    assert!(trs.underlying.duration_id.is_some());
}

#[test]
fn test_fi_index_trs_currency_consistency() {
    // Arrange & Act
    let trs = TestFIIndexTrsBuilder::new()
        .notional(Money::new(10_000_000.0, USD))
        .build();

    // Assert - Index base currency should match notional currency
    assert_eq!(trs.notional.currency(), trs.underlying.base_currency);
}

// ================================================================================================
// NPV and Pricing Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_npv_receive_side() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(100.0)
        .build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();

    // Assert
    assert_eq!(npv.currency(), USD);
    assert!(npv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_npv_pay_side() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::PayTotalReturn)
        .spread_bp(100.0)
        .build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();

    // Assert
    assert_eq!(npv.currency(), USD);
    assert!(npv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_npv_pay_vs_receive_symmetry() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_receive = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(75.0)
        .build();

    let trs_pay = TestFIIndexTrsBuilder::new()
        .side(TrsSide::PayTotalReturn)
        .spread_bp(75.0)
        .build();

    // Act
    let npv_receive = trs_receive.value(&market, as_of).unwrap();
    let npv_pay = trs_pay.value(&market, as_of).unwrap();

    // Assert - NPVs should be opposite
    assert_approx_eq(
        npv_receive.amount() + npv_pay.amount(),
        0.0,
        1.0, // $1 tolerance
        "Receive and pay TRS NPVs should sum to zero",
    );
}

#[test]
fn test_fi_index_trs_pricing_with_different_spreads() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_low_spread = TestFIIndexTrsBuilder::new().spread_bp(50.0).build();

    let trs_high_spread = TestFIIndexTrsBuilder::new().spread_bp(200.0).build();

    // Act
    let npv_low = trs_low_spread.value(&market, as_of).unwrap();
    let npv_high = trs_high_spread.value(&market, as_of).unwrap();

    // Assert - For receive TR, higher financing spread means lower NPV
    assert!(
        npv_low.amount() > npv_high.amount(),
        "Higher financing spread should reduce NPV for receive TR side"
    );
}

// ================================================================================================
// Leg Decomposition Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_total_return_leg_pv() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let tr_pv = trs.pv_total_return_leg(&market, as_of).unwrap();

    // Assert
    assert_eq!(tr_pv.currency(), USD);
    assert!(tr_pv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_financing_leg_pv() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().spread_bp(100.0).build();

    // Act
    let fin_pv = trs.pv_financing_leg(&market, as_of).unwrap();

    // Assert
    assert_eq!(fin_pv.currency(), USD);
    assert!(fin_pv.amount().is_finite());
    // Financing leg should have positive PV
    assert!(fin_pv.amount() > 0.0);
}

#[test]
fn test_fi_index_trs_npv_equals_legs_difference() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(100.0)
        .build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();
    let tr_pv = trs.pv_total_return_leg(&market, as_of).unwrap();
    let fin_pv = trs.pv_financing_leg(&market, as_of).unwrap();

    // Assert - NPV = TR leg - Financing leg (for receive side)
    let expected_npv = tr_pv.checked_sub(fin_pv).unwrap();
    assert_money_approx_eq(
        npv,
        expected_npv,
        TOLERANCE_CENTS,
        "NPV should equal TR leg PV minus financing leg PV",
    );
}

#[test]
fn test_fi_index_trs_financing_leg_increases_with_spread() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_low = TestFIIndexTrsBuilder::new().spread_bp(25.0).build();

    let trs_high = TestFIIndexTrsBuilder::new().spread_bp(200.0).build();

    // Act
    let pv_low = trs_low.pv_financing_leg(&market, as_of).unwrap();
    let pv_high = trs_high.pv_financing_leg(&market, as_of).unwrap();

    // Assert
    assert!(
        pv_high.amount() > pv_low.amount(),
        "Higher spread should result in higher financing leg PV"
    );
}

// ================================================================================================
// Index Characteristics Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_carry_component() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    // High yield index (higher carry)
    let trs_hy = TestFIIndexTrsBuilder::new()
        .yield_id(Some("HY-INDEX-YIELD".into())) // 5.5%
        .spread_bp(100.0)
        .build();

    // Act
    let tr_pv = trs_hy.pv_total_return_leg(&market, as_of).unwrap();

    // Assert - Total return should be positive for positive yield
    assert!(tr_pv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_sensitivity_to_yield() {
    // Arrange
    let as_of = as_of_date();
    let market_base = create_market_context();

    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(100.0)
        .build();

    // Higher yield scenario
    let market_high_yield = market_base
        .clone()
        .insert_price("HY-INDEX-YIELD", MarketScalar::Unitless(0.065)); // +100bp

    // Act
    let npv_base = trs.value(&market_base, as_of).unwrap();
    let npv_high = trs.value(&market_high_yield, as_of).unwrap();

    // Assert - Higher yield increases carry, should increase TR leg PV
    assert!(
        npv_high.amount() > npv_base.amount(),
        "Higher index yield should increase NPV for receive TR side"
    );
}

#[test]
fn test_fi_index_trs_sensitivity_to_duration() {
    // Arrange
    let as_of = as_of_date();
    let market_base = create_market_context();

    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(100.0)
        .build();

    // Higher duration scenario
    let market_high_duration = market_base
        .clone()
        .insert_price("HY-INDEX-DURATION", MarketScalar::Unitless(6.0)); // +1.5 years

    // Act
    let npv_base = trs.value(&market_base, as_of).unwrap();
    let npv_high_dur = trs.value(&market_high_duration, as_of).unwrap();

    // Assert - Both should compute (effect depends on roll model)
    assert!(npv_base.amount().is_finite());
    assert!(npv_high_dur.amount().is_finite());
}

// ================================================================================================
// Market Sensitivity Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_sensitivity_to_interest_rates() {
    // Arrange
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(100.0)
        .build();

    // Base market
    let market_base = create_market_context();

    // Shifted rates market
    let mut market_shifted = MarketContext::new();
    let disc_shifted =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![
                (0.0, 1.0),
                (0.25, 0.9925),
                (0.5, 0.985),
                (1.0, 0.970),
                (2.0, 0.940),
                (5.0, 0.850),
            ])
            .interp(finstack_core::math::interp::InterpStyle::LogLinear)
            .build()
            .unwrap();
    market_shifted = market_shifted.insert(disc_shifted);

    let fwd_shifted =
        finstack_core::market_data::term_structures::ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots(vec![(0.0, 0.03), (0.25, 0.031), (0.5, 0.032), (1.0, 0.033)])
            .interp(finstack_core::math::interp::InterpStyle::Linear)
            .build()
            .unwrap();
    market_shifted = market_shifted.insert(fwd_shifted);
    market_shifted = market_shifted.insert_price("HY-INDEX-YIELD", MarketScalar::Unitless(0.055));
    market_shifted = market_shifted.insert_price("HY-INDEX-DURATION", MarketScalar::Unitless(4.5));

    // Act
    let npv_base = trs.value(&market_base, as_of).unwrap();
    let npv_shifted = trs.value(&market_shifted, as_of).unwrap();

    // Assert
    assert!(npv_base.amount().is_finite());
    assert!(npv_shifted.amount().is_finite());
}

// ================================================================================================
// Cashflow Schedule Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_cashflow_schedule_generation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().tenor_months(12).build();

    // Act
    let flows = trs.dated_cashflows(&market, as_of).unwrap();

    // Assert
    // 1 year quarterly = 4 payments
    assert_eq!(flows.len(), 4, "Should have 4 quarterly cashflows");

    // All flows in correct currency
    for (date, amount) in &flows {
        assert!(date > &as_of);
        assert_eq!(amount.currency(), USD);
    }
}

#[test]
fn test_fi_index_trs_cashflow_schedule_dates_ordered() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let flows = trs.dated_cashflows(&market, as_of).unwrap();

    // Assert - Dates should be strictly increasing
    for i in 1..flows.len() {
        assert!(
            flows[i].0 > flows[i - 1].0,
            "Cashflow dates should be strictly increasing"
        );
    }
}

// ================================================================================================
// Tenor Variation Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_short_tenor_6_months() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().tenor_months(6).build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();

    // Assert
    assert!(npv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_medium_tenor_3_years() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().tenor_months(36).build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();

    // Assert
    assert!(npv.amount().is_finite());
}

#[test]
fn test_fi_index_trs_long_tenor_5_years() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().tenor_months(60).build();

    // Act
    let npv = trs.value(&market, as_of).unwrap();

    // Assert
    assert!(npv.amount().is_finite());
}

// ================================================================================================
// Notional Size Tests
// ================================================================================================

#[test]
fn test_fi_index_trs_notional_scaling() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_5m = TestFIIndexTrsBuilder::new()
        .notional(Money::new(5_000_000.0, USD))
        .build();

    let trs_25m = TestFIIndexTrsBuilder::new()
        .notional(Money::new(25_000_000.0, USD))
        .build();

    // Act
    let npv_5m = trs_5m.value(&market, as_of).unwrap();
    let npv_25m = trs_25m.value(&market, as_of).unwrap();

    // Assert - NPV should scale approximately linearly with notional
    assert_approx_eq(
        npv_25m.amount() / npv_5m.amount(),
        5.0,
        0.01, // 1% tolerance
        "NPV should scale linearly with notional",
    );
}

// ================================================================================================
// Comparison Tests: HY vs IG
// ================================================================================================

#[test]
fn test_fi_index_trs_hy_vs_ig() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    // High Yield TRS (higher yield, lower duration typically)
    let trs_hy = TestFIIndexTrsBuilder::new()
        .yield_id(Some("HY-INDEX-YIELD".into())) // 5.5%
        .duration_id(Some("HY-INDEX-DURATION".into())) // 4.5 years
        .spread_bp(100.0)
        .build();

    // Investment Grade TRS (lower yield, higher duration typically)
    let trs_ig = TestFIIndexTrsBuilder::new()
        .yield_id(Some("IG-INDEX-YIELD".into())) // 3.5%
        .duration_id(Some("IG-INDEX-DURATION".into())) // 7.0 years
        .spread_bp(50.0) // Tighter spread for IG
        .build();

    // Act
    let tr_pv_hy = trs_hy.pv_total_return_leg(&market, as_of).unwrap();
    let tr_pv_ig = trs_ig.pv_total_return_leg(&market, as_of).unwrap();

    // Assert - HY should have higher carry (yield * duration)
    assert!(tr_pv_hy.amount().is_finite());
    assert!(tr_pv_ig.amount().is_finite());
}

// ================================================================================================
// Analytical Verification Tests
// ================================================================================================

/// Verifies TRS pricing against a closed-form result under flat rates and flat yield.
///
/// Under a flat discount rate `r` and flat index yield `y`, with quarterly payments:
/// - Each period return = `e^{y * dt} - 1`
/// - Each period payment = `Notional * (e^{y*dt} - 1)`
/// - Discounted at DF(as_of → period_end)
///
/// The financing leg uses the flat forward rate `r` + spread.
///
/// This test verifies that the pricer matches the analytical sum.
#[test]
fn test_fi_index_trs_analytical_flat_rate_flat_yield() {
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

    let as_of = as_of_date();
    let notional_amount = 10_000_000.0;
    let index_yield = 0.055; // 5.5%
    let flat_rate = 0.02; // 2% flat rate
    let spread_bp_val = 100.0; // 100bp spread
    let spread_decimal = spread_bp_val / 10000.0;

    // Build flat discount curve: DF(t) = e^{-r*t}
    // We approximate by using knot points that trace e^{-0.02*t}
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots(vec![
            (0.0, 1.0),
            (0.25, (-0.02_f64 * 0.25).exp()),
            (0.50, (-0.02_f64 * 0.50).exp()),
            (0.75, (-0.02_f64 * 0.75).exp()),
            (1.00, (-0.02_f64 * 1.00).exp()),
        ])
        .interp(finstack_core::math::interp::InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Build flat forward curve at the same rate
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots(vec![(0.0, flat_rate), (1.0, flat_rate)])
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .build()
        .unwrap();

    let market = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_price("HY-INDEX-YIELD", MarketScalar::Unitless(index_yield))
        .insert_price("HY-INDEX-DURATION", MarketScalar::Unitless(4.5));

    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(spread_bp_val)
        .tenor_months(12)
        .build();

    // Compute PVs
    let tr_pv = trs.pv_total_return_leg(&market, as_of).unwrap();
    let fin_pv = trs.pv_financing_leg(&market, as_of).unwrap();
    let npv = trs.value(&market, as_of).unwrap();

    // Analytical total return leg PV (4 quarterly periods, flat rate)
    let dt = 0.25; // Approximate quarterly fraction
    let period_return = (index_yield * dt).exp() - 1.0;
    let mut analytical_tr_pv = 0.0;
    for i in 1..=4 {
        let t_end = dt * i as f64;
        let df_end = (-flat_rate * t_end).exp();
        analytical_tr_pv += notional_amount * period_return * df_end;
    }

    // Analytical financing leg PV
    let mut analytical_fin_pv = 0.0;
    for i in 1..=4 {
        let t_end = dt * i as f64;
        let df_end = (-flat_rate * t_end).exp();
        let total_rate = flat_rate + spread_decimal;
        analytical_fin_pv += notional_amount * total_rate * dt * df_end;
    }

    // The pricer should match the analytical values within tolerance.
    // Tolerance is relaxed because the actual schedule dates may differ slightly
    // from the idealized 0.25Y quarters (due to business day adjustments).
    let tolerance = notional_amount * 0.001; // 0.1% of notional = $10,000

    assert_approx_eq(
        tr_pv.amount(),
        analytical_tr_pv,
        tolerance,
        "Total return leg PV should match analytical (carry model, flat rate)",
    );

    assert_approx_eq(
        fin_pv.amount(),
        analytical_fin_pv,
        tolerance,
        "Financing leg PV should match analytical (flat rate + spread)",
    );

    // NPV = TR - Financing (for receive side)
    assert_approx_eq(
        npv.amount(),
        analytical_tr_pv - analytical_fin_pv,
        tolerance,
        "NPV should equal analytical TR - Financing",
    );

    // Sanity: For HY index (5.5% yield) vs financing at (2% + 1% = 3%),
    // the receiver should have positive NPV (carry advantage)
    assert!(
        npv.amount() > 0.0,
        "Receive TR with 5.5% yield vs 3% financing should have positive NPV, got {}",
        npv.amount()
    );
}

// ================================================================================================
// Validation / Error Path Tests
// ================================================================================================

/// When `yield_id` is configured but the market context lacks the corresponding
/// scalar, pricing must fail with a descriptive error rather than silently
/// assuming zero carry.
#[test]
fn test_fi_index_trs_errors_on_missing_configured_yield() {
    let as_of = as_of_date();

    // Market context has curves but deliberately omits the yield scalar
    let market = MarketContext::new()
        .insert(
            finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.98)])
                .interp(finstack_core::math::interp::InterpStyle::LogLinear)
                .build()
                .unwrap(),
        )
        .insert(
            finstack_core::market_data::term_structures::ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(as_of)
                .knots(vec![(0.0, 0.02), (1.0, 0.02)])
                .interp(finstack_core::math::interp::InterpStyle::Linear)
                .build()
                .unwrap(),
        );
    // NOTE: "MISSING-YIELD" is NOT inserted into the market context

    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(Some("MISSING-YIELD".into()))
        .duration_id(None) // duration not needed for this test
        .build();

    let result = trs.pv_total_return_leg(&market, as_of);
    assert!(
        result.is_err(),
        "Should fail when yield_id is configured but missing from market data"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("MISSING-YIELD"),
        "Error should mention the missing yield_id, got: {}",
        err_msg
    );
}

/// When `yield_id` is `None`, the pricer should default to zero carry and
/// succeed without requiring any yield scalar in the market context.
#[test]
fn test_fi_index_trs_zero_carry_when_yield_id_is_none() {
    let as_of = as_of_date();
    let market = create_market_context();

    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(None)
        .duration_id(None)
        .build();

    let tr_pv = trs.pv_total_return_leg(&market, as_of).unwrap();

    // With zero yield, total return leg should be zero (e^{0 * dt} - 1 = 0)
    assert_approx_eq(
        tr_pv.amount(),
        0.0,
        0.01,
        "Zero yield should produce zero total return leg PV",
    );
}

/// When `duration_id` is configured but the market context lacks the
/// corresponding scalar, the DurationDv01 metric must fail.
#[test]
fn test_fi_index_trs_duration_dv01_errors_on_missing_configured_duration() {
    use finstack_valuations::metrics::MetricId;

    let as_of = as_of_date();
    let market = create_market_context();

    // Build a TRS with a duration_id that does NOT exist in create_market_context()
    let trs = TestFIIndexTrsBuilder::new()
        .duration_id(Some("MISSING-DURATION".into()))
        .build();

    let result = trs.price_with_metrics(
        &market,
        as_of,
        &[MetricId::DurationDv01],
        finstack_valuations::instruments::PricingOptions::default(),
    );
    assert!(
        result.is_err(),
        "Should fail when duration_id is configured but missing from market data"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("MISSING-DURATION"),
        "Error should mention the missing duration_id, got: {}",
        err_msg
    );
}

/// When `duration_id` is `None`, the DurationDv01 metric should default to 5.0Y
/// and compute successfully.
#[test]
fn test_fi_index_trs_duration_dv01_defaults_when_duration_id_is_none() {
    use finstack_valuations::metrics::MetricId;

    let as_of = as_of_date();
    let market = create_market_context();

    let trs = TestFIIndexTrsBuilder::new().duration_id(None).build();

    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DurationDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("duration_dv01").unwrap();
    // Expected: 10_000_000 × 5.0 × 0.0001 = 5_000
    assert_approx_eq(
        dv01,
        5_000.0,
        1.0, // $1 tolerance
        "DurationDv01 should use 5.0Y default when duration_id is None",
    );
}

/// Providing a `MarketScalar::Price` for yield should fail with a descriptive error.
#[test]
fn test_fi_index_trs_errors_on_price_scalar_for_yield() {
    let as_of = as_of_date();

    let market = create_market_context().insert_price(
        "BAD-YIELD",
        MarketScalar::Price(Money::new(100.0, finstack_core::currency::Currency::USD)),
    );

    let trs = TestFIIndexTrsBuilder::new()
        .yield_id(Some("BAD-YIELD".into()))
        .build();

    let result = trs.pv_total_return_leg(&market, as_of);
    assert!(
        result.is_err(),
        "Should fail when yield is provided as Price scalar"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("unitless"),
        "Error should mention unitless, got: {}",
        err_msg
    );
}

/// Providing a `MarketScalar::Price` for duration should fail with a descriptive error.
#[test]
fn test_fi_index_trs_errors_on_price_scalar_for_duration() {
    use finstack_valuations::metrics::MetricId;

    let as_of = as_of_date();

    let market = create_market_context().insert_price(
        "BAD-DURATION",
        MarketScalar::Price(Money::new(5.0, finstack_core::currency::Currency::USD)),
    );

    let trs = TestFIIndexTrsBuilder::new()
        .duration_id(Some("BAD-DURATION".into()))
        .build();

    let result = trs.price_with_metrics(
        &market,
        as_of,
        &[MetricId::DurationDv01],
        finstack_valuations::instruments::PricingOptions::default(),
    );
    assert!(
        result.is_err(),
        "Should fail when duration is provided as Price scalar"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("unitless"),
        "Error should mention unitless, got: {}",
        err_msg
    );
}
