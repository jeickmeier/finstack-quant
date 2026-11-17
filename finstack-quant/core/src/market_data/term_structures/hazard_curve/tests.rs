use super::*;
use crate::dates::{Date, DayCount};
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::market_data::term_structures::{HazardCalibrationInput, HazardCalibrationRecipe};
use time::{Duration, Month};

fn test_hazard_recipe(base: Date) -> HazardCalibrationRecipe {
    let input = HazardCalibrationInput {
        quote: serde_json::json!({
            "type": "cds_par_spread",
            "id": "CDS-5Y"
        }),
        pillar_date: base + Duration::days(365),
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
fn hazard_recipe_invalidation_and_exact_noop_preservation() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let source = HazardCurve::builder("RECIPE")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(1.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .hazard_calibration(test_hazard_recipe(base))
        .build()
        .expect("recipe-backed curve");

    let unchanged_rec = source
        .with_recovery_rate(0.4)
        .expect("exact recovery preserves calibration");
    assert_eq!(
        unchanged_rec.hazard_calibration(),
        source.hazard_calibration()
    );

    let recovery = source.with_recovery_rate(0.35).expect("recovery override");
    assert!(recovery.hazard_calibration().is_none());

    let zero_parallel = source
        .with_parallel_hazard_rate_bump_bp(0.0)
        .expect("zero bump");
    assert_eq!(
        zero_parallel.hazard_calibration(),
        source.hazard_calibration()
    );

    let parallel = source
        .with_parallel_hazard_rate_bump_bp(1.0)
        .expect("parallel bump");
    assert!(parallel.hazard_calibration().is_none());

    let zero_tenor = source
        .with_tenor_hazard_rate_bumps_bp(&[(5.0, 0.0)])
        .expect("zero exact tenor bump");
    assert_eq!(zero_tenor.hazard_calibration(), source.hazard_calibration());

    let rolled = source
        .condition(base + Duration::days(30))
        .expect("curve condition");
    assert!(rolled.hazard_calibration().is_none());

    let rebuilt = source
        .to_builder_with_id("REBUILT")
        .build()
        .expect("generic rebuild");
    assert_eq!(rebuilt.hazard_calibration(), source.hazard_calibration());

    let mut in_place = source;
    in_place
        .bump_in_place(&BumpSpec::parallel_bp(1.0))
        .expect("in-place bump");
    assert!(in_place.hazard_calibration().is_none());
}

#[test]
fn dates_before_base_date_and_invalid_numeric_queries_are_rejected() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("HZ")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.02), (5.0, 0.03)])
        .recovery_rate(0.4)
        .build()
        .expect("valid hazard curve");

    let pre = base - Duration::days(1);
    assert!(curve.sp_on_date(pre).is_err());
    assert!(curve.hazard_rate_on_date(pre).is_err());
    assert!(curve.conditional_default_prob(pre, base).is_err());
    assert!(curve.integrated_hazard_between_dates(pre, base).is_err());
    assert!(curve.survival_at_dates(&[pre, base]).is_err());

    assert!(curve.sp(-0.1).is_err());
    assert!(curve.sp(f64::NAN).is_err());
    assert!(curve.sp(f64::INFINITY).is_err());
    assert!(curve.hazard_rate(-1.0).is_err());
    assert!(curve.hazard_rate(f64::NAN).is_err());
    assert!(curve.hazard_rate(f64::INFINITY).is_err());
    assert!(curve.default_prob(-0.1, 1.0).is_err());
    assert!(curve.default_prob(2.0, 1.0).is_err());
    assert!(curve.sp_between(-0.1, 1.0).is_err());
    assert!(curve.sp_between(2.0, 1.0).is_err());
}

#[test]
fn construction_rejects_zero_time_unsorted_and_missing_clock_without_cap() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    // t = 0 rejected
    assert!(
        HazardCurve::builder("ZERO-KNOT")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(0.0, 0.01), (5.0, 0.02)])
            .build()
            .is_err()
    );
    // Unsorted rejected (no silent sort)
    assert!(
        HazardCurve::builder("UNSORTED")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(5.0, 0.02), (1.0, 0.01)])
            .build()
            .is_err()
    );
    // Missing day_count rejected
    assert!(
        HazardCurve::builder("NO-DC")
            .base_date(base)
            .recovery_rate(0.4)
            .knots([(1.0, 0.01)])
            .build()
            .is_err()
    );
    // Calendarless Bus252 rejected
    assert!(
        HazardCurve::builder("NO-CAL")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .recovery_rate(0.4)
            .knots([(1.0, 0.01)])
            .build()
            .is_err()
    );

    // High intensity without artificial cap succeeds and round-trips via JSON
    let high = HazardCurve::builder("HIGH")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(1.0)
        .knots([(1.0, 40.0), (5.0, 50.0)])
        .build()
        .expect("unbounded finite intensity and R=1.0 accepted");
    let json = serde_json::to_string(&high).expect("serialize");
    assert!(!json.contains("max_hazard_rate"));
    let restored: HazardCurve = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.recovery_rate(), 1.0);
    assert_eq!(restored.hazard_rate(1.0).expect("rate"), 40.0);
}

#[test]
fn left_continuous_intervals_and_origin_right_limit() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("BOUNDARY")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.01), (2.0, 0.02), (3.0, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    assert_eq!(curve.hazard_rate(0.0).expect("t=0"), 0.01);
    assert_eq!(curve.hazard_rate(0.5).expect("0.5"), 0.01);
    assert_eq!(curve.hazard_rate(1.0).expect("1.0"), 0.01);
    assert_eq!(curve.hazard_rate(1.0 + 1e-12).expect("1+eps"), 0.02);
    assert_eq!(curve.hazard_rate(2.0).expect("2.0"), 0.02);
    assert_eq!(curve.hazard_rate(3.0).expect("3.0"), 0.03);
    assert_eq!(curve.hazard_rate(10.0).expect("tail"), 0.03);
}

#[test]
fn conditioning_preserves_conditional_survival_and_handles_exact_and_tail_horizons() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("ROLL")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");

    let rolled_1y = curve
        .condition(base + Duration::days(365))
        .expect("exact-pillar conditioning succeeds");
    // Exact pillar conditioning at t_1 = 1.0 transitions immediately to lambda_2 = 0.02.
    assert_eq!(rolled_1y.hazard_rate(0.0).expect("t=0"), 0.02);
    for days in [180_i64, 365, 1460, 3285, 4000] {
        let target = base + Duration::days(365 + days);
        let expected = curve.sp_on_date(target).expect("base") / curve.sp(1.0).expect("s1");
        assert!(
            (rolled_1y.sp_on_date(target).expect("rolled") - expected).abs() < 1e-12,
            "days={days}"
        );
    }

    // Multi-knot conditioning past t_last retains constant tail
    let beyond = base + Duration::days(365 * 12);
    let rolled_tail = curve
        .condition(beyond)
        .expect("conditioning past final knot succeeds");
    let target = beyond + Duration::days(730);
    assert!((rolled_tail.sp_on_date(target).expect("tail sp") - (-0.06_f64).exp()).abs() < 1e-12);
    // Re-conditioning retains the original source timeline without offset drift
    let back_within = rolled_tail
        .condition(base + Duration::days(730))
        .expect("backward condition in original domain");
    assert_eq!(back_within.hazard_rate(0.0).expect("rate"), 0.02);
}

#[test]
fn bumps_apply_direct_intensity_atomically_and_require_exact_knots() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = HazardCurve::builder("DIRECT")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.40)
        .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020), (10.0, 0.025)])
        .build()
        .expect("valid hazard curve");

    // Direct parallel apply_bump shifts intensity directly without 1 / (1 - R)
    let parallel = curve
        .apply_bump(BumpSpec::parallel_bp(10.0))
        .expect("parallel direct shift");
    for ((_, base_l), (_, bumped_l)) in curve.knot_points().zip(parallel.knot_points()) {
        assert!((bumped_l - base_l - 0.0010).abs() < 1e-15);
    }

    // Exact tenor targeting succeeds for valid unique knots and rejects off-grid/duplicates
    let tenor_bumped = curve
        .with_tenor_hazard_rate_bumps_bp(&[(3.0, 2.0), (5.0, 8.0)])
        .expect("exact unique knots");
    let rates: Vec<f64> = tenor_bumped.knot_points().map(|(_, r)| r).collect();
    assert!((rates[0] - 0.010).abs() < 1e-15);
    assert!((rates[1] - 0.0152).abs() < 1e-15);
    assert!((rates[2] - 0.0208).abs() < 1e-15);
    assert!((rates[3] - 0.025).abs() < 1e-15);

    assert!(
        curve
            .with_tenor_hazard_rate_bumps_bp(&[(4.0, 5.0)])
            .is_err(),
        "off-grid tenor must error"
    );
    assert!(
        curve
            .with_tenor_hazard_rate_bumps_bp(&[(3.0, 2.0), (3.0, 3.0)])
            .is_err(),
        "duplicate tenor must error"
    );
    assert!(
        curve
            .with_tenor_hazard_rate_bumps_bp(&[(0.0, 1.0)])
            .is_err()
    );
    assert!(
        curve
            .with_tenor_hazard_rate_bumps_bp(&[(12.0, 1.0)])
            .is_err()
    );

    // Atomic failed bump
    let mut attempted = curve.clone();
    let err = attempted
        .bump_in_place(&BumpSpec::parallel_bp(-150.0))
        .expect_err("negative shifted hazard rate must fail");
    assert!(err.to_string().contains("negative hazard rate after bump"));
    assert_eq!(
        attempted.knot_points().collect::<Vec<_>>(),
        curve.knot_points().collect::<Vec<_>>()
    );
}

#[test]
fn triangular_key_rate_bumps_partition_the_parallel_hazard_shift() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let curve = HazardCurve::builder("KR")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.010), (2.0, 0.012), (5.0, 0.015), (10.0, 0.020)])
        .recovery_rate(0.40)
        .build()
        .expect("valid hazard curve");
    let buckets = [1.0, 3.0, 5.0, 10.0];
    let spec_for = |i: usize| match i {
        0 => BumpSpec::triangular_key_rate_first_bp(buckets[0], buckets[1], 10.0),
        3 => BumpSpec::triangular_key_rate_last_bp(buckets[2], buckets[3], 10.0),
        _ => BumpSpec::triangular_key_rate_bp(buckets[i - 1], buckets[i], buckets[i + 1], 10.0),
    };
    let parallel = curve
        .apply_bump(BumpSpec::parallel_bp(10.0))
        .expect("parallel bump");

    let mut summed = [0.0; 4];
    for i in 0..buckets.len() {
        let bumped = curve.apply_bump(spec_for(i)).expect("key-rate bump");
        for (k, ((_, base_lambda), (_, lambda))) in
            curve.knot_points().zip(bumped.knot_points()).enumerate()
        {
            summed[k] += lambda - base_lambda;
        }
    }
    for (k, ((_, base_lambda), (_, lambda))) in
        curve.knot_points().zip(parallel.knot_points()).enumerate()
    {
        assert!((summed[k] - (lambda - base_lambda)).abs() < 1e-15);
    }
    let first = curve.apply_bump(spec_for(0)).expect("first bucket");
    let lambda_2y = first.knot_points().nth(1).expect("2y knot").1;
    assert!((lambda_2y - (0.012 + 0.5 * 0.0010)).abs() < 1e-15);
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
    }

    #[test]
    fn test_seniority_fromstr_rejects_unknown() {
        for rejected in ["sub", "Senior", "senior-secured", " senior"] {
            assert!(rejected.parse::<Seniority>().is_err());
        }
    }
}

#[test]
fn sub_nanosecond_segment_and_subnormal_unconditional_pd_are_preserved() {
    let base = Date::from_calendar_date(1900, Month::January, 1).expect("date");
    let short_front = HazardCurve::builder("SHORT-FRONT")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(1e-10, 0.02), (1.0, 0.01)])
        .recovery_rate(0.4)
        .build()
        .expect("short front knot curve");
    let j = short_front.integrated_hazard(0.0, 5e-11).expect("J");
    assert!((j - 1e-12).abs() < 1e-26);

    let tiny = HazardCurve::builder("TINY")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(100.0, 7.0), (200.0, 1e-18)])
        .recovery_rate(0.4)
        .build()
        .expect("curve");
    let start = base + Duration::days(365 * 125);
    let end = start + Duration::days(365);
    let cond = tiny.conditional_default_prob(start, end).expect("tiny PD");
    assert!(cond > 0.0 && (cond / 1e-18 - 1.0).abs() < 1e-14);

    let uncond = tiny.default_prob(125.0, 126.0).expect("subnormal PD");
    assert!(uncond > 0.0);

    let overflow_curve =
        HazardCurve::flat("BIG", base, 1e308, 0.4, DayCount::Act365F).expect("flat");
    assert_eq!(overflow_curve.sp(10.0).expect("saturates"), 0.0);
    assert!(overflow_curve.integrated_hazard(0.0, 10.0).is_err());
}

#[test]
fn from_survival_probs_validates_strict_order_and_reproduces_pillars() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    assert!(
        HazardCurve::from_survival_probs(
            "BAD",
            base,
            &[(2.0, 0.96), (1.0, 0.98)],
            0.4,
            DayCount::Act365F,
        )
        .is_err()
    );
    assert!(
        HazardCurve::from_survival_probs("ZERO-SP", base, &[(1.0, 0.0)], 0.4, DayCount::Act365F,)
            .is_err()
    );
    let hc = HazardCurve::from_survival_probs(
        "OK",
        base,
        &[
            (0.0, 1.0),
            (1.0, (-0.02_f64).exp()),
            (3.0, (-0.12_f64).exp()),
        ],
        0.4,
        DayCount::Act365F,
    )
    .expect("valid survival curve");
    assert_eq!(hc.len(), 2);
    assert!((hc.hazard_rate(1.0).expect("r1") - 0.02).abs() < 1e-14);
    assert!((hc.hazard_rate(3.0).expect("r2") - 0.05).abs() < 1e-14);
}
