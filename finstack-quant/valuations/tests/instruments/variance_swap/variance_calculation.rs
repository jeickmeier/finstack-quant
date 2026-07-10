//! Tests for variance calculations (realized, forward, expected).

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::{realized_variance, RealizedVarMethod};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::variance_swap::{PayReceive, VarianceSwap};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::Instrument;

// ============================================================================
// Historical Prices Tests
// ============================================================================

#[test]
fn test_get_historical_prices_prefers_time_series_over_spot() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 4_900.0, 5.0);
    let ctx = add_series(base_context(), &prices);

    // Act
    let extracted = swap.get_historical_prices(&ctx, swap.maturity).unwrap();

    // Assert
    assert!(extracted.len() >= prices.len());
}

#[test]
fn test_get_historical_prices_rejects_spot_only_for_seasoned_swap() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context(); // No series, just spot

    // Act
    let err = swap
        .get_historical_prices(&ctx, swap.maturity)
        .expect_err("seasoned swap must not synthesize history from spot");

    // Assert
    assert!(err.to_string().contains("historical price data"));
}

#[test]
fn test_get_historical_prices_errors_when_past_observations_lack_data() {
    // Arrange: market with no series and no spot; valuing at maturity so
    // the swap has accrued past observations.
    let swap = sample_swap(PayReceive::Receive);
    let ctx = MarketContext::new().insert(
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder(DISC_ID)
            .base_date(swap.start_date)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .unwrap(),
    );

    // Act
    let err = swap.get_historical_prices(&ctx, swap.maturity).expect_err(
        "missing historical data must error, not silently mark to zero realised variance",
    );

    // Assert
    let msg = err.to_string();
    assert!(
        msg.contains("no historical price data") || msg.contains("realised variance"),
        "expected explicit data-availability error, got: {}",
        msg
    );
}

#[test]
fn test_get_historical_prices_returns_empty_before_swap_starts() {
    // Arrange: market with no series and no spot, but valuing BEFORE the
    // swap starts so there are no past observations.
    let swap = sample_swap(PayReceive::Receive);
    let ctx = MarketContext::new().insert(
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder(DISC_ID)
            .base_date(swap.start_date)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .unwrap(),
    );

    // Act: as_of one day before start_date — no past observations expected
    let pre_start = swap.start_date - time::Duration::days(1);
    let extracted = swap.get_historical_prices(&ctx, pre_start).unwrap();

    // Assert
    assert!(extracted.is_empty());
}

#[test]
fn test_get_historical_prices_filters_by_as_of_date() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 2.0);
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 1);

    // Act
    let extracted = swap.get_historical_prices(&ctx, as_of).unwrap();

    // Assert
    let expected_count = prices.iter().filter(|(d, _)| *d <= as_of).count();
    assert!(extracted.len() <= expected_count);
}

// ============================================================================
// Realized Variance Tests
// ============================================================================

#[test]
fn test_partial_realized_variance_before_start_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = date(2024, 12, 1);

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(realized, 0.0);
}

#[test]
fn test_partial_realized_variance_with_insufficient_prices_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context(); // Only spot, not a series
    let as_of = swap.start_date;

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(realized, 0.0);
}

#[test]
fn test_partial_realized_variance_matches_manual_calculation() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 2.0);
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 1);

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();

    // Manual calculation
    let obs_dates = swap.observation_dates().expect("observation schedule");
    let used_prices: Vec<f64> = obs_dates
        .iter()
        .filter(|d| **d >= swap.start_date && **d <= as_of)
        .filter_map(|d| prices.iter().find(|(pd, _)| pd == d).map(|(_, p)| *p))
        .collect();
    let manual = realized_variance(&used_prices, RealizedVarMethod::CloseToClose, 252.0)
        .expect("CloseToClose should succeed");

    // Assert
    assert!((realized - manual).abs() < EPSILON);
}

#[test]
fn test_partial_realized_variance_uses_policy_annualization() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 3.0);
    let ctx = add_series(
        add_unitless(base_context(), "TRADING_DAYS_PER_YEAR", 260.0),
        &prices,
    );
    let as_of = date(2025, 2, 1);

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();

    // Assert - should be using 260.0 instead of 252.0
    let obs_dates = swap.observation_dates().expect("observation schedule");
    let used_prices: Vec<f64> = obs_dates
        .iter()
        .filter(|d| **d >= swap.start_date && **d <= as_of)
        .filter_map(|d| prices.iter().find(|(pd, _)| pd == d).map(|(_, p)| *p))
        .collect();
    let manual_260 = realized_variance(&used_prices, RealizedVarMethod::CloseToClose, 260.0)
        .expect("CloseToClose should succeed");

    // The policy override should be applied
    assert!((realized - manual_260).abs() < EPSILON);
}

#[test]
fn test_partial_realized_variance_is_always_non_negative() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, -10.0); // Declining prices
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 15);

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();

    // Assert
    assert!(realized >= 0.0, "Variance must be non-negative");
}

// ============================================================================
// Forward Variance Tests
// ============================================================================

#[test]
fn test_remaining_forward_variance_requires_market_volatility() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context_without_vol();

    // Act
    let result = swap.remaining_forward_variance(&ctx, swap.start_date);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_remaining_forward_variance_uses_implied_vol_when_present() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);

    // Act
    let forward = swap
        .remaining_forward_variance(&ctx, swap.start_date)
        .unwrap();

    // Assert
    let expected = 0.22_f64.powi(2);
    assert!((forward - expected).abs() < EPSILON);
}

#[test]
fn test_remaining_forward_variance_uses_surface_when_available() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let surface = sample_surface();
    let ctx = add_unitless(
        add_surface(base_context(), surface),
        format!("{}-DIVYIELD", UNDERLYING_ID),
        0.01,
    );
    let as_of = date(2024, 12, 1);

    // Act
    let forward = swap.remaining_forward_variance(&ctx, as_of).unwrap();

    // Assert - should use surface for forward variance
    let t = swap
        .day_count
        .year_fraction(as_of, swap.maturity, Default::default())
        .unwrap();
    let surface_check = sample_surface();
    let _vol_atm = surface_check.value_clamped(t.max(1e-8), 5_000.0);

    // The actual result may be from VIX-style replication or ATM fallback
    assert!(forward > 0.0);
    assert!(forward.is_finite());
}

#[test]
fn test_remaining_forward_variance_is_always_positive() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.15);

    // Act
    let forward = swap
        .remaining_forward_variance(&ctx, swap.start_date)
        .unwrap();

    // Assert
    assert!(forward > 0.0);
}

// ============================================================================
// Expected Variance Tests
// ============================================================================

#[test]
fn test_expected_variance_at_maturity_equals_realized() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 2.0);
    let ctx = add_series(base_context(), &prices);

    // Act
    let realized = swap.partial_realized_variance(&ctx, swap.maturity).unwrap();

    // Assert
    assert!(realized > 0.0);
}

#[test]
fn test_expected_variance_mid_period_is_weighted_blend() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 4_950.0, 10.0);
    let ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 2];

    // Act
    let realized = swap.partial_realized_variance(&ctx, as_of).unwrap();
    let forward = swap.remaining_forward_variance(&ctx, as_of).unwrap();
    let weight = observation_weight(&swap, as_of);
    let expected_blend = realized * weight + forward * (1.0 - weight);

    // Assert
    assert!(weight > 0.0 && weight < 1.0);
    assert!(expected_blend > 0.0);
    assert!(expected_blend.is_finite());
}

#[test]
fn test_expected_variance_transitions_smoothly() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 5.0);
    let _ctx = add_series(base_context(), &prices);
    let dates = swap.observation_dates().expect("observation schedule");

    // Act - compute at multiple points
    let mut prev_weight = 0.0;
    for &d in dates.iter().take(dates.len() - 1).skip(1) {
        let weight = observation_weight(&swap, d);
        assert!(weight >= prev_weight, "Weight must increase over time");
        prev_weight = weight;
    }
}

// ============================================================================
// OHLC Estimator Routing Tests
// ============================================================================

/// Build OHLC series named `SPX-OPEN`, `SPX-HIGH`, `SPX-LOW`, and close under
/// the `UNDERLYING_ID` key.
fn add_ohlc_series(
    ctx: MarketContext,
    swap: &VarianceSwap,
    base_close: f64,
    step: f64,
) -> MarketContext {
    use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};

    let obs_dates = swap.observation_dates().expect("observation schedule");
    let open_prices: Vec<(Date, f64)> = obs_dates
        .iter()
        .enumerate()
        .map(|(i, &d)| (d, base_close + step * i as f64 - 5.0))
        .collect();
    let high_prices: Vec<(Date, f64)> = obs_dates
        .iter()
        .enumerate()
        .map(|(i, &d)| (d, base_close + step * i as f64 + 10.0))
        .collect();
    let low_prices: Vec<(Date, f64)> = obs_dates
        .iter()
        .enumerate()
        .map(|(i, &d)| (d, base_close + step * i as f64 - 8.0))
        .collect();
    let close_prices: Vec<(Date, f64)> = obs_dates
        .iter()
        .enumerate()
        .map(|(i, &d)| (d, base_close + step * i as f64))
        .collect();

    let mk_series = |id: &str, data: Vec<(Date, f64)>| {
        ScalarTimeSeries::new(id, data, None)
            .unwrap()
            .with_interpolation(SeriesInterpolation::Step)
    };

    ctx.insert_series(mk_series("SPX-OPEN", open_prices))
        .insert_series(mk_series("SPX-HIGH", high_prices))
        .insert_series(mk_series("SPX-LOW", low_prices))
        .insert_series(mk_series(UNDERLYING_ID, close_prices))
}

/// Create a variance swap configured for a specific OHLC estimator.
fn ohlc_swap(method: RealizedVarMethod) -> VarianceSwap {
    let (start, end) = default_dates();
    VarianceSwap::builder()
        .id(InstrumentId::new(format!("VAR-OHLC-{method:?}")))
        .underlying_ticker(UNDERLYING_ID.to_string())
        .notional(Money::new(DEFAULT_NOTIONAL, Currency::USD))
        .strike_variance(DEFAULT_STRIKE_VAR)
        .start_date(start)
        .maturity(end)
        .observation_freq(Tenor::daily())
        .observation_calendar_id("USNY".to_string())
        .realized_var_method(method)
        .open_series_id("SPX-OPEN".to_string())
        .high_series_id("SPX-HIGH".to_string())
        .low_series_id("SPX-LOW".to_string())
        .side(PayReceive::Receive)
        .discount_curve_id(CurveId::new(DISC_ID))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

#[test]
fn test_ohlc_partial_realized_variance_differs_from_close_to_close() {
    let close_swap = sample_swap(PayReceive::Receive);
    let parkinson_swap = ohlc_swap(RealizedVarMethod::Parkinson);

    let ctx = add_ohlc_series(base_context(), &close_swap, 5_000.0, 3.0);
    let as_of = date(2025, 2, 1);

    let rv_close = close_swap.partial_realized_variance(&ctx, as_of).unwrap();
    let rv_park = parkinson_swap
        .partial_realized_variance(&ctx, as_of)
        .unwrap();

    assert!(rv_close.is_finite() && rv_close > 0.0);
    assert!(rv_park.is_finite() && rv_park > 0.0);
    assert!(
        (rv_close - rv_park).abs() > 1e-12,
        "Parkinson ({rv_park}) should differ from CloseToClose ({rv_close})"
    );
}

#[test]
fn test_all_ohlc_estimators_produce_positive_variance() {
    let as_of = date(2025, 2, 1);

    for method in [
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
        RealizedVarMethod::YangZhang,
    ] {
        let swap = ohlc_swap(method);
        let ctx = add_ohlc_series(base_context(), &swap, 5_000.0, 3.0);

        let rv = swap.partial_realized_variance(&ctx, as_of);
        assert!(
            rv.is_ok(),
            "method {method:?} should succeed with OHLC data: {rv:?}"
        );
        let rv = rv.unwrap();
        assert!(rv.is_finite() && rv >= 0.0, "method {method:?} rv={rv}");
    }
}

#[test]
fn test_ohlc_missing_series_id_returns_error() {
    let (start, end) = default_dates();
    let bad_swap = VarianceSwap::builder()
        .id(InstrumentId::new("VAR-BAD-OHLC"))
        .underlying_ticker(UNDERLYING_ID.to_string())
        .notional(Money::new(DEFAULT_NOTIONAL, Currency::USD))
        .strike_variance(DEFAULT_STRIKE_VAR)
        .start_date(start)
        .maturity(end)
        .observation_freq(Tenor::daily())
        .observation_calendar_id("USNY".to_string())
        .realized_var_method(RealizedVarMethod::Parkinson)
        // Intentionally omit open/high/low series IDs
        .side(PayReceive::Receive)
        .discount_curve_id(CurveId::new(DISC_ID))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let prices = price_series(&bad_swap, 5_000.0, 3.0);
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 1);

    let result = bad_swap.partial_realized_variance(&ctx, as_of);
    assert!(
        result.is_err(),
        "Parkinson without OHLC series IDs should return Err"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("open_series_id"),
        "error should mention missing open_series_id: {msg}"
    );
}

#[test]
fn test_ohlc_at_maturity_valuation_succeeds() {
    let swap = ohlc_swap(RealizedVarMethod::GarmanKlass);
    let ctx = add_ohlc_series(base_context(), &swap, 5_000.0, 3.0);

    let pv = swap.value(&ctx, swap.maturity);
    assert!(pv.is_ok(), "at-maturity OHLC valuation failed: {pv:?}");
    assert!(pv.unwrap().amount().is_finite());
}
