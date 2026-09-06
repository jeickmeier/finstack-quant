use super::*;
use crate::market_data::bumps::BumpSpec;

use crate::market_data::term_structures::{HazardCalibrationInput, HazardCalibrationRecipe};
use time::Month;

fn test_hazard_recipe(base: Date) -> HazardCalibrationRecipe {
    let input = HazardCalibrationInput {
        quote: serde_json::json!({
            "type": "cds_par_spread",
            "id": "CDS-5Y"
        }),
        pillar_date: base + time::Duration::days(365),
        pillar_time: 1.0,
    };
    HazardCalibrationRecipe::new(
        serde_json::json!({"curve_id": "RECIPE"}),
        vec![input.clone()],
        vec![input],
        serde_json::json!({"fail_on_bad_fit": true}),
    )
    .expect("valid hazard calibration recipe")
}

#[test]
fn hazard_recipe_invalidation_covers_synthetic_transformations() {
    use BumpSpec;

    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let source = HazardCurve::builder("RECIPE")
        .base_date(base)
        .recovery_rate(0.4)
        .knots([(1.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .hazard_calibration(test_hazard_recipe(base))
        .build()
        .expect("recipe-backed curve");

    let recovery = source.with_recovery_rate(0.35).expect("recovery override");
    assert!(recovery.hazard_calibration().is_none());

    let parallel = source
        .with_parallel_hazard_rate_bump_bp(1.0)
        .expect("parallel bump");
    assert!(parallel.hazard_calibration().is_none());

    let rolled = source.roll_forward(30).expect("curve roll");
    assert!(rolled.hazard_calibration().is_none());

    let rebuilt = source
        .to_builder_with_id("REBUILT")
        .build()
        .expect("generic rebuild");
    assert!(rebuilt.hazard_calibration().is_none());

    let mut in_place = source;
    in_place
        .bump_in_place(&BumpSpec::parallel_bp(1.0))
        .expect("in-place bump");
    assert!(in_place.hazard_calibration().is_none());
}

#[test]
fn survival_interpolation_preserves_hazard_consistency() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let builder = || {
        HazardCurve::builder("HZ")
            .base_date(base)
            .knots([(1.0, 0.02), (2.0, 1.0)])
            .recovery_rate(0.4)
    };
    assert!(builder().interp(InterpStyle::Linear).build().is_err());
    let curve = builder().build().expect("valid hazard curve");
    for t in [0.5, 1.5, 3.0] {
        let eps = 1e-5;
        let implied = -(curve.sp(t + eps).ln() - curve.sp(t - eps).ln()) / (2.0 * eps);
        assert!((implied - curve.hazard_rate(t)).abs() < 1e-9);
    }
    let mut state = serde_json::to_value(&curve).expect("serialize");
    state["survival_interp"] = serde_json::to_value(InterpStyle::Linear).expect("style");
    assert!(serde_json::from_value::<HazardCurve>(state).is_err());
}

#[test]
fn survival_monotone_decreasing() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let hc = HazardCurve::builder("USD-CREDIT")
        .base_date(base)
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .recovery_rate(0.40)
        .build()
        .expect("HazardCurve builder should succeed with valid test data");
    assert!(hc.sp(1.0) < 1.0);
    assert!(hc.sp(6.0) < hc.sp(1.0));
}

#[test]
fn default_prob_positive() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let hc = HazardCurve::builder("USD")
        .base_date(base)
        .knots([(1.0, 0.01), (10.0, 0.015)])
        .recovery_rate(0.40)
        .build()
        .expect("HazardCurve builder should succeed with valid test data");
    let dp = hc
        .default_prob(2.0, 4.0)
        .expect("default_prob should succeed with valid inputs");
    assert!(dp >= 0.0);
}

#[test]
fn zero_anchored_tail_uses_last_hazard() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("TAIL")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    let implied_tail_hazard = -(curve.sp(11.0) / curve.sp(10.0)).ln();
    assert!((implied_tail_hazard - 0.03).abs() < 1e-12);
    assert!((curve.hazard_rate(10.0) - 0.03).abs() < 1e-12);
}

#[test]
fn unanchored_hazard_changes_immediately_after_end_knot() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("BOUNDARY")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.01), (2.0, 0.02), (3.0, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    assert!((curve.hazard_rate(1.0) - 0.01).abs() < 1e-12);
    assert!((curve.hazard_rate(1.0 + 1e-12) - 0.02).abs() < 1e-12);
}

#[test]
fn roll_forward_preserves_conditional_survival() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("ROLL")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    let rolled = curve
        .roll_forward(365)
        .expect("one-year roll should succeed");
    for t in [0.5, 1.0, 4.0, 5.0, 8.0] {
        let expected = curve.sp(t + 1.0) / curve.sp(1.0);
        assert!(
            (rolled.sp(t) - expected).abs() < 1e-12,
            "t={t}: rolled={}, expected={expected}",
            rolled.sp(t)
        );
    }
}

#[test]
fn quoted_spread_interpolation_linear() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let hc = HazardCurve::builder("TEST")
        .base_date(base)
        .knots([(1.0, 0.02)])
        .par_spreads([(1.0, 100.0), (3.0, 200.0)])
        .recovery_rate(0.40)
        .build()
        .expect("HazardCurve builder should succeed with valid test data");
    assert!((hc.cds_quote_bp(2.0, ParInterp::Linear) - 150.0).abs() < 1e-9);
}

#[test]
fn roll_forward_works() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let _hc = HazardCurve::builder("TEST-ROLL")
        .base_date(base)
        .day_count(DayCount::Act365F) // Use Act365F for simple math
        .knots([(0.5, 0.01), (1.5, 0.02)])
        .recovery_rate(0.40)
        .build()
        .expect("Builder works");

    let hc = HazardCurve::builder("TEST-ROLL")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.5, 0.01), (1.5, 0.02), (2.5, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("Builder works");

    let rolled = hc.roll_forward(183).expect("Roll should succeed"); // > 0.5 years

    assert_eq!(rolled.base_date(), base + time::Duration::days(183));

    let knots: Vec<f64> = rolled.knot_points().map(|(t, _)| t).collect();
    assert_eq!(knots.len(), 3);
    assert!(
        knots[0].abs() < 1e-12,
        "rolled curve must be anchored at zero"
    );
    // 1.5 - (183/365) = 1.5 - 0.50137 = 0.9986
    // 2.5 - (183/365) = 1.9986
    assert!(knots[1] < 1.0 && knots[1] > 0.99);
}

#[test]
fn builder_allows_explicit_zero_time_knot() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let result = HazardCurve::builder("USD-CREDIT")
        .base_date(base)
        .knots([(0.0, 0.01), (5.0, 0.02)])
        .recovery_rate(0.40)
        .build();

    assert!(result.is_ok(), "t=0 hazard knots should be accepted");
}

#[test]
fn hazard_rate_is_available_for_valid_built_curves() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let hc = HazardCurve::builder("USD-CREDIT")
        .base_date(base)
        .knots([(0.0, 0.01), (5.0, 0.02)])
        .recovery_rate(0.40)
        .build()
        .expect("HazardCurve builder should succeed with valid test data");

    assert_eq!(hc.hazard_rate(-1.0), 0.01);
    assert_eq!(hc.hazard_rate(0.0), 0.01);
    assert_eq!(hc.hazard_rate(10.0), 0.02);
}

/// Regression test (2026-06-09 "Major — market data"
/// item 2): `build()` and `rebuild_interp` (the `MarketContext::bump` /
/// CS01 path via `bump_in_place`) must share one λ-segment attribution
/// convention. A zero-size bump must be an exact no-op even for curves
/// with an explicit t=0 anchor knot.
#[test]
fn zero_size_bump_in_place_is_noop_for_zero_anchored_curve() {
    use BumpSpec;

    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("ZERO-ANCHORED")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(0.0, 0.01), (1.0, 0.02), (5.0, 0.015)])
        .build()
        .expect("zero-anchored hazard curve builds");

    let mut bumped = curve.clone();
    bumped
        .bump_in_place(&BumpSpec::parallel_bp(0.0))
        .expect("zero bump succeeds");

    for t in [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0] {
        assert!(
            (bumped.sp(t) - curve.sp(t)).abs() < 1e-15,
            "zero bump must not change survival at t={t}: \
             bumped {} vs base {}",
            bumped.sp(t),
            curve.sp(t)
        );
    }
}

/// A small parallel spread bump on a zero-anchored curve must shift the
/// average hazard −ln(S(t))/t by exactly spread/(1−R) at every t inside
/// the knot range — with no spurious re-attribution of base hazards.
#[test]
fn small_bump_in_place_shifts_average_hazard_uniformly() {
    use BumpSpec;

    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let recovery = 0.40;
    let curve = HazardCurve::builder("ZERO-ANCHORED-SMALL")
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(0.0, 0.01), (1.0, 0.02), (5.0, 0.015)])
        .build()
        .expect("zero-anchored hazard curve builds");

    let spread_bp = 10.0;
    let expected_shift = (spread_bp / 10_000.0) / (1.0 - recovery);

    let mut bumped = curve.clone();
    bumped
        .bump_in_place(&BumpSpec::parallel_bp(spread_bp))
        .expect("small bump succeeds");

    for t in [0.5, 1.0, 2.0, 3.0, 5.0] {
        let base_avg = -curve.sp(t).ln() / t;
        let bumped_avg = -bumped.sp(t).ln() / t;
        assert!(
            (bumped_avg - base_avg - expected_shift).abs() < 1e-12,
            "average hazard change at t={t} must equal the bump: \
             got {}, expected {}",
            bumped_avg - base_avg,
            expected_shift
        );
    }
}

/// `bump_in_place` clears stored par-spread quotes (they were calibrated
/// to the unbumped hazards); `cds_quote_bp` then falls back to the
/// hazard-based approximation λ·(1−R)·1e4 reflecting the bumped curve.
#[test]
fn bump_in_place_clears_stale_par_spread_quotes() {
    use BumpSpec;

    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let recovery = 0.40;
    let mut curve = HazardCurve::builder("QUOTED")
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, 0.01), (5.0, 0.01)])
        .par_spreads([(1.0, 60.0), (5.0, 60.0)])
        .build()
        .expect("quoted hazard curve builds");

    curve
        .bump_in_place(&BumpSpec::parallel_bp(10.0))
        .expect("bump succeeds");

    assert_eq!(
        curve.par_spread_points().count(),
        0,
        "stale par quotes must be cleared on bump"
    );
    // Fallback quote reflects the bumped hazard: (0.01 + 0.001/0.6)·0.6·1e4 = 70bp.
    let quote = curve.cds_quote_bp(3.0, ParInterp::Linear);
    assert!(
        (quote - 70.0).abs() < 1e-9,
        "fallback quote must reflect bumped hazards, got {quote}"
    );
}

#[test]
fn direct_hazard_rate_parallel_bump_is_additive() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("DIRECT-PARALLEL")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020)])
        .par_spreads([(1.0, 60.0), (3.0, 90.0), (5.0, 120.0)])
        .build()
        .expect("valid hazard curve");

    let up = curve
        .with_parallel_hazard_rate_bump_bp(25.0)
        .expect("parallel direct shock");
    let round_trip = up
        .with_parallel_hazard_rate_bump_bp(-25.0)
        .expect("reverse direct shock");

    for (tenor, base_rate) in curve.knot_points() {
        assert!((up.hazard_rate(tenor) - base_rate - 25e-4).abs() < 1e-12);
        assert!((round_trip.hazard_rate(tenor) - base_rate).abs() < 1e-12);
    }
    assert_eq!(up.id(), curve.id());
    assert_eq!(up.par_spread_points().count(), 0);
    assert!(up.hazard_calibration().is_none());
}

#[test]
fn direct_hazard_rate_tenor_bumps_preserve_segment_matching_and_additivity() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("DIRECT-TENORS")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020), (10.0, 0.025)])
        .build()
        .expect("valid hazard curve");

    let bumped = curve
        .with_tenor_hazard_rate_bumps_bp(&[
            (4.0, 3.0),
            (4.0, 5.0),
            (3.0, 2.0),
            (10.0 + 2e-6, 100.0),
        ])
        .expect("tenor direct shocks");
    let expected = [0.010, 0.0152, 0.0208, 0.025];
    for ((tenor, rate), expected_rate) in bumped.knot_points().zip(expected) {
        assert!(
            (rate - expected_rate).abs() < 1e-12,
            "unexpected direct hazard rate at {tenor}: {rate}"
        );
    }
}

#[test]
fn tenor_hazard_bump_changes_only_the_containing_segment() {
    let curve = HazardCurve::builder("SEGMENTS")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .recovery_rate(0.4)
        .knots([(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
        .build()
        .unwrap();
    let bumped = curve
        .with_tenor_hazard_rate_bumps_bp(&[(4.0, 100.0)])
        .unwrap();
    assert_eq!(bumped.hazard_rate(2.0), curve.hazard_rate(2.0));
    assert!((bumped.hazard_rate(4.0) - 0.03).abs() < 1e-12);
    assert_eq!(bumped.hazard_rate(6.0), curve.hazard_rate(6.0));
    assert!((bumped.sp(3.0) - curve.sp(3.0)).abs() < 1e-12);
    assert!((bumped.sp(4.0) / curve.sp(4.0) - (-0.01_f64).exp()).abs() < 1e-12);
    assert!((bumped.sp(6.0) / curve.sp(6.0) - (-0.02_f64).exp()).abs() < 1e-12);
}

#[test]
fn tenor_hazard_down_bumps_reject_negative_rates() {
    let curve = HazardCurve::builder("DOWN")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .recovery_rate(0.4)
        .knots([(1.0, 0.01), (3.0, 0.015), (5.0, 0.02)])
        .build()
        .unwrap();
    assert!(curve
        .with_tenor_hazard_rate_bumps_bp(&[(3.0, -200.0)])
        .is_err());
    assert!(curve
        .with_tenor_hazard_rate_bumps_bp(&[(3.0, -100.0), (3.0, -100.0)])
        .is_err());
    assert_eq!(curve.hazard_rate(3.0), 0.015);
    let zero = curve
        .with_tenor_hazard_rate_bumps_bp(&[(1.0, -100.0)])
        .unwrap();
    assert_eq!(zero.hazard_rate(1.0), 0.0);
    let restored = zero
        .with_tenor_hazard_rate_bumps_bp(&[(1.0, 100.0)])
        .unwrap();
    assert!((restored.sp(4.0) - curve.sp(4.0)).abs() < 1e-12);
}

#[test]
fn failed_bump_in_place_is_atomic() {
    use BumpSpec;

    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let original = HazardCurve::builder("ATOMIC")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(1.0, 0.010), (5.0, 0.001)])
        .build()
        .expect("valid hazard curve");
    let mut attempted = original.clone();

    attempted
        .bump_in_place(&BumpSpec::parallel_bp(-30.0))
        .expect_err("second hazard node would become negative");

    assert_eq!(
        attempted.knot_points().collect::<Vec<_>>(),
        original.knot_points().collect::<Vec<_>>()
    );
    for t in [0.5, 1.0, 3.0, 5.0] {
        assert_eq!(attempted.sp(t), original.sp(t));
    }
}

#[test]
fn parallel_bump_rejects_negative_shift_that_crosses_zero_hazard() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("HY")
        .base_date(base)
        .knots([(1.0, 0.001), (5.0, 0.002)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    let err = curve
        .with_parallel_hazard_rate_bump_bp(-15.0)
        .expect_err("negative shifted hazard rate must be rejected");

    assert!(
        err.to_string().contains("negative hazard rate after bump"),
        "unexpected error: {err}"
    );
}
mod seniority_tests {
    use super::Seniority;

    #[test]
    fn test_seniority_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("senior_secured", Seniority::SeniorSecured),
            ("senior", Seniority::Senior),
            ("subordinated", Seniority::Subordinated),
            ("junior", Seniority::Junior),
        ] {
            assert!(matches!(input.parse::<Seniority>(), Ok(value) if value == expected));
        }

        for variant in [
            Seniority::SeniorSecured,
            Seniority::Senior,
            Seniority::Subordinated,
            Seniority::Junior,
        ] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<Seniority>(), Ok(value) if value == variant));
        }
    }

    #[test]
    fn test_seniority_fromstr_rejects_unknown() {
        for rejected in ["sub", "Senior", "senior-secured", " senior"] {
            assert!(rejected.parse::<Seniority>().is_err());
        }
    }
}
