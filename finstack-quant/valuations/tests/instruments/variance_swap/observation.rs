//! Tests for observation dates, annualization factors, and time fractions.

use super::common::*;
use finstack_quant_core::dates::Tenor;
use finstack_quant_valuations::instruments::equity::variance_swap::PayReceive;

// Observation Dates Tests

#[test]
fn test_observation_dates_includes_start_and_maturity() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let dates = swap.observation_dates().expect("observation schedule");

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
    let dates = swap.observation_dates().expect("observation schedule");

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
    swap.observation_frequency = Tenor::daily();

    // Act
    let dates = swap.observation_dates().expect("observation schedule");

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
    swap.observation_frequency = Tenor::weekly();

    // Act
    let dates = swap.observation_dates().expect("observation schedule");

    // Assert
    // 3 months ~= 13 weeks
    assert!(dates.len() >= 10 && dates.len() <= 15);
}

#[test]
fn test_observation_dates_monthly_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::monthly();

    // Act
    let dates = swap.observation_dates().expect("observation schedule");

    // Assert
    // Start to end is 3 months => 4 dates (start + 3 months)
    assert!(dates.len() >= 3 && dates.len() <= 5);
}

#[test]
fn test_observation_dates_quarterly_frequency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::quarterly();

    // Act
    let dates = swap.observation_dates().expect("observation schedule");

    // Assert
    // Start to end is 3 months => at least start and end
    assert!(dates.len() >= 2);
}

#[test]
fn test_realized_variance_rejects_missing_exact_observation() {
    let swap = sample_swap(PayReceive::Receive);
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[2];
    let context = add_series(base_context(), &[(dates[0], 5_000.0), (dates[2], 5_010.0)]);

    let error = swap
        .partial_realized_variance(&context, as_of)
        .expect_err("missing contractual observation must not be carried forward");
    assert!(error.to_string().contains(&dates[1].to_string()));
}

// Annualization Factor Tests

#[test]
fn test_annualization_factor_daily_equals_252() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::daily();

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
    swap.observation_frequency = Tenor::weekly();

    // Act
    let factor = swap.annualization_factor();

    // Assert: 52 calendar weeks per year
    assert_eq!(factor, 52.0);
}

#[test]
fn test_annualization_factor_monthly_equals_12() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::monthly();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 12.0);
}

#[test]
fn test_annualization_factor_quarterly_equals_4() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::quarterly();

    // Act
    let factor = swap.annualization_factor();

    // Assert
    assert_eq!(factor, 4.0);
}

#[test]
fn test_annualization_factor_semi_annual_equals_2() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::semi_annual();

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

// Time Elapsed Fraction Tests

#[test]
fn test_time_elapsed_fraction_boundaries() {
    let swap = sample_swap(PayReceive::Receive);
    let day = time::Duration::days(1);

    assert_eq!(swap.time_elapsed_fraction(swap.start_date - day), 0.0);
    assert_eq!(swap.time_elapsed_fraction(swap.start_date), 0.0);

    let just_after_start = swap.time_elapsed_fraction(swap.start_date + day);
    assert!(just_after_start > 0.0 && just_after_start < 1.0);

    let just_before_maturity = swap.time_elapsed_fraction(swap.maturity - day);
    assert!(just_before_maturity > 0.0 && just_before_maturity < 1.0);

    assert_eq!(swap.time_elapsed_fraction(swap.maturity), 1.0);
    assert_eq!(swap.time_elapsed_fraction(swap.maturity + day), 1.0);
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
    swap.day_count = finstack_quant_core::dates::DayCount::Act365F;
    let frac_actual = swap.time_elapsed_fraction(as_of);

    swap.day_count = finstack_quant_core::dates::DayCount::Thirty360;
    let frac_30_360 = swap.time_elapsed_fraction(as_of);

    // Assert
    assert!(frac_actual > 0.0 && frac_actual < 1.0);
    assert!(frac_30_360 > 0.0 && frac_30_360 < 1.0);
    assert!(
        (frac_actual - frac_30_360).abs() > 1e-4,
        "actual and 30/360 conventions should produce distinct elapsed fractions"
    );
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

// Realized Fraction by Observations Tests

#[test]
fn test_realized_fraction_by_observations_boundaries() {
    let swap = sample_swap(PayReceive::Receive);
    let day = time::Duration::days(1);

    let cases = [
        (swap.start_date - day, 0.0),
        (swap.start_date, 0.0),
        (swap.maturity, 1.0),
        (swap.maturity + day, 1.0),
    ];

    for (as_of, expected) in cases {
        let fraction = swap
            .realized_fraction_by_observations(as_of)
            .expect("observation fraction");
        assert_eq!(fraction, expected, "unexpected fraction at {as_of}");
    }
}

#[test]
fn test_realized_fraction_by_observations_increases_with_time() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::weekly();
    let dates = swap.observation_dates().expect("observation schedule");
    let mid_idx = dates.len() / 2;
    let mid_date = dates[mid_idx];

    // Act
    let frac_start = swap
        .realized_fraction_by_observations(swap.start_date)
        .expect("start fraction");
    let frac_mid = swap
        .realized_fraction_by_observations(mid_date)
        .expect("mid fraction");
    let frac_end = swap
        .realized_fraction_by_observations(swap.maturity)
        .expect("maturity fraction");

    // Assert
    assert!(frac_start < frac_mid);
    assert!(frac_mid < frac_end);
}

#[test]
fn test_realized_fraction_by_observations_matches_observation_count() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.observation_frequency = Tenor::weekly();
    let dates = swap.observation_dates().expect("observation schedule");
    let as_of = dates[dates.len() / 2];

    // Act
    let fraction = swap
        .realized_fraction_by_observations(as_of)
        .expect("observation fraction");
    let manual_frac =
        (dates.iter().filter(|&&d| d <= as_of).count() - 1) as f64 / (dates.len() - 1) as f64;

    // Assert
    assert!((fraction - manual_frac).abs() < EPSILON);
}

#[test]
fn realized_ohlc_fraction_includes_first_observed_bar() {
    let mut swap = sample_swap(PayReceive::Receive);
    swap.start_date = time::macros::date!(2025 - 01 - 06);
    swap.realized_var_method = finstack_quant_core::math::stats::RealizedVarMethod::Parkinson;
    let dates = swap.observation_dates().expect("schedule");
    let actual = swap
        .realized_fraction_by_observations(dates[0])
        .expect("fraction");
    assert!((actual - 1.0 / dates.len() as f64).abs() < EPSILON);
}
