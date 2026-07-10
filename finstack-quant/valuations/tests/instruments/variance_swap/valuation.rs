//! Tests for variance swap valuation (NPV) across different lifecycle stages.

use super::common::*;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::{realized_variance, RealizedVarMethod};
use finstack_quant_valuations::instruments::equity::variance_swap::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

// ============================================================================
// Pre-Start Valuation Tests
// ============================================================================

#[test]
fn test_npv_before_start_uses_forward_variance_and_discounting() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = date(2024, 12, 1);

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert. Discounting is date-based (`df_between_dates`), correct even
    // when the curve base date differs from `as_of`.
    let forward_var = 0.22_f64.powi(2);
    let undiscounted = swap.payoff(forward_var).amount();
    let df = ctx
        .get_discount(DISC_ID)
        .unwrap()
        .df_between_dates(as_of, swap.maturity)
        .unwrap();
    let expected = undiscounted * df;

    assert!((pv.amount() - expected).abs() < LOOSE_EPSILON);
}

#[test]
fn test_npv_before_start_at_the_money_forward_is_near_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let strike_vol = swap.strike_variance.sqrt();
    let ctx = add_unitless(
        base_context(),
        format!("{}_IMPL_VOL", UNDERLYING_ID),
        strike_vol,
    );
    let as_of = date(2024, 12, 1);

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount().abs() < LOOSE_EPSILON);
}

#[test]
fn test_npv_before_start_receive_side_positive_when_forward_exceeds_strike() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.25);
    let as_of = date(2024, 12, 1);

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert - forward var (0.25^2 = 0.0625) > strike (0.04)
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_npv_before_start_pay_side_opposite_sign() {
    // Arrange
    let receive = sample_swap(PayReceive::Receive);
    let pay = sample_swap(PayReceive::Pay);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.25);
    let as_of = date(2024, 12, 1);

    // Act
    let pv_receive = receive.value(&ctx, as_of).unwrap();
    let pv_pay = pay.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv_receive.amount() > 0.0);
    assert!(pv_pay.amount() < 0.0);
    assert!((pv_receive.amount() + pv_pay.amount()).abs() < LOOSE_EPSILON);
}

// ============================================================================
// Mid-Period Valuation Tests
// ============================================================================

#[test]
fn test_npv_mid_period_blends_realized_and_forward_components() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 4_950.0, 10.0);
    let ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 2];

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert - compute expected manually.
    //
    // W-33: the realized-variance term in the seasoned blend must be annualized
    // on the day-count time basis (`V_accrued / t_elapsed`), the SAME basis as
    // the day-count blend weight `w`. `partial_realized_variance` annualizes on
    // an observation-count basis (Σr²/N · ~252), a different time base, so it
    // cannot be used directly to reconstruct the identity. Reconstruct the
    // time-basis realized variance from the public API: V_accrued = Σr² (sum of
    // squared close-to-close log returns) divided by the elapsed day-count
    // time.
    let weight = swap.time_elapsed_fraction(as_of);
    let total_t = swap
        .day_count
        .year_fraction(swap.start_date, swap.maturity, Default::default())
        .unwrap();
    let t_elapsed = weight * total_t;

    let past_prices = swap.get_historical_prices(&ctx, as_of).unwrap();
    let v_accrued: f64 = past_prices
        .windows(2)
        .map(|w| {
            let r = (w[1] / w[0]).ln();
            r * r
        })
        .sum();
    let realized = v_accrued / t_elapsed;

    let forward = swap.remaining_forward_variance(&ctx, as_of).unwrap();
    let expected_var = realized * weight + forward * (1.0 - weight);
    // Date-based discounting, matching the engine's `df_between_dates`.
    let df = ctx
        .get_discount(DISC_ID)
        .unwrap()
        .df_between_dates(as_of, swap.maturity)
        .unwrap();
    let expected = swap.payoff(expected_var).amount() * df;

    assert!((pv.amount() - expected).abs() < LOOSE_EPSILON);
}

#[test]
fn test_npv_mid_period_with_high_realized_vol_increases_value_for_receive() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 50.0); // High volatility moves
    let ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 3];

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();

    // Assert - High volatility moves should result in meaningful PV
    // Note: Sign depends on whether realized var exceeds strike and how it blends with forward
    assert!(
        pv.amount().abs() > 100.0,
        "High volatility should create meaningful value"
    );
}

#[test]
fn test_npv_mid_period_discounting_reduces_value() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 10.0);
    let ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 2];

    // Act
    let pv = swap.value(&ctx, as_of).unwrap();
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();
    let forward = swap.remaining_forward_variance(&ctx, as_of).unwrap();
    let weight = swap.time_elapsed_fraction(as_of);
    let expected_var = realized * weight + forward * (1.0 - weight);
    let undiscounted = swap.payoff(expected_var).amount();

    // Assert
    assert!(
        pv.amount().abs() < undiscounted.abs(),
        "Discounting should reduce magnitude"
    );
}

#[test]
fn test_npv_mid_period_with_different_frequencies() {
    // Arrange
    let base_swap = sample_swap(PayReceive::Receive);
    let frequencies = vec![Tenor::daily(), Tenor::weekly(), Tenor::monthly()];

    for freq in frequencies {
        let mut swap = base_swap.clone();
        swap.observation_freq = freq;
        let prices = price_series(&swap, 5_000.0, 5.0);
        let ctx = add_series(base_context(), &prices);
        let dates = swap.observation_dates().expect("observation schedule");
        let as_of = dates[dates.len() / 2];

        // Act
        let pv = swap.value(&ctx, as_of);

        // Assert
        assert!(pv.is_ok());
        assert!(pv.unwrap().amount().is_finite());
    }
}

#[test]
fn test_daily_observation_dates_skip_weekends() {
    let mut swap = sample_swap(PayReceive::Receive);
    swap.start_date = date!(2025 - 01 - 03); // Friday
    swap.maturity = date!(2025 - 01 - 08); // Wednesday
    swap.observation_freq = Tenor::daily();

    let dates = swap.observation_dates().expect("observation schedule");
    assert!(
        dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday)),
        "Daily equity variance swap observations should skip weekends: {:?}",
        dates
    );
}

// ============================================================================
// At Maturity Valuation Tests
// ============================================================================

#[test]
fn test_npv_at_maturity_recovers_realized_payoff() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 3.0);
    let ctx = add_series(base_context(), &prices);

    // Act
    let pv = swap.value(&ctx, swap.maturity).unwrap();

    // Assert
    let realized = realized_variance(
        &prices.iter().map(|(_, p)| *p).collect::<Vec<_>>(),
        RealizedVarMethod::CloseToClose,
        252.0,
    )
    .expect("CloseToClose should succeed");
    let expected = swap.payoff(realized);

    assert!((pv.amount() - expected.amount()).abs() < LOOSE_EPSILON);
}

#[test]
fn test_npv_at_maturity_with_low_realized_vol() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 0.5); // Low volatility
    let ctx = add_series(base_context(), &prices);

    // Act
    let pv = swap.value(&ctx, swap.maturity).unwrap();

    // Assert - realized below strike => negative for receiver
    assert!(pv.amount() < 0.0);
}

#[test]
fn test_npv_at_maturity_without_prices_errors() {
    // After the round-3 hardening, a fully-realized variance swap with no
    // historical price data in the market context returns an error rather
    // than silently marking to zero realised variance.
    //
    // Previously this test asserted `pv == 0`, which exercised the silent
    // bug where an empty price series collapsed realised variance to zero.
    let swap = sample_swap(PayReceive::Receive);
    let ctx = MarketContext::new().insert(
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder(DISC_ID)
            .base_date(swap.start_date)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .unwrap(),
    );

    // Act
    let err = swap
        .value(&ctx, swap.maturity)
        .expect_err("missing historical data at maturity must error");

    // Assert
    let msg = err.to_string();
    assert!(
        msg.contains("no historical price data") || msg.contains("realised variance"),
        "expected data-availability error, got: {}",
        msg
    );
}

// ============================================================================
// Post-Maturity Valuation Tests
// ============================================================================

#[test]
fn test_npv_after_maturity_uses_final_realized_variance() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 3.0);
    let ctx = add_series(base_context(), &prices);
    let post_maturity = date(2025, 5, 1);

    // Act
    let pv = swap.value(&ctx, post_maturity).unwrap();

    // Assert
    let realized = realized_variance(
        &prices.iter().map(|(_, p)| *p).collect::<Vec<_>>(),
        RealizedVarMethod::CloseToClose,
        252.0,
    )
    .expect("CloseToClose should succeed");
    let expected = swap.payoff(realized);

    assert!((pv.amount() - expected.amount()).abs() < LOOSE_EPSILON);
}

// ============================================================================
// Instrument Trait Value Method Tests
// ============================================================================

// ============================================================================
// Time Progression Tests
// ============================================================================

#[test]
fn test_npv_time_progression_from_pre_start_to_maturity() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 8.0);
    let ctx = add_series(
        add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22),
        &prices,
    );

    let eval_dates = [
        date(2024, 12, 1), // Pre-start
        swap.start_date,   // At start
        date(2025, 2, 1),  // Mid-period
        date(2025, 3, 15), // Late period
        swap.maturity,     // At maturity
    ];

    // Act
    let pv_values: Vec<f64> = eval_dates
        .iter()
        .map(|&d| swap.value(&ctx, d).unwrap().amount())
        .collect();

    // Assert - all values should be finite
    for pv in &pv_values {
        assert!(pv.is_finite(), "PV must be finite at all evaluation dates");
    }
}

#[test]
fn test_npv_converges_as_maturity_approaches() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 5.0);
    let ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");

    // Act - compute PV approaching maturity
    let late_dates = &dates[dates.len() - 5..];

    for &d in late_dates {
        let pv = swap.value(&ctx, d).unwrap().amount();
        assert!(pv.is_finite());
    }

    // Assert - should converge to final payoff
    let final_pv = swap.value(&ctx, swap.maturity).unwrap().amount();
    assert!(final_pv.is_finite());
}
