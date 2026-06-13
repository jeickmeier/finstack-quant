//! Integration tests for the ECL / IFRS 9 / CECL public API.
//!
//! Exercises the workflows a user replicates end-to-end via public
//! constructors only: Stage 3 (credit-impaired) measurement, CECL engine
//! input validation, and EAD amortization schedules.

use finstack_statements_analytics::analysis::{
    compute_ecl_single, CeclConfig, CeclEngine, CeclMethodology, EclConfig, EclConfigBuilder,
    Exposure, MacroScenario, PdTermStructure, QualitativeFlags, RawPdCurve, Stage,
};

fn exposure(id: &str) -> Exposure {
    Exposure {
        id: id.to_string(),
        segments: vec!["corporate".to_string()],
        ead: 1_000_000.0,
        eir: 0.05,
        remaining_maturity_years: 5.0,
        lgd: 0.45,
        days_past_due: 0,
        current_rating: Some("BBB".to_string()),
        origination_rating: Some("BBB".to_string()),
        qualitative_flags: QualitativeFlags::default(),
        consecutive_performing_periods: 0,
        previous_stage: None,
        ead_schedule: None,
    }
}

fn bbb_curve() -> RawPdCurve {
    RawPdCurve::new(
        "BBB",
        vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.04), (5.0, 0.10)],
    )
    .unwrap()
}

struct ForecastHorizonGuardCurve {
    rating: &'static str,
    max_t: f64,
}

impl PdTermStructure for ForecastHorizonGuardCurve {
    fn cumulative_pd(&self, rating: &str, t: f64) -> finstack_core::Result<f64> {
        if rating != self.rating {
            return Err(finstack_core::Error::Validation(format!(
                "curve is for {}, got {rating}",
                self.rating
            )));
        }
        if t > self.max_t + 1e-12 {
            return Err(finstack_core::Error::Validation(format!(
                "forecast curve queried beyond supportable horizon: {t}"
            )));
        }
        Ok(0.02 * t)
    }
}

// ---------------------------------------------------------------------------
// Stage 3 measurement (IFRS 9 5.5.33 / B5.5.33)
// ---------------------------------------------------------------------------

#[test]
fn stage3_ecl_is_discounted_lgd_times_ead() {
    let exp = exposure("defaulted-1");
    let config = EclConfig::default();

    let result = compute_ecl_single(&exp, Stage::Stage3, &bbb_curve(), &config).unwrap();

    // PD ≡ 1 for credit-impaired assets: ECL = LGD x EAD x DF(t_recovery),
    // with default time-to-recovery of 1.0 year at EIR 5%.
    let expected = 0.45 * 1_000_000.0 / 1.05;
    assert!(
        (result.ecl - expected).abs() < 1e-6,
        "Stage 3 ECL {} != discounted LGD x EAD {}",
        result.ecl,
        expected
    );
    assert_eq!(result.buckets.len(), 1);
    assert!((result.buckets[0].marginal_pd - 1.0).abs() < 1e-12);
}

#[test]
fn stage3_zero_remaining_maturity_has_positive_allowance() {
    let mut exp = exposure("defaulted-matured");
    exp.remaining_maturity_years = 0.0;

    let result =
        compute_ecl_single(&exp, Stage::Stage3, &bbb_curve(), &EclConfig::default()).unwrap();

    assert!(
        result.ecl > 0.0,
        "A defaulted exposure at maturity must still carry an allowance, got {}",
        result.ecl
    );
}

#[test]
fn stage3_ecl_exceeds_stage2_for_same_exposure() {
    let exp = exposure("compare");
    let config = EclConfig::default();
    let curve = bbb_curve();

    let s2 = compute_ecl_single(&exp, Stage::Stage2, &curve, &config).unwrap();
    let s3 = compute_ecl_single(&exp, Stage::Stage3, &curve, &config).unwrap();

    assert!(
        s3.ecl > s2.ecl,
        "Credit-impaired ECL ({}) must exceed performing lifetime ECL ({})",
        s3.ecl,
        s2.ecl
    );
}

#[test]
fn stage3_still_validates_pd_curve_rating() {
    let exp = exposure("defaulted-rating-mismatch");
    let wrong_curve = RawPdCurve::new("AAA", vec![(0.0, 0.0), (1.0, 0.005), (5.0, 0.02)]).unwrap();

    let err = compute_ecl_single(&exp, Stage::Stage3, &wrong_curve, &EclConfig::default())
        .expect_err("Stage 3 shortcut must still validate curve/rating mapping");
    assert!(err.to_string().contains("RawPdCurve is for rating"));
}

#[test]
fn negative_eir_is_rejected() {
    let mut exp = exposure("negative-eir");
    exp.eir = -0.01;

    let err = compute_ecl_single(&exp, Stage::Stage1, &bbb_curve(), &EclConfig::default())
        .expect_err("negative EIR must be rejected");
    assert!(err.to_string().contains("EIR"));
}

// ---------------------------------------------------------------------------
// CECL engine validation
// ---------------------------------------------------------------------------

#[test]
fn cecl_engine_rejects_empty_pd_sources() {
    let result = CeclEngine::new(CeclConfig::default(), Vec::new());
    assert!(
        result.is_err(),
        "CeclEngine::new must reject empty pd_sources instead of returning ECL = 0"
    );
}

#[test]
fn cecl_engine_rejects_invalid_pd_source_weights() {
    let curve = bbb_curve();
    let s1 = MacroScenario {
        id: "base".to_string(),
        weight: 0.5,
        lgd_override: None,
    };
    let s2 = MacroScenario {
        id: "downside".to_string(),
        weight: 0.3, // sums to 0.8, not 1.0
        lgd_override: None,
    };
    let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![
        (&s1, &curve as &dyn PdTermStructure),
        (&s2, &curve as &dyn PdTermStructure),
    ];

    let result = CeclEngine::new(CeclConfig::default(), pd_sources);
    assert!(
        result.is_err(),
        "CeclEngine::new must validate pd_sources scenario weights"
    );
}

#[test]
fn cecl_engine_rejects_unimplemented_methodologies() {
    let curve = bbb_curve();
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    for methodology in [CeclMethodology::Warm, CeclMethodology::Vintage] {
        let config = CeclConfig {
            methodology,
            ..CeclConfig::default()
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];
        assert!(
            CeclEngine::new(config, pd_sources).is_err(),
            "{methodology:?} must be rejected rather than silently no-op"
        );
    }
}

#[test]
fn cecl_impaired_dpd_uses_lgd_ead_shortcut() {
    let curve = bbb_curve();
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &curve as &dyn PdTermStructure)];
    let config = CeclConfig {
        impaired_time_to_recovery_years: 1.0,
        ..CeclConfig::default()
    };
    let engine = CeclEngine::new(config, pd_sources).unwrap();

    let mut exp = exposure("cecl-impaired");
    exp.days_past_due = 90;
    let result = engine.compute_cecl(&exp).unwrap();

    let expected = exp.lgd * exp.ead / 1.05;
    assert!(
        (result.ecl - expected).abs() < 1e-6,
        "CECL impaired ECL {} != discounted LGD x EAD {}",
        result.ecl,
        expected
    );
    assert_eq!(result.horizon, 1.0);
}

#[test]
fn cecl_discount_toggle_controls_expected_loss_discounting() {
    let curve = bbb_curve();
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    let discounted_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &curve as &dyn PdTermStructure)];
    let undiscounted_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &curve as &dyn PdTermStructure)];

    let discounted = CeclEngine::new(CeclConfig::default(), discounted_sources)
        .unwrap()
        .compute_cecl(&exposure("discounted"))
        .unwrap();
    let undiscounted = CeclEngine::new(
        CeclConfig {
            discount_expected_losses: false,
            ..CeclConfig::default()
        },
        undiscounted_sources,
    )
    .unwrap()
    .compute_cecl(&exposure("undiscounted"))
    .unwrap();

    assert!(
        undiscounted.ecl > discounted.ecl,
        "turning discounting off should increase CECL ECL"
    );
}

#[test]
fn cecl_linear_reversion_does_not_query_forecast_after_rs_horizon() {
    let curve = ForecastHorizonGuardCurve {
        rating: "BBB",
        max_t: 1.0,
    };
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &curve as &dyn PdTermStructure)];
    let engine = CeclEngine::new(
        CeclConfig {
            forecast_horizon_years: 1.0,
            reversion_method: finstack_statements_analytics::analysis::ReversionMethod::Linear {
                reversion_years: 1.0,
            },
            historical_annual_pd: 0.03,
            ..CeclConfig::default()
        },
        pd_sources,
    )
    .unwrap();

    let mut exp = exposure("linear-reversion");
    exp.remaining_maturity_years = 2.0;
    let result = engine.compute_cecl(&exp).unwrap();
    assert!(result.ecl > 0.0);
}

#[test]
fn cecl_impaired_still_validates_pd_curve_rating() {
    let wrong_curve = RawPdCurve::new("AAA", vec![(0.0, 0.0), (1.0, 0.005), (5.0, 0.02)]).unwrap();
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &wrong_curve as &dyn PdTermStructure)];
    let engine = CeclEngine::new(CeclConfig::default(), pd_sources).unwrap();

    let mut exp = exposure("cecl-impaired-rating-mismatch");
    exp.days_past_due = 90;
    let err = engine
        .compute_cecl(&exp)
        .expect_err("CECL impaired shortcut must validate curve/rating mapping");
    assert!(err.to_string().contains("RawPdCurve is for rating"));
}

// ---------------------------------------------------------------------------
// EAD schedule
// ---------------------------------------------------------------------------

#[test]
fn amortizing_ead_schedule_reduces_lifetime_ecl_in_ifrs9_and_cecl() {
    let curve = bbb_curve();
    let config = EclConfigBuilder::new().build().unwrap();

    let constant = exposure("constant");
    let mut amortizing = exposure("amortizing");
    amortizing.ead_schedule = Some(vec![(0.0, 1_000_000.0), (5.0, 0.0)]);

    // IFRS 9 lifetime (Stage 2)
    let ifrs9_constant = compute_ecl_single(&constant, Stage::Stage2, &curve, &config).unwrap();
    let ifrs9_amortizing = compute_ecl_single(&amortizing, Stage::Stage2, &curve, &config).unwrap();
    assert!(
        ifrs9_amortizing.ecl < ifrs9_constant.ecl,
        "IFRS 9: amortizing ECL ({}) must be below constant-EAD ECL ({})",
        ifrs9_amortizing.ecl,
        ifrs9_constant.ecl
    );

    // CECL (always lifetime)
    let scenario = MacroScenario {
        id: "base".to_string(),
        weight: 1.0,
        lgd_override: None,
    };
    let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
        vec![(&scenario, &curve as &dyn PdTermStructure)];
    let engine = CeclEngine::new(CeclConfig::default(), pd_sources).unwrap();
    let cecl_constant = engine.compute_cecl(&constant).unwrap();
    let cecl_amortizing = engine.compute_cecl(&amortizing).unwrap();
    assert!(
        cecl_amortizing.ecl < cecl_constant.ecl,
        "CECL: amortizing ECL ({}) must be below constant-EAD ECL ({})",
        cecl_amortizing.ecl,
        cecl_constant.ecl
    );
}

#[test]
fn invalid_ead_schedule_is_rejected() {
    let curve = bbb_curve();
    let config = EclConfig::default();

    let mut exp = exposure("bad-schedule");
    exp.ead_schedule = Some(vec![(2.0, 100.0), (1.0, 50.0)]); // non-increasing times
    assert!(compute_ecl_single(&exp, Stage::Stage1, &curve, &config).is_err());

    exp.ead_schedule = Some(vec![(0.0, f64::NAN)]); // non-finite EAD
    assert!(compute_ecl_single(&exp, Stage::Stage1, &curve, &config).is_err());
}
