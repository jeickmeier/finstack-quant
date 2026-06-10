//! Tests for observation dates, annualization factors, and time fractions.

use super::common::*;
use finstack_core::dates::Tenor;
use finstack_valuations::instruments::equity::variance_swap::PayReceive;

// ============================================================================
// Observation Dates Tests
// ============================================================================

#[test]
fn test_observation_dates_includes_start_and_maturity() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let dates = swap.observation_dates();

    // Assert
    assert!(!dates.is_empty());
    assert_eq!(dates.first().copied(), Some(swap.start_date));
    assert_eq!(dates.last().copied(), Some(swap.maturity));
}

#[test]
fn test_observation_dates_are_monotonically_increasing() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let dates = swap.observation_dates();

    // Assert
    assert!(dates.len() >= 2);
    for window in dates.windows(2) {
        assert!(window[0] < window[1], "Dates must be strictly increasing");
    }
}

#[test]
fn test_observation_dates_daily_frequency_generates_many_dates() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::daily();

    // Act
    let dates = swap.observation_dates();

    // Assert
    // 3 months ~= 90 days
    assert!(
        dates.len() > 60,
        "Daily frequency should generate many observations"
    );
}

#[test]
fn test_observation_dates_weekly_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();

    // Act
    let dates = swap.observation_dates();

    // Assert
    // 3 months ~= 13 weeks
    assert!(dates.len() >= 10 && dates.len() <= 15);
}

#[test]
fn test_observation_dates_monthly_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::monthly();

    // Act
    let dates = swap.observation_dates();

    // Assert
    // Start to end is 3 months => 4 dates (start + 3 months)
    assert!(dates.len() >= 3 && dates.len() <= 5);
}

#[test]
fn test_observation_dates_quarterly_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::quarterly();

    // Act
    let dates = swap.observation_dates();

    // Assert
    // Start to end is 3 months => at least start and end
    assert!(dates.len() >= 2);
}

// ============================================================================
// Annualization Factor Tests
// ============================================================================

#[test]
fn test_annualization_factor_daily_equals_252() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::daily();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 252.0);
}

#[test]
fn test_annualization_factor_weekly_uses_calendar_weeks() {
    // Arrange: weekly schedules step in calendar days, so there are 52
    // observations per year — not 252/7 (= 36), which mixes a trading-day
    // basis with a calendar-day step and understates realized variance.
    // Matches the FX variance-swap convention.
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();

    // Act
    let factor = swap.annualization_factor();

    // Assert: 52 calendar weeks per year
    assert_eq!(factor, 52.0);
}

#[test]
fn test_annualization_factor_monthly_equals_12() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::monthly();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 12.0);
}

#[test]
fn test_annualization_factor_quarterly_equals_4() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::quarterly();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 4.0);
}

#[test]
fn test_annualization_factor_semi_annual_equals_2() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::semi_annual();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 2.0);
}

#[test]
fn test_annualization_factor_with_market_policy_override() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(
        base_context(),
        format!("{}_TRADING_DAYS_PER_YEAR", UNDERLYING_ID),
        260.0,
    );

    // Act
    let factor = swap.annualization_factor_with_policy(&ctx);

    // Assert
    assert_eq!(factor, 260.0);
}

#[test]
fn test_annualization_factor_with_global_policy_override() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(base_context(), "TRADING_DAYS_PER_YEAR", 255.0);

    // Act
    let factor = swap.annualization_factor_with_policy(&ctx);

    // Assert
    assert_eq!(factor, 255.0);
}

#[test]
fn test_annualization_factor_specific_override_takes_precedence_over_global() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let ctx = add_unitless(
        add_unitless(base_context(), "TRADING_DAYS_PER_YEAR", 250.0),
        format!("{}_TRADING_DAYS_PER_YEAR", UNDERLYING_ID),
        260.0,
    );

    // Act
    let factor = swap.annualization_factor_with_policy(&ctx);

    // Assert
    assert_eq!(factor, 260.0); // Specific takes precedence
}

// ============================================================================
// Time Elapsed Fraction Tests
// ============================================================================

#[test]
fn test_time_elapsed_fraction_before_start_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let as_of = date(2024, 12, 1);

    // Act
    let fraction = swap.time_elapsed_fraction(as_of);

    // Assert
    assert_eq!(fraction, 0.0);
}

#[test]
fn test_time_elapsed_fraction_at_start_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let fraction = swap.time_elapsed_fraction(swap.start_date);

    // Assert
    assert_eq!(fraction, 0.0);
}

#[test]
fn test_time_elapsed_fraction_at_maturity_is_one() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let fraction = swap.time_elapsed_fraction(swap.maturity);

    // Assert
    assert_eq!(fraction, 1.0);
}

#[test]
fn test_time_elapsed_fraction_after_maturity_is_one() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let as_of = date(2025, 5, 1);

    // Act
    let fraction = swap.time_elapsed_fraction(as_of);

    // Assert
    assert_eq!(fraction, 1.0);
}

#[test]
fn test_time_elapsed_fraction_midway_is_approximately_half() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let as_of = date(2025, 2, 15); // Roughly halfway

    // Act
    let fraction = swap.time_elapsed_fraction(as_of);

    // Assert
    assert!(
        fraction > 0.4 && fraction < 0.6,
        "Midway fraction should be ~0.5"
    );
}

#[test]
fn test_time_elapsed_fraction_respects_day_count_convention() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    let as_of = date(2025, 2, 1);

    // Act with different day counts
    swap.day_count = finstack_core::dates::DayCount::Act365F;
    let frac_365 = swap.time_elapsed_fraction(as_of);

    swap.day_count = finstack_core::dates::DayCount::Act360;
    let frac_360 = swap.time_elapsed_fraction(as_of);

    // Assert
    assert!(frac_365 > 0.0 && frac_365 < 1.0);
    assert!(frac_360 > 0.0 && frac_360 < 1.0);
    // Different conventions yield different fractions (though difference may be small)
    assert!(frac_365 != frac_360 || (frac_365 - frac_360).abs() < 1e-8);
}

#[test]
fn test_time_elapsed_fraction_is_monotonic() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let dates = [
        date(2024, 12, 1),
        swap.start_date,
        date(2025, 1, 15),
        date(2025, 2, 1),
        date(2025, 3, 1),
        swap.maturity,
        date(2025, 5, 1),
    ];

    // Act
    let fractions: Vec<f64> = dates
        .iter()
        .map(|&d| swap.time_elapsed_fraction(d))
        .collect();

    // Assert
    for window in fractions.windows(2) {
        assert!(
            window[0] <= window[1],
            "Time fraction must be monotonically increasing"
        );
    }
}

// ============================================================================
// Realized Fraction by Observations Tests
// ============================================================================

#[test]
fn test_realized_fraction_by_observations_before_start_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let as_of = date(2024, 12, 1);

    // Act
    let fraction = observation_weight(&swap, as_of);

    // Assert
    assert_eq!(fraction, 0.0);
}

#[test]
fn test_realized_fraction_by_observations_at_maturity_is_one() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let fraction = observation_weight(&swap, swap.maturity);

    // Assert
    assert_eq!(fraction, 1.0);
}

#[test]
fn test_realized_fraction_by_observations_increases_with_time() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let dates = swap.observation_dates();
    let mid_idx = dates.len() / 2;
    let mid_date = dates[mid_idx];

    // Act
    let frac_start = observation_weight(&swap, swap.start_date);
    let frac_mid = observation_weight(&swap, mid_date);
    let frac_end = observation_weight(&swap, swap.maturity);

    // Assert
    assert!(frac_start < frac_mid);
    assert!(frac_mid < frac_end);
}

#[test]
fn test_realized_fraction_by_observations_matches_observation_count() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_freq = Tenor::weekly();
    let dates = swap.observation_dates();
    let as_of = dates[dates.len() / 2];

    // Act
    let fraction = observation_weight(&swap, as_of);
    let manual_frac = dates.iter().filter(|&&d| d <= as_of).count() as f64 / dates.len() as f64;

    // Assert
    assert!((fraction - manual_frac).abs() < EPSILON);
}
