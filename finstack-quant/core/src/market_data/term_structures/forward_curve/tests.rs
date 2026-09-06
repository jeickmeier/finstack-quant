use super::*;
use crate::market_data::bumps::BumpSpec;

fn sample_forward() -> ForwardCurve {
    ForwardCurve::builder("USD-LIB3M", 0.25)
        .base_date(
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
        )
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build()
        .expect("ForwardCurve builder should succeed with valid test data")
}

#[test]
fn interpolates_rate() {
    let fc = sample_forward();
    assert!((fc.rate(0.5) - 0.035).abs() < 1e-12);
}

#[test]
fn point_average_and_discount_factor_implied_forwards_are_distinct_on_steep_curve() {
    let fc = ForwardCurve::builder("USD-LIB3M", 0.25)
        .base_date(
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
        )
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("ForwardCurve builder should succeed with valid test data");
    let (t1, t2) = (0.25, 0.75);

    let point_rate = fc.rate(t1);
    let integrated_average = fc.rate_period(t1, t2);
    let df_implied_rate = fc
        .rate_between(t1, t2)
        .expect("strictly increasing finite times should produce a forward rate");

    assert!((point_rate - integrated_average).abs() > 1e-6);
    assert!((point_rate - df_implied_rate).abs() > 1e-6);
    assert!((integrated_average - df_implied_rate).abs() > 1e-6);
    assert!(
        (df_implied_rate
            - (fc.df(t1).expect("valid DF") / fc.df(t2).expect("valid DF") - 1.0) / (t2 - t1))
            .abs()
            < 1e-14
    );
    assert!(fc.rate_between(t1, t1).is_err());
    assert!(fc.rate_between(t2, t1).is_err());
}

#[test]
fn tiny_positive_intervals_preserve_finite_forward_rate() {
    let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
        )
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .build()
        .expect("flat forward curve");

    for dt in [5e-13, 1e-14, 1e-16] {
        let rate = curve
            .rate_between(0.0, dt)
            .expect("small positive interval");
        assert!(rate.is_finite());
        assert!(
            (rate - 0.05).abs() < 1e-12,
            "dt={dt}: expected 5%, got {rate}"
        );
    }
}

#[test]
fn reset_grid_preserves_off_grid_fixed_tenor_quote_meaning() {
    let t_3m = 91.0 / 360.0;
    let t_6m = 183.0 / 360.0;
    let first_rate = 0.047;
    let second_rate = 0.0485;
    let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
        )
        .day_count(DayCount::Act360)
        .knots([(0.0, first_rate), (t_3m, second_rate)])
        .projection_grid([0.0, t_3m, t_6m])
        .build()
        .expect("off-grid reset curve should build");

    assert!((curve.rate(0.0) - first_rate).abs() < 1e-14);
    assert!((curve.rate(t_3m) - second_rate).abs() < 1e-14);
    assert!(
        (curve.rate_between(0.0, t_3m).expect("first reset period") - first_rate).abs() < 1e-14
    );
    assert!(
        (curve.rate_between(t_3m, t_6m).expect("second reset period") - second_rate).abs() < 1e-14
    );
}

#[test]
fn contractual_projection_grid_survives_serde_round_trip() {
    let terminal_time = 183.0 / 360.0;
    let projection_grid = [0.0, 91.0 / 360.0, terminal_time];
    let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
        )
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.047), (91.0 / 360.0, 0.0485)])
        .projection_grid(projection_grid)
        .build()
        .expect("reset-grid curve should build");

    let json = serde_json::to_string(&curve).expect("serialize forward curve");
    let restored: ForwardCurve = serde_json::from_str(&json).expect("deserialize forward curve");

    assert_eq!(restored.projection_grid(), Some(projection_grid.as_slice()));
    assert_eq!(restored.knots(), curve.knots());
    assert_eq!(restored.forwards(), curve.forwards());
    assert!(
        (restored
            .rate_between(91.0 / 360.0, terminal_time)
            .expect("restored reset period")
            - 0.0485)
            .abs()
            < 1e-14
    );
}

#[test]
fn full_rate_calibration_retains_one_day_cutoff() {
    let json = serde_json::json!({
        "id": "USD-SOFR",
        "base": "2025-01-02",
        "reset_lag": 0,
        "day_count": "act_365f",
        "tenor": 1.0,
        "knot_points": [[0.0, 0.04], [5.0, 0.04]],
        "interp_style": "linear",
        "extrapolation": "flat_forward",
        "rate_calibration": {
            "currency": "USD",
            "method": {
                "global_solve": {
                    "use_analytical_jacobian": true
                }
            },
            "curve_day_count": "act_365f",
            "ois_compounding": {
                "compounded_with_rate_cutoff": {
                    "cutoff_days": 1
                }
            },
            "role": {
                "projection": {
                    "discount_curve_id": "USD-OIS"
                }
            },
            "quotes": [{
                "swap": {
                    "index_id": "USD-SOFR-OIS",
                    "pillar": {
                        "tenor": {
                            "count": 5,
                            "unit": "years"
                        }
                    },
                    "rate": 0.04,
                    "spread_decimal": null
                }
            }]
        },
        "fx_policy": null
    });

    let curve: ForwardCurve = serde_json::from_value(json).expect("full calibration recipe");
    let serialized = serde_json::to_value(curve).expect("serialize full recipe");
    let restored: ForwardCurve =
        serde_json::from_value(serialized.clone()).expect("round-trip full recipe");

    assert_eq!(
        serialized["rate_calibration"]["ois_compounding"]["compounded_with_rate_cutoff"]
            ["cutoff_days"],
        1
    );
    assert_eq!(
        serialized["rate_calibration"]["role"]["projection"]["discount_curve_id"],
        "USD-OIS"
    );
    let recipe = restored.rate_calibration().expect("restored recipe");
    assert!(matches!(
        recipe.ois_compounding.as_ref(),
        Some(
            crate::market_data::term_structures::RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                cutoff_days
            }
        ) if *cutoff_days == 1
    ));
}

#[test]
fn contractual_projection_grid_rejects_invalid_boundaries_and_coverage() {
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date");
    for grid in [
        vec![-0.01, 0.25, 0.5],
        vec![0.0, f64::NAN, 0.5],
        vec![0.0, 0.5, 0.25],
        vec![0.1, 0.25, 0.5],
        vec![0.0, 0.20],
    ] {
        let error = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .knots([(0.0, 0.047), (0.25, 0.048)])
            .projection_grid(grid)
            .build()
            .expect_err("invalid contractual grid must be rejected");
        assert!(error.to_string().contains("projection_grid"));
    }
}

#[test]
fn absent_projection_grid_uses_numeric_tenor_df_economics() {
    let json = serde_json::json!({
        "id": "USD-SOFR-3M",
        "base": "2025-01-01",
        "reset_lag": 2,
        "day_count": "act_360",
        "tenor": 0.25,
        "knot_points": [[0.0, 0.04], [1.0, 0.05], [5.0, 0.06]],
        "projection_grid": null,
        "interp_style": "linear",
        "extrapolation": "flat_forward",
        "rate_calibration": null,
        "fx_policy": null
    });
    let curve: ForwardCurve =
        serde_json::from_value(json).expect("canonical curve should deserialize");

    assert_eq!(curve.projection_grid(), None);
    let expected = (0..4).fold(1.0, |df, step| {
        let reset = step as f64 * 0.25;
        df / (1.0 + curve.rate(reset) * 0.25)
    });
    assert!((curve.df(1.0).expect("projection DF") - expected).abs() < 1e-14);
}

#[test]
fn failed_bump_in_place_is_atomic() {
    let mut curve = sample_forward();
    let before_forwards = curve.forwards().to_vec();
    let before_rate = curve.rate(0.5);

    let error = curve
        .bump_in_place(&BumpSpec::parallel_bp(f64::NAN))
        .expect_err("non-finite bump must fail");

    assert!(error.to_string().contains("finite"));
    assert_eq!(curve.forwards(), before_forwards.as_slice());
    assert_eq!(curve.rate(0.5).to_bits(), before_rate.to_bits());
}

// Reversed times are a caller bug: debug builds fire a `debug_assert`,
// release builds return NaN (documented NaN contract on `rate_period`).
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "rate_period requires t1 <= t2")]
fn rate_period_reversed_times_debug_asserts() {
    let fc = sample_forward();
    let _ = fc.rate_period(1.0, 0.5);
}

#[cfg(not(debug_assertions))]
#[test]
fn rate_period_reversed_times_returns_nan() {
    let fc = sample_forward();
    assert!(fc.rate_period(1.0, 0.5).is_nan());
}

#[test]
fn tail_continuity_with_flatforward_extrapolation() {
    // Test that FlatForward extrapolation maintains stable tail forwards
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
    let fc = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::FlatForward)
        .build()
        .expect("ForwardCurve builder should succeed with valid test data");

    // Rate at last knot and beyond should be continuous
    let rate_at_last = fc.rate(5.0);
    let rate_beyond = fc.rate(10.0);

    // FlatForward should maintain the rate (or slope)
    let abs_diff = (rate_beyond - rate_at_last).abs();
    assert!(
        abs_diff < 0.01,
        "Forward rate tail discontinuity: rate_at_last={:.6}, rate_beyond={:.6}",
        rate_at_last,
        rate_beyond
    );
}

#[test]
fn default_uses_flatforward_extrapolation() {
    // Verify new market-standard default extrapolation
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
    let fc = ForwardCurve::builder("TEST", 0.25)
        .base_date(base)
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build()
        .expect("ForwardCurve builder should succeed with valid test data");

    // With FlatForward, tail rate should be stable (not zero)
    let rate_tail = fc.rate(5.0);
    assert!(
        rate_tail > 0.02,
        "Tail forward should remain positive with FlatForward: {:.6}",
        rate_tail
    );
}

#[test]
fn builder_infers_market_conventions_from_curve_id() {
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");

    let sofr_term = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build()
        .expect("USD-SOFR-3M curve should build");
    assert_eq!(sofr_term.day_count(), DayCount::Act360);
    assert_eq!(sofr_term.reset_lag(), 2);

    let sonia = ForwardCurve::builder("GBP-SONIA", 1.0 / 365.0)
        .base_date(base)
        .knots([(0.0, 0.03), (1.0, 0.035)])
        .build()
        .expect("GBP-SONIA curve should build");
    assert_eq!(sonia.day_count(), DayCount::Act365F);
    assert_eq!(sonia.reset_lag(), 0);

    let generic = ForwardCurve::builder("TEST", 0.25)
        .base_date(base)
        .knots([(0.0, 0.03), (1.0, 0.035)])
        .build()
        .expect("Generic forward curve should build");
    assert_eq!(generic.reset_lag(), 0);
}

#[test]
fn roll_forward_uses_curve_day_count() {
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
    let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.05, 0.03), (0.15, 0.035), (0.30, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("ForwardCurve builder should succeed with valid test data");

    // Roll 36 days => Act/360 year fraction = 36/360 = 0.1
    let rolled = curve.roll_forward(36).expect("roll_forward should succeed");
    let ks = rolled.knots();
    assert_eq!(
        ks.len(),
        3,
        "Rolled curve should contain a new-origin anchor and two future knots"
    );
    // Original knots were at 0.05, 0.15, 0.30
    // After rolling 0.1 years: anchor at 0, -0.05 (expired), 0.05, 0.20
    assert!(ks[0].abs() < 1e-12, "Expected a zero-time anchor");
    assert!(
        (ks[1] - 0.05).abs() < 1e-12,
        "Expected 0.15 - 0.10 = 0.05, got {}",
        ks[1]
    );
    assert!(
        (ks[2] - 0.20).abs() < 1e-12,
        "Expected 0.30 - 0.10 = 0.20, got {}",
        ks[2]
    );
}

#[test]
fn roll_forward_preserves_shaped_linear_curve() {
    let base = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
    let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.02), (1.0, 0.10), (2.0, 0.02)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid shaped forward curve");

    let rolled = curve.roll_forward(180).expect("roll should succeed");
    for t in [0.0, 0.25, 0.5, 1.0, 1.5] {
        let expected = curve.rate(t + 0.5);
        let actual = rolled.rate(t);
        assert!(
            (actual - expected).abs() < 1e-12,
            "t={t}: rolled={actual}, original shifted={expected}"
        );
    }
}
