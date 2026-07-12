//! Comprehensive tests for variance swap metrics.

use super::common::*;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::{realized_variance, RealizedVarMethod};
use finstack_quant_valuations::instruments::equity::variance_swap::{PayReceive, VarianceSwap};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

/// Metric tests that value after inception use a complete realized history.
fn base_context() -> MarketContext {
    let swap = sample_swap(PayReceive::Receive);
    add_series(
        super::common::base_context(),
        &price_series(&swap, 4_900.0, 5.0),
    )
}

// ============================================================================
// Basic Metrics Tests
// ============================================================================

#[test]
fn test_variance_notional_returns_correct_amount() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::VarianceNotional],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let notional = *result
        .measures
        .get(MetricId::VarianceNotional.as_str())
        .unwrap();

    // Assert
    assert_eq!(notional, swap.notional.amount());
}

#[test]
fn vega_notional_conversion_uses_one_vol_point() {
    let variance_notional = VarianceSwap::vega_to_variance_notional(100_000.0, 0.20);
    assert!((variance_notional - 25_000_000.0).abs() < 1e-9);

    let vega_notional = VarianceSwap::variance_to_vega_notional(variance_notional, 0.20);
    assert!((vega_notional - 100_000.0).abs() < 1e-9);
}

#[test]
fn test_strike_vol_is_square_root_of_strike_variance() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::VarianceStrikeVol],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let strike_vol = *result
        .measures
        .get(MetricId::VarianceStrikeVol.as_str())
        .unwrap();

    // Assert
    assert!((strike_vol - swap.strike_variance.sqrt()).abs() < EPSILON);
}

#[test]
fn test_time_to_maturity_before_maturity_is_positive() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = swap.start_date;

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::VarianceTimeToMaturity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ttm = *result
        .measures
        .get(MetricId::VarianceTimeToMaturity.as_str())
        .unwrap();

    // Assert
    assert!(ttm > 0.0);
}

#[test]
fn test_time_to_maturity_at_maturity_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            swap.maturity,
            &[MetricId::VarianceTimeToMaturity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ttm = *result
        .measures
        .get(MetricId::VarianceTimeToMaturity.as_str())
        .unwrap();

    // Assert
    assert_eq!(ttm, 0.0);
}

#[test]
fn test_time_to_maturity_decreases_over_time() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let dates = [
        date(2024, 12, 1),
        swap.start_date,
        date(2025, 2, 1),
        date(2025, 3, 1),
        swap.maturity,
    ];

    // Act
    let ttms: Vec<f64> = dates
        .iter()
        .map(|&d| {
            swap.price_with_metrics(
                &ctx,
                d,
                &[MetricId::VarianceTimeToMaturity],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap()
            .measures[MetricId::VarianceTimeToMaturity.as_str()]
        })
        .collect();

    // Assert
    for window in ttms.windows(2) {
        assert!(window[0] >= window[1], "TTM must decrease over time");
    }
}

// ============================================================================
// Realized and Expected Variance Metrics Tests
// ============================================================================

#[test]
fn test_realized_variance_before_start_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = base_context();
    let as_of = date(2024, 12, 1);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealizedVariance],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let rv = *result
        .measures
        .get(MetricId::RealizedVariance.as_str())
        .unwrap();

    // Assert
    assert_eq!(rv, 0.0);
}

#[test]
fn test_realized_variance_matches_series_calculation() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices: Vec<(finstack_quant_core::dates::Date, f64)> = swap
        .observation_dates()
        .expect("observation schedule")
        .into_iter()
        .map(|d| (d, 4_900.0 + (d.ordinal() as f64 % 30.0)))
        .collect();
    let ctx = add_series(base_context(), &prices);
    let as_of = date(2025, 2, 1);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealizedVariance],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let rv = *result
        .measures
        .get(MetricId::RealizedVariance.as_str())
        .unwrap();

    // Assert - RealizedVarianceCalculator uses swap's annualization (252 trading days for equity)
    let annualization_factor = swap.annualization_factor_with_policy(&ctx);

    let used_prices: Vec<f64> = prices
        .iter()
        .filter(|(d, _)| *d >= swap.start_date && *d <= as_of)
        .map(|(_, p)| *p)
        .collect();
    let manual = realized_variance(
        &used_prices,
        RealizedVarMethod::CloseToClose,
        annualization_factor,
    )
    .expect("CloseToClose should succeed");

    assert!((rv - manual).abs() < EPSILON);
}

#[test]
fn test_expected_variance_before_start_uses_implied_vol() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = date(2024, 12, 1);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedVariance],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ev = *result
        .measures
        .get(MetricId::ExpectedVariance.as_str())
        .unwrap();

    // Assert
    assert!((ev - 0.22_f64.powi(2)).abs() < EPSILON);
}

#[test]
fn test_expected_variance_blends_realized_and_forward_mid_period() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices: Vec<(finstack_quant_core::dates::Date, f64)> = swap
        .observation_dates()
        .expect("observation schedule")
        .into_iter()
        .map(|d| (d, 4_950.0 + (d.ordinal() as f64 % 20.0)))
        .collect();
    let as_of = swap.start_date + time::Duration::days(28);
    let ctx = add_unitless(
        add_series(base_context(), &prices),
        format!("{}_IMPL_VOL", UNDERLYING_ID),
        0.23,
    );

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedVariance],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ev = *result
        .measures
        .get(MetricId::ExpectedVariance.as_str())
        .unwrap();

    // Assert - the metric is the seasoned mark-to-market expected variance: the
    // day-count time-weighted blend of realized-to-date and remaining forward
    // variance that the swap is actually priced on (W-33). It must equal that
    // shared quantity end-to-end through `price_with_metrics`.
    let expected = swap
        .seasoned_expected_variance(&ctx, as_of)
        .expect("seasoned expected variance");
    assert!((ev - expected).abs() < LOOSE_EPSILON);
}

// ============================================================================
// Vega and Variance Vega Tests
// ============================================================================

#[test]
fn test_vega_matches_formula() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let as_of = swap.start_date + time::Duration::days(21);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.25);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega = *result.measures.get(MetricId::Vega.as_str()).unwrap();

    // Assert
    // Vega must differentiate the same PV as `compute_pv`, which uses the
    // day-count `time_elapsed_fraction` (W-32), not an observation-count weight.
    // PV vega differentiates the remaining forward-variance leg, so its
    // volatility base is the current forward vol. Strike vol is reserved for
    // quoted-notional conversion.
    let remaining_fraction = 1.0 - swap.time_elapsed_fraction(as_of);
    let df = ctx
        .get_discount(DISC_ID)
        .unwrap()
        .df_between_dates(as_of, swap.maturity)
        .unwrap();
    let forward_vol = swap.remaining_forward_variance(&ctx, as_of).unwrap().sqrt();
    let expected = df
        * 2.0
        * swap.notional.amount()
        * forward_vol
        * 0.01
        * remaining_fraction
        * swap.side.sign();

    assert!((vega - expected).abs() < LOOSE_EPSILON);
}

/// The vega / variance-vega chain-rule identity uses the current remaining
/// forward volatility because both metrics differentiate the PV axis.
#[test]
fn test_vega_and_variance_vega_satisfy_chain_rule_identity() {
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let as_of = swap.start_date + time::Duration::days(21);
    // Forward implied vol deliberately different from the strike vol (0.20)
    // so a forward-vol-anchored vega would break the identity.
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.35);

    let opts = finstack_quant_valuations::instruments::PricingOptions::default();
    let vega = swap
        .price_with_metrics(&ctx, as_of, &[MetricId::Vega], opts.clone())
        .unwrap()
        .measures[MetricId::Vega.as_str()];
    let variance_vega = swap
        .price_with_metrics(&ctx, as_of, &[MetricId::VarianceVega], opts)
        .unwrap()
        .measures[MetricId::VarianceVega.as_str()];

    let forward_vol = swap.remaining_forward_variance(&ctx, as_of).unwrap().sqrt();
    let implied = variance_vega * 2.0 * forward_vol * 0.01;
    assert!(
        (vega - implied).abs() <= LOOSE_EPSILON * vega.abs().max(1.0),
        "vega ({vega}) must equal variance_vega * 2σ_forward * 0.01 ({implied})"
    );
}

#[test]
fn test_vega_sign_matches_swap_side() {
    // Arrange
    let receive = sample_swap(PayReceive::Receive);
    let pay = sample_swap(PayReceive::Pay);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.25);
    let as_of = receive.start_date;

    // Act
    let vega_receive = receive
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures[MetricId::Vega.as_str()];
    let vega_pay = pay
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures[MetricId::Vega.as_str()];

    // Assert
    assert!(vega_receive > 0.0);
    assert!(vega_pay < 0.0);
}

#[test]
fn test_variance_vega_matches_formula() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let as_of = swap.start_date + time::Duration::days(21);
    let ctx = base_context();

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::VarianceVega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let var_vega = *result
        .measures
        .get(MetricId::VarianceVega.as_str())
        .unwrap();

    // Assert
    // Variance vega uses the day-count `time_elapsed_fraction` (W-32) to stay
    // consistent with `compute_pv`, not an observation-count weight.
    let remaining_fraction = 1.0 - swap.time_elapsed_fraction(as_of);
    let df = ctx
        .get_discount(DISC_ID)
        .unwrap()
        .df_between_dates(as_of, swap.maturity)
        .unwrap();
    let expected = df * swap.notional.amount() * remaining_fraction * swap.side.sign();

    assert!((var_vega - expected).abs() < LOOSE_EPSILON);
}

#[test]
fn test_vega_decreases_as_maturity_approaches() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.25);
    let dates = swap.observation_dates().expect("observation schedule");

    // Act
    let vegas: Vec<f64> = dates
        .iter()
        .map(|&d| {
            swap.price_with_metrics(
                &ctx,
                d,
                &[MetricId::Vega],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap()
            .measures[MetricId::Vega.as_str()]
        })
        .collect();

    // Assert - magnitude should decrease
    for window in vegas.windows(2) {
        assert!(
            window[0].abs() >= window[1].abs(),
            "Vega magnitude should decrease"
        );
    }
}

// ============================================================================
// DV01 Tests
// ============================================================================

#[test]
fn test_dv01_matches_bump_and_reprice() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let as_of = date(2025, 1, 10);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.20);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get(MetricId::Dv01.as_str()).unwrap();

    // Assert - use bump-and-reprice validation
    // Bump the discount curve by 1bp and verify DV01 matches the PV change
    use finstack_quant_core::market_data::bumps::MarketBump;
    use finstack_quant_core::market_data::context::BumpSpec;

    let base_pv = swap.value(&ctx, as_of).unwrap().amount();
    let bumped_ctx = ctx
        .bump([MarketBump::Curve {
            id: swap.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .unwrap();
    let bumped_pv = swap.value(&bumped_ctx, as_of).unwrap().amount();
    let expected_dv01 = bumped_pv - base_pv;

    // DV01 should match the actual PV change from a 1bp bump
    assert!((dv01 - expected_dv01).abs() < LOOSE_EPSILON);
}

#[test]
fn test_dv01_at_maturity_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let prices = price_series(&swap, 5_000.0, 3.0);
    let ctx = add_series(base_context(), &prices);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            swap.maturity,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get(MetricId::Dv01.as_str()).unwrap();

    // Assert
    assert_eq!(dv01, 0.0);
}

#[test]
fn test_dv01_decreases_as_maturity_approaches() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let dates = [
        date(2024, 12, 1),
        swap.start_date,
        date(2025, 2, 1),
        swap.maturity,
    ];

    // Act
    let dv01s: Vec<f64> = dates
        .iter()
        .map(|&d| {
            swap.price_with_metrics(
                &ctx,
                d,
                &[MetricId::Dv01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap()
            .measures[MetricId::Dv01.as_str()]
        })
        .collect();

    // Assert - magnitude should decrease
    for window in dv01s.windows(2) {
        assert!(
            window[0].abs() >= window[1].abs(),
            "DV01 magnitude should decrease"
        );
    }
}

// ============================================================================
// Combined Metrics Tests
// ============================================================================

#[test]
fn test_all_metrics_pre_start() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.22);
    let as_of = date(2024, 12, 1);

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Vega,
                MetricId::Dv01,
                MetricId::VarianceVega,
                MetricId::ExpectedVariance,
                MetricId::RealizedVariance,
                MetricId::VarianceNotional,
                MetricId::VarianceStrikeVol,
                MetricId::VarianceTimeToMaturity,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - all metrics should be present and finite
    assert!(result.measures.contains_key(MetricId::Vega.as_str()));
    assert!(result.measures.contains_key(MetricId::Dv01.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::VarianceVega.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::ExpectedVariance.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::RealizedVariance.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::VarianceNotional.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::VarianceStrikeVol.as_str()));
    assert!(result
        .measures
        .contains_key(MetricId::VarianceTimeToMaturity.as_str()));

    for (_, &value) in result.measures.iter() {
        assert!(value.is_finite(), "All metrics must be finite");
    }
}

#[test]
fn test_all_metrics_mid_period() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let prices = price_series(&swap, 5_000.0, 5.0);
    let ctx = add_series(
        add_unitless(base_context(), format!("{}_IMPL_VOL", UNDERLYING_ID), 0.23),
        &prices,
    );
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 2];

    // Act
    let result = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Vega,
                MetricId::Dv01,
                MetricId::VarianceVega,
                MetricId::ExpectedVariance,
                MetricId::RealizedVariance,
                MetricId::VarianceNotional,
                MetricId::VarianceStrikeVol,
                MetricId::VarianceTimeToMaturity,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - all metrics should be present, finite, and reasonable
    for (_, &value) in result.measures.iter() {
        assert!(value.is_finite(), "All metrics must be finite");
    }

    // Realized variance should be positive mid-period
    assert!(result.measures[MetricId::RealizedVariance.as_str()] >= 0.0);
}
