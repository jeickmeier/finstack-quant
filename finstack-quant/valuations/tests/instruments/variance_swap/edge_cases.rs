//! Edge case and boundary condition tests for variance swaps.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::variance_swap::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

// ============================================================================
// Extreme Market Conditions
// ============================================================================

#[test]
fn test_valuation_with_extreme_high_volatility() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 2.0); // 200% vol
    let as_of = date(2024, 12, 1);

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
    assert!(pv.amount() > 0.0); // Receive side profits from high vol
}

#[test]
fn test_valuation_with_near_zero_volatility() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.01); // 1% vol
    let as_of = date(2024, 12, 1);

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
    assert!(pv.amount() < 0.0); // Receive side loses when vol below strike
}

#[test]
fn test_valuation_with_extreme_price_moves() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let mut prices = price_series(&swap, 5_000.0, 0.0);
    // Add extreme jumps
    for (i, (_, p)) in prices.iter_mut().enumerate() {
        if i % 10 == 0 {
            *p *= 1.5; // 50% jump
        }
    }
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 1);

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
}

#[test]
fn test_valuation_with_negative_rates() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    // Use earlier base date to allow pre-start valuation
    let curve_base = date(2024, 12, 1);
    let disc_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder(DISC_ID)
            .base_date(curve_base)
            .knots([
                (0.0, 1.0),
                (0.25, 1.005), // Negative rates => DF > 1
                (0.5, 1.01),
                (1.0, 1.02),
            ])
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            ) // Increasing DFs for negative rates
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear) // MonotoneConvex doesn't work for increasing DFs
            .build()
            .unwrap();
    let ctx = MarketContext::new().insert(disc_curve).insert_price(
        UNDERLYING_ID,
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(5_000.0),
    );
    let as_of = curve_base;

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

// ============================================================================
// Extreme Notionals and Strike Values
// ============================================================================

#[test]
fn test_valuation_with_very_large_notional() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(1e12, Currency::USD); // $1 trillion
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = swap.start_date;

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

#[test]
fn test_valuation_with_very_small_notional() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(1.0, Currency::USD); // $1
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = swap.start_date;

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
    assert!(pv.amount().abs() < 1.0); // Small notional => small PV
}

#[test]
fn test_valuation_with_very_high_strike_variance() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.strike_variance = 4.0; // 200% vol strike
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.5); // 50% vol
    let as_of = swap.start_date;

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
    assert!(pv.amount() < 0.0); // Market vol far below strike
}

#[test]
fn test_valuation_with_very_low_strike_variance() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.strike_variance = 0.0001; // ~1% vol strike
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.20);
    let as_of = swap.start_date;

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    assert!(pv.amount().is_finite());
    assert!(pv.amount() > 0.0); // Market vol far above strike
}

// ============================================================================
// Edge Cases in Time
// ============================================================================

#[test]
fn test_valuation_with_very_short_tenor() {
    // Arrange
    let start = date(2025, 1, 2);
    let end = date(2025, 1, 9); // 1 week
    let mut swap = sample_swap(PayReceive::Receive);
    swap.start_date = start;
    swap.maturity = end;
    swap.observation_freq = Tenor::daily();
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);

    // Act
    let pv = swap.value(&ctx, start);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

#[test]
fn test_valuation_with_very_long_tenor() {
    // Arrange
    let start = date(2025, 1, 2);
    let end = date(2035, 1, 2); // 10 years
    let mut swap = sample_swap(PayReceive::Receive);
    swap.start_date = start;
    swap.maturity = end;
    swap.observation_freq = Tenor::monthly();
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);

    // Act
    let pv = swap.value(&ctx, start);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

#[test]
fn test_valuation_on_exact_start_date() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);

    // Act
    let pv = swap.value(&ctx, swap.start_date);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

#[test]
fn test_valuation_one_day_before_maturity() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 3.0);
    let ctx = add_series(base_context(), &prices);
    let as_of = swap.maturity - time::Duration::days(1);

    // Act
    let pv = swap.value(&ctx, as_of);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

// ============================================================================
// Observation Tenor Edge Cases
// ============================================================================

#[test]
fn test_valuation_with_single_observation() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    // Set maturity very close to start for minimal observations
    swap.maturity = swap.start_date + time::Duration::days(1);
    swap.observation_freq = Tenor::daily();
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);

    // Act
    let pv = swap.value(&ctx, swap.start_date);

    // Assert
    assert!(pv.is_ok());
    assert!(pv.unwrap().amount().is_finite());
}

#[test]
fn test_observation_dates_with_very_high_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::daily();

    // Act
    let dates = swap.observation_dates();

    // Assert
    assert!(!dates.is_empty());
    assert!(dates.len() > 50); // Should have many observations
    for window in dates.windows(2) {
        let gap = window[1] - window[0];
        assert!(gap.whole_days() >= 1);
    }
}

// ============================================================================
// Missing or Incomplete Data
// ============================================================================

#[test]
fn test_valuation_with_missing_discount_curve_fails_gracefully() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = MarketContext::new().insert_price(
        UNDERLYING_ID,
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(5_000.0),
    );
    let as_of = swap.start_date;

    // Act
    let result = swap.value(&ctx, as_of);

    // Assert - should return error, not panic
    assert!(result.is_err());
}

#[test]
fn test_metrics_with_missing_implied_vol_falls_back_to_strike() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context(); // No implied vol
    let as_of = date(2024, 12, 1);

    // Act
    let result = swap.price_with_metrics(
        &ctx,
        as_of,
        &[MetricId::ExpectedVariance],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let ev = result.unwrap().measures[MetricId::ExpectedVariance.as_str()];
    // Should fallback to strike variance
    assert!((ev - swap.strike_variance).abs() < LOOSE_EPSILON);
}

#[test]
fn test_realized_variance_with_single_price_point() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context(); // Only spot, no series
    let as_of = swap.start_date;

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of);

    // Assert
    assert!(realized.is_ok());
    assert_eq!(realized.unwrap(), 0.0); // Insufficient data => 0
}

// ============================================================================
// Currency Edge Cases
// ============================================================================

#[test]
fn test_valuation_preserves_currency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(DEFAULT_NOTIONAL, Currency::EUR);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = swap.start_date;

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(pv.currency(), Currency::EUR);
}

#[test]
fn test_payoff_preserves_currency_across_calculations() {
    // Arrange
    let currencies = vec![Currency::USD, Currency::EUR, Currency::GBP, Currency::JPY];

    for ccy in currencies {
        let mut swap = sample_swap(PayReceive::Receive);
        swap.notional = Money::new(DEFAULT_NOTIONAL, ccy);
        let realized_var = 0.06;

        // Act
        let payoff = swap.payoff(realized_var);

        // Assert
        assert_eq!(payoff.currency(), ccy);
    }
}

// ============================================================================
// Numerical Stability
// ============================================================================

#[test]
fn test_valuation_is_stable_under_repeated_calculations() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = swap.start_date;

    // Act - compute multiple times
    let pv1 = swap.value(&ctx, as_of).unwrap().amount();
    let pv2 = swap.value(&ctx, as_of).unwrap().amount();
    let pv3 = swap.value(&ctx, as_of).unwrap().amount();

    // Assert - should be exactly the same (deterministic)
    assert_eq!(pv1, pv2);
    assert_eq!(pv2, pv3);
}

#[test]
fn test_metrics_are_stable_under_repeated_calculations() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = swap.start_date;

    // Act
    let result1 = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result2 = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert_eq!(
        result1.measures[MetricId::Vega.as_str()],
        result2.measures[MetricId::Vega.as_str()]
    );
    assert_eq!(
        result1.measures[MetricId::Dv01.as_str()],
        result2.measures[MetricId::Dv01.as_str()]
    );
}

#[test]
fn test_payoff_is_linear_in_variance_difference() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let var_diffs = [0.01, 0.02, 0.03, 0.04];

    // Act
    let payoffs: Vec<f64> = var_diffs
        .iter()
        .map(|&diff| swap.payoff(swap.strike_variance + diff).amount())
        .collect();

    // Assert - should be exactly linear
    for i in 1..payoffs.len() {
        let ratio = payoffs[i] / payoffs[0];
        let expected_ratio = var_diffs[i] / var_diffs[0];
        assert!((ratio - expected_ratio).abs() < EPSILON);
    }
}

// ============================================================================
// As-Of Date Validation Tests
// ============================================================================

#[test]
fn test_valuation_fails_when_as_of_before_discount_curve_base() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    // Try to value before the discount curve base date (2024-12-01)
    let as_of = date(2024, 11, 30);

    // Act
    let result = swap.value(&ctx, as_of);

    // Assert - should return validation error
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("Validation"));
    assert!(err_msg.contains("base date"));
}

#[test]
fn test_price_with_metrics_fails_when_as_of_before_discount_curve_base() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = date(2024, 11, 30);

    // Act
    let result = swap.price_with_metrics(
        &ctx,
        as_of,
        &[MetricId::Vega],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("Validation"));
}

#[test]
fn test_valuation_succeeds_when_as_of_equals_discount_curve_base() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date; // Equals discount curve base date

    // Act
    let result = swap.value(&ctx, as_of);

    // Assert
    assert!(result.is_ok());
    assert!(result.unwrap().amount().is_finite());
}

// ============================================================================
// CashflowProvider Tests
// ============================================================================

#[test]
fn test_cashflow_schedule_is_empty_placeholder() {
    use finstack_quant_cashflows::CashflowProvider;

    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let schedule = swap.cashflow_schedule(&ctx, as_of).unwrap();
    let flows = swap.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    assert!(schedule.flows.is_empty());
    assert_eq!(
        schedule.meta.representation,
        finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
    );
    assert!(flows.is_empty());
}

#[test]
fn test_cashflow_schedule_preserves_currency() {
    use finstack_quant_cashflows::CashflowProvider;

    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(DEFAULT_NOTIONAL, Currency::EUR);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let schedule = swap.cashflow_schedule(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(schedule.notional.initial.currency(), Currency::EUR);
}

#[test]
fn test_cashflow_schedule_has_no_synthetic_zero_amount_before_settlement() {
    use finstack_quant_cashflows::CashflowProvider;

    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let flows = swap.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - variance swaps no longer emit a synthetic zero payoff placeholder.
    assert!(flows.is_empty());
}

// ============================================================================
// Boundary Conditions
// ============================================================================

#[test]
fn test_time_elapsed_fraction_boundary_at_start() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let before = swap.time_elapsed_fraction(swap.start_date - time::Duration::days(1));
    let at = swap.time_elapsed_fraction(swap.start_date);
    let after = swap.time_elapsed_fraction(swap.start_date + time::Duration::days(1));

    // Assert
    assert_eq!(before, 0.0);
    assert_eq!(at, 0.0);
    assert!(after > 0.0);
}

#[test]
fn test_time_elapsed_fraction_boundary_at_maturity() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let before = swap.time_elapsed_fraction(swap.maturity - time::Duration::days(1));
    let at = swap.time_elapsed_fraction(swap.maturity);
    let after = swap.time_elapsed_fraction(swap.maturity + time::Duration::days(1));

    // Assert
    assert!(before < 1.0);
    assert_eq!(at, 1.0);
    assert_eq!(after, 1.0);
}

#[test]
fn test_observation_weight_boundaries() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let before_start = observation_weight(&swap, swap.start_date - time::Duration::days(1));
    let at_start = observation_weight(&swap, swap.start_date);
    let at_maturity = observation_weight(&swap, swap.maturity);
    let after_maturity = observation_weight(&swap, swap.maturity + time::Duration::days(1));

    // Assert
    assert_eq!(before_start, 0.0);
    assert!(at_start >= 0.0);
    assert_eq!(at_maturity, 1.0);
    assert_eq!(after_maturity, 1.0);
}
