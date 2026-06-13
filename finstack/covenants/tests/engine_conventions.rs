//! Regression tests for covenant engine identity and evaluation conventions.
//!
//! Covers:
//! - B3: `project_finance` MinDSCR identity collision — a distribution-lockup
//!   breach must trigger `BlockDistributions` only, never the primary
//!   covenant's Event of Default.
//! - Duplicate instance keys are rejected at evaluation time.
//! - Negative leverage-type ratios (negative EBITDA) breach max covenants.
//! - Relative headroom keeps its sign for negative thresholds.

use finstack_core::dates::{Date, Tenor};
use finstack_covenants::{
    templates, Covenant, CovenantConsequence, CovenantEngine, CovenantMetricId, CovenantSpec,
    CovenantType, CovenantWindow, HashMapMetricSource, InstrumentMutator, ThresholdSchedule,
    ThresholdTest,
};
use time::Month;

fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

/// Minimal instrument recording which consequences were applied.
#[derive(Default)]
struct MockInstrument {
    in_default: bool,
    distributions_blocked: bool,
    rate_increases: Vec<f64>,
    sweep: Option<f64>,
}

impl InstrumentMutator for MockInstrument {
    fn set_default_status(&mut self, is_default: bool, _as_of: Date) -> finstack_core::Result<()> {
        self.in_default = is_default;
        Ok(())
    }
    fn increase_rate(&mut self, increase: f64) -> finstack_core::Result<()> {
        self.rate_increases.push(increase);
        Ok(())
    }
    fn set_cash_sweep(&mut self, percentage: f64) -> finstack_core::Result<()> {
        self.sweep = Some(percentage);
        Ok(())
    }
    fn set_distribution_block(&mut self, blocked: bool) -> finstack_core::Result<()> {
        self.distributions_blocked = blocked;
        Ok(())
    }
    fn set_maturity(&mut self, _new_maturity: Date) -> finstack_core::Result<()> {
        Ok(())
    }
}

/// B3 regression: DSCR between the lockup threshold (1.25) and the primary
/// default threshold (1.05) must breach ONLY the lockup covenant
/// (`BlockDistributions`), never the primary covenant's `Default`
/// consequence, and both covenants must appear in the report map.
#[test]
fn project_finance_lockup_breach_blocks_distributions_without_default() {
    let mut engine = CovenantEngine::new();
    for spec in templates::project_finance(1.05, 1.25, 5_000_000.0, 6.0) {
        engine.add_spec(spec);
    }

    // DSCR 1.15: above the 1.05 default trigger, below the 1.25 lockup.
    let mut metrics = HashMapMetricSource::from_pairs([
        ("dscr", 1.15),
        ("liquidity", 10_000_000.0),
        ("net_debt_to_ebitda", 3.0),
    ]);

    let test_date = d(2025, 3, 31);
    let reports = engine
        .evaluate_and_track(&mut metrics, test_date)
        .expect("evaluation should succeed");

    // Both MinDSCR covenants are present under distinct keys.
    assert!(
        reports.contains_key("min_dscr_default"),
        "primary DSCR covenant missing from reports: {:?}",
        reports.keys().collect::<Vec<_>>()
    );
    assert!(
        reports.contains_key("min_dscr_lockup"),
        "lockup DSCR covenant missing from reports"
    );
    assert_eq!(reports.len(), 4, "all four covenants must be reported");

    assert!(reports["min_dscr_default"].passed, "1.15 >= 1.05 must pass");
    assert!(!reports["min_dscr_lockup"].passed, "1.15 < 1.25 must fail");

    // Apply consequences for the recorded breaches once the cure period has
    // lapsed; the result must be a distribution block only — no Event of
    // Default.
    let breaches = engine.breach_history.clone();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].covenant_id, "min_dscr_lockup");

    let past_cure = d(2025, 5, 15); // > 30-day default cure window
    let mut instrument = MockInstrument::default();
    let applications = engine
        .apply_consequences(&mut instrument, &breaches, past_cure)
        .expect("consequence application should succeed");

    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].consequence_type, "Block Distributions");
    assert!(instrument.distributions_blocked);
    assert!(
        !instrument.in_default,
        "a lockup-only breach must never trigger an Event of Default"
    );
}

#[test]
fn duplicate_instance_keys_are_rejected_at_evaluation() {
    let mut engine = CovenantEngine::new();
    // Two same-type covenants without labels collide on "min_dscr".
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinDSCR { threshold: 1.05 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("dscr"),
    ));
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinDSCR { threshold: 1.25 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("dscr"),
    ));

    let mut metrics = HashMapMetricSource::from_pairs([("dscr", 1.15)]);
    let err = engine
        .evaluate(&mut metrics, d(2025, 3, 31))
        .expect_err("duplicate instance keys must be a validation error");
    assert!(
        err.to_string().contains("duplicate covenant instance key"),
        "unexpected error: {err}"
    );
}

#[test]
fn negative_ebitda_leverage_breaches_max_ratio_covenant() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    // Negative EBITDA → negative ratio. Naively -10 <= 4 would "pass".
    let mut metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", -10.0)]);
    let reports = engine
        .evaluate(&mut metrics, d(2025, 3, 31))
        .expect("evaluation should succeed");

    let report = &reports["max_debt_ebitda"];
    assert!(
        !report.passed,
        "negative leverage ratio (NM) must breach a max covenant"
    );
    assert!(report
        .details
        .as_deref()
        .is_some_and(|s| s.contains("not meaningful")));
}

#[test]
fn negative_metric_still_passes_custom_maximum() {
    // Custom maximum covenants keep plain IEEE semantics: negative values
    // may be legitimate (e.g. net-short exposure caps).
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::Custom {
                metric: "net_exposure".to_string(),
                test: ThresholdTest::Maximum(0.5),
            },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("net_exposure"),
    ));

    let mut metrics = HashMapMetricSource::from_pairs([("net_exposure", -0.2)]);
    let reports = engine
        .evaluate(&mut metrics, d(2025, 3, 31))
        .expect("evaluation should succeed");
    assert!(reports["custom"].passed);
}

#[test]
fn relative_headroom_keeps_sign_for_negative_threshold() {
    // Custom Maximum(-1.0): a value of -2.0 is comfortably below the cap →
    // positive headroom. Pre-fix, dividing by a signed negative threshold
    // flipped the sign.
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::Custom {
                metric: "net_position".to_string(),
                test: ThresholdTest::Maximum(-1.0),
            },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("net_position"),
    ));

    let mut metrics = HashMapMetricSource::from_pairs([("net_position", -2.0)]);
    let reports = engine
        .evaluate(&mut metrics, d(2025, 3, 31))
        .expect("evaluation should succeed");
    let report = &reports["custom"];
    assert!(report.passed);
    let headroom = report.headroom.expect("headroom should be present");
    assert!(
        headroom > 0.0,
        "passing covenant must report positive headroom, got {headroom}"
    );
    // (threshold - value) / |threshold| = (-1 - (-2)) / 1 = 1.0
    assert!((headroom - 1.0).abs() < 1e-12);

    // And a failing value above the cap reports negative headroom.
    let mut metrics = HashMapMetricSource::from_pairs([("net_position", -0.5)]);
    let reports = engine
        .evaluate(&mut metrics, d(2025, 3, 31))
        .expect("evaluation should succeed");
    let report = &reports["custom"];
    assert!(!report.passed);
    assert!(report.headroom.expect("headroom should be present") < 0.0);
}

#[test]
fn persistent_breach_is_one_episode_with_one_consequence_application() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        )
        .with_cure_period(Some(30))
        .with_consequence(CovenantConsequence::RateIncrease { bp_increase: 200.0 }),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    let first_date = d(2025, 3, 31);
    let second_date = d(2025, 6, 30);
    engine
        .evaluate_and_track(
            &mut HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]),
            first_date,
        )
        .expect("first breach should track");
    engine
        .evaluate_and_track(
            &mut HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.5)]),
            second_date,
        )
        .expect("persistent breach should evaluate");

    assert_eq!(
        engine.breach_history.len(),
        1,
        "continuous breach must remain one active episode"
    );
    assert_eq!(engine.breach_history[0].breach_date, first_date);
    assert_eq!(engine.breach_history[0].cure_deadline, Some(d(2025, 4, 30)));

    let breaches = engine.breach_history.clone();
    let mut instrument = MockInstrument::default();
    let applications = engine
        .apply_consequences(&mut instrument, &breaches, d(2025, 7, 15))
        .expect("consequence application should succeed");
    assert_eq!(applications.len(), 1);
    assert_eq!(instrument.rate_increases, vec![0.02]);

    let applications = engine
        .apply_consequences(&mut instrument, &breaches, d(2025, 7, 16))
        .expect("repeat application should be deduped");
    assert!(applications.is_empty());
    assert_eq!(instrument.rate_increases, vec![0.02]);
}

#[test]
fn recovery_before_cure_deadline_marks_breach_cured() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        )
        .with_cure_period(Some(30))
        .with_consequence(CovenantConsequence::Default),
        CovenantMetricId::from("debt_to_ebitda"),
    ));

    engine
        .evaluate_and_track(
            &mut HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]),
            d(2025, 3, 31),
        )
        .expect("breach should track");
    engine
        .evaluate_and_track(
            &mut HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.5)]),
            d(2025, 4, 15),
        )
        .expect("recovery should evaluate");

    assert_eq!(engine.breach_history.len(), 1);
    assert!(
        engine.breach_history[0].is_cured,
        "passing before cure deadline must mark active breach cured"
    );

    let breaches = engine.breach_history.clone();
    let mut instrument = MockInstrument::default();
    let applications = engine
        .apply_consequences(&mut instrument, &breaches, d(2025, 5, 15))
        .expect("cured breach should be ignored");
    assert!(applications.is_empty());
    assert!(!instrument.in_default);
}

#[test]
fn validation_rejects_negative_cure_duplicate_schedule_and_overlapping_windows() {
    let mut negative_cure = CovenantEngine::new();
    negative_cure.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        )
        .with_cure_period(Some(-1)),
        CovenantMetricId::from("debt_to_ebitda"),
    ));
    assert!(negative_cure.validate().is_err());

    let duplicate_schedule =
        ThresholdSchedule::try_new(vec![(d(2025, 1, 1), 4.5), (d(2025, 1, 1), 4.0)]);
    assert!(duplicate_schedule.is_err());

    let mut overlapping = CovenantEngine::new();
    let spec = CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    );
    overlapping.add_window(CovenantWindow {
        start: d(2025, 1, 1),
        end: d(2025, 6, 30),
        covenants: vec![spec.clone()],
    });
    overlapping.add_window(CovenantWindow {
        start: d(2025, 6, 1),
        end: d(2025, 12, 31),
        covenants: vec![spec],
    });
    assert!(overlapping.validate().is_err());
}

#[test]
fn window_fallback_to_base_specs_is_explicit() {
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEBITDA { threshold: 4.0 },
            Tenor::quarterly(),
        ),
        CovenantMetricId::from("debt_to_ebitda"),
    ));
    engine.add_window(CovenantWindow {
        start: d(2025, 1, 1),
        end: d(2025, 3, 31),
        covenants: vec![CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MinInterestCoverage { threshold: 2.0 },
                Tenor::quarterly(),
            ),
            CovenantMetricId::from("interest_coverage"),
        )],
    });

    let reports = engine
        .evaluate(
            &mut HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.0)]),
            d(2025, 6, 30),
        )
        .expect("outside all windows should fall back to base specs");
    assert!(reports.contains_key("max_debt_ebitda"));
}
