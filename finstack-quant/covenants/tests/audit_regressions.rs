//! Regression coverage for effective terms, execution progress, and forecast contracts.
use finstack_quant_core::dates::{Date, PeriodId, Tenor};
use finstack_quant_covenants::*;
use time::macros::date;

fn leverage() -> CovenantSpec {
    CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "leverage",
        ),
        "leverage",
    )
}
fn source(value: f64) -> HashMapMetricSource {
    HashMapMetricSource::from_pairs([("leverage", value)])
}
fn engine(spec: CovenantSpec) -> CovenantEngine {
    let mut engine = CovenantEngine::new();
    engine.add_spec(spec);
    engine
}

#[derive(Clone, Default)]
struct Target {
    rate: f64,
    sweep: f64,
    fail_sweep: bool,
}
impl InstrumentMutator for Target {
    fn set_default_status(&mut self, _: bool, _: Date) -> finstack_quant_core::Result<()> {
        Ok(())
    }
    fn increase_rate(&mut self, value: f64) -> finstack_quant_core::Result<()> {
        self.rate += value;
        Ok(())
    }
    fn set_cash_sweep(&mut self, value: f64) -> finstack_quant_core::Result<()> {
        self.sweep = value;
        if self.fail_sweep {
            return Err(finstack_quant_core::Error::Validation(
                "temporary failure".into(),
            ));
        }
        Ok(())
    }
    fn set_distribution_block(&mut self, _: bool) -> finstack_quant_core::Result<()> {
        Ok(())
    }
    fn set_maturity(&mut self, _: Date) -> finstack_quant_core::Result<()> {
        Ok(())
    }
    fn require_collateral(&mut self, _: &str, _: Date) -> finstack_quant_core::Result<()> {
        Err(finstack_quant_core::Error::Validation(
            "collateral execution unsupported".into(),
        ))
    }
}

#[test]
fn window_breach_retains_cure_and_consequences_after_window_expires() {
    let mut window_spec = leverage();
    window_spec
        .covenant
        .consequences
        .push(CovenantConsequence::RateIncrease { bp_increase: 200.0 });
    let mut engine = engine(leverage());
    engine.add_window(CovenantWindow {
        start: date!(2026 - 01 - 01),
        end: date!(2026 - 03 - 31),
        covenants: vec![window_spec],
    });
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    assert_eq!(
        engine.breach_history[0].cure_deadline,
        Some(date!(2026 - 04 - 30))
    );
    let snapshots = engine.breach_history.clone();
    let mut target = Target::default();
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 04 - 30))
        .unwrap()
        .is_empty());
    engine.specs.clear();
    engine.windows.clear();
    assert_eq!(
        engine
            .apply_consequences(&mut target, &snapshots, date!(2026 - 05 - 01))
            .unwrap()
            .len(),
        1
    );
    assert!((target.rate - 0.02).abs() < 1e-12);
}

#[test]
fn window_only_breach_has_cure_deadline() {
    let mut engine = CovenantEngine::new();
    engine.add_window(CovenantWindow {
        start: date!(2026 - 01 - 01),
        end: date!(2026 - 12 - 31),
        covenants: vec![leverage()],
    });
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    assert_eq!(
        engine.breach_history[0].cure_deadline,
        Some(date!(2026 - 04 - 30))
    );
}

#[test]
fn consequence_retry_commits_each_action_once_and_discards_failed_mutation() {
    let mut spec = leverage();
    spec.covenant.cure_period_days = None;
    spec.covenant.consequences = vec![
        CovenantConsequence::RateIncrease { bp_increase: 100.0 },
        CovenantConsequence::CashSweep {
            sweep_percentage: 0.5,
        },
    ];
    let mut engine = engine(spec);
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    let snapshots = engine.breach_history.clone();
    let mut target = Target {
        fail_sweep: true,
        ..Target::default()
    };
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 03 - 30))
        .unwrap()
        .is_empty());
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 03 - 31))
        .is_err());
    assert_eq!(target.sweep, 0.0);
    assert_eq!(engine.breach_history[0].applied_consequences.len(), 1);
    target.fail_sweep = false;
    assert_eq!(
        engine
            .apply_consequences(&mut target, &snapshots, date!(2026 - 04 - 01))
            .unwrap()
            .len(),
        1
    );
    assert!((target.rate - 0.01).abs() < 1e-12);
    assert_eq!(target.sweep, 0.5);
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 04 - 02))
        .unwrap()
        .is_empty());
}

#[test]
fn stale_breach_snapshot_cannot_override_cure_or_invent_history() {
    let mut spec = leverage();
    spec.covenant
        .consequences
        .push(CovenantConsequence::RateIncrease { bp_increase: 100.0 });
    let mut engine = engine(spec);
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    let snapshots = engine.breach_history.clone();
    engine
        .evaluate_and_track(
            &source(3.0),
            date!(2026 - 04 - 01),
            CovenantScope::Maintenance,
        )
        .unwrap();
    let mut target = Target::default();
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 05 - 01))
        .unwrap()
        .is_empty());
    engine.breach_history.clear();
    assert!(engine
        .apply_consequences(&mut target, &snapshots, date!(2026 - 05 - 01))
        .is_err());
}

#[test]
fn unsupported_collateral_remains_outstanding() {
    let mut spec = leverage();
    spec.covenant.cure_period_days = None;
    spec.covenant
        .consequences
        .push(CovenantConsequence::RequireCollateral {
            description: "pledge cash".into(),
        });
    let mut engine = engine(spec);
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    let snapshots = engine.breach_history.clone();
    assert!(engine
        .apply_consequences(&mut Target::default(), &snapshots, date!(2026 - 03 - 31))
        .is_err());
    assert!(engine.breach_history[0].applied_consequences.is_empty());
}

#[test]
fn non_testing_reports_do_not_cure_breaches() {
    let mut spec = leverage();
    spec.covenant.springing_condition = Some(SpringingCondition {
        metric_id: "utilization".into(),
        test: ThresholdTest::Minimum(0.35),
    });
    let mut engine = engine(spec);
    engine
        .evaluate_and_track(
            &HashMapMetricSource::from_pairs([("leverage", 5.0), ("utilization", 0.5)]),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance,
        )
        .unwrap();
    engine
        .evaluate_and_track(
            &HashMapMetricSource::from_pairs([("utilization", 0.1)]),
            date!(2026 - 04 - 01),
            CovenantScope::Maintenance,
        )
        .unwrap();
    assert!(!engine.breach_history[0].is_cured);
    engine.add_waiver(CovenantWaiver {
        covenant_id: "leverage".into(),
        effective_date: date!(2026 - 04 - 02),
        expiry_date: None,
        amended_threshold: None,
        description: "waive future tests".into(),
    });
    engine
        .evaluate_and_track(
            &HashMapMetricSource::new(),
            date!(2026 - 04 - 02),
            CovenantScope::Maintenance,
        )
        .unwrap();
    assert!(!engine.breach_history[0].is_cured);
}

#[test]
fn incurrence_requires_explicit_tracking_scope() {
    let mut spec = leverage();
    spec.covenant.scope = CovenantScope::Incurrence;
    let mut engine = engine(spec);
    assert!(engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Maintenance
        )
        .unwrap()
        .is_empty());
    assert!(engine.breach_history.is_empty());
    engine
        .evaluate_and_track(
            &source(5.0),
            date!(2026 - 03 - 31),
            CovenantScope::Incurrence,
        )
        .unwrap();
    assert_eq!(engine.breach_history.len(), 1);
}

#[test]
fn validation_rejects_invalid_triggers_consequences_and_overlapping_waivers() {
    let mut spec = leverage();
    spec.covenant.springing_condition = Some(SpringingCondition {
        metric_id: "utilization".into(),
        test: ThresholdTest::Minimum(f64::NAN),
    });
    assert!(engine(spec).validate().is_err());
    for consequence in [
        CovenantConsequence::RateIncrease {
            bp_increase: f64::INFINITY,
        },
        CovenantConsequence::RateIncrease { bp_increase: -1.0 },
        CovenantConsequence::CashSweep {
            sweep_percentage: 1.5,
        },
        CovenantConsequence::CashSweep {
            sweep_percentage: f64::NAN,
        },
    ] {
        let mut spec = leverage();
        spec.covenant.consequences.push(consequence);
        assert!(engine(spec).validate().is_err());
    }
    let mut engine = engine(leverage());
    for effective_date in [date!(2026 - 01 - 01), date!(2026 - 02 - 01)] {
        engine.add_waiver(CovenantWaiver {
            covenant_id: "leverage".into(),
            effective_date,
            expiry_date: None,
            amended_threshold: Some(6.0),
            description: String::new(),
        });
    }
    assert!(engine.validate().is_err());
}

#[test]
fn cure_date_overflow_leaves_history_unchanged() {
    let mut engine = engine(leverage());
    assert!(engine
        .evaluate_and_track(&source(5.0), Date::MAX, CovenantScope::Maintenance)
        .is_err());
    assert!(engine.breach_history.is_empty());
}

struct Series;
impl ModelTimeSeries for Series {
    fn get_scalar(&self, name: &str, _: &PeriodId) -> Option<f64> {
        match name {
            "leverage" => Some(5.0),
            "net_debt_to_ebitda" => Some(-1.0),
            "ebitda" => Some(50.0),
            _ => None,
        }
    }
    fn period_end_date(&self, period: &PeriodId) -> Date {
        Date::from_ordinal_date(period.year, period.index).unwrap()
    }
}
fn periods() -> Vec<PeriodId> {
    vec![
        PeriodId::day(2026, 90).unwrap(),
        PeriodId::day(2026, 181).unwrap(),
    ]
}

#[test]
fn net_cash_passes_but_non_positive_denominator_does_not() {
    let spec = CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxNetDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "net",
        ),
        "net_debt_to_ebitda",
    );
    let engine = engine(spec.clone());
    for earnings in [50.0, -50.0, 0.0] {
        let metrics =
            HashMapMetricSource::from_pairs([("net_debt_to_ebitda", -1.0), ("ebitda", earnings)]);
        let report = &engine.evaluate(&metrics, date!(2026 - 03 - 31)).unwrap()["net"];
        assert_eq!(report.passed, earnings > 0.0);
        assert_eq!(report.headroom.is_some(), earnings > 0.0);
    }
    assert!(engine
        .evaluate(
            &HashMapMetricSource::from_pairs([("net_debt_to_ebitda", -1.0)]),
            date!(2026 - 03 - 31)
        )
        .is_err());
    let forecast = forecast_covenant_generic(
        &spec,
        &Series,
        &periods(),
        CovenantForecastConfig::default(),
    )
    .unwrap();
    assert_eq!(forecast.breach_probability, vec![0.0, 0.0]);
    assert!(forecast.headroom.iter().all(Option::is_some));
}

#[test]
fn forecasts_respect_windows_waivers_and_inactivity() {
    let mut engine = CovenantEngine::new();
    engine.add_window(CovenantWindow {
        start: date!(2026 - 01 - 01),
        end: date!(2026 - 12 - 31),
        covenants: vec![leverage()],
    });
    assert_eq!(
        forecast_breaches_generic(
            &engine,
            &Series,
            &periods(),
            CovenantForecastConfig::default()
        )
        .unwrap()
        .len(),
        2
    );
    engine.add_waiver(CovenantWaiver {
        covenant_id: "leverage".into(),
        effective_date: date!(2026 - 01 - 01),
        expiry_date: Some(date!(2026 - 03 - 31)),
        amended_threshold: Some(6.0),
        description: String::new(),
    });
    let breaches = forecast_breaches_generic(
        &engine,
        &Series,
        &periods(),
        CovenantForecastConfig::default(),
    )
    .unwrap();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].breach_date, date!(2026 - 06 - 30));
    let mut inactive = leverage();
    inactive.covenant.is_active = false;
    inactive.metric_id = Some("missing".into());
    let forecast = forecast_covenant_generic(
        &inactive,
        &Series,
        &periods(),
        CovenantForecastConfig::default(),
    )
    .unwrap();
    assert_eq!(forecast.projected_values, vec![None, None]);
    assert_eq!(forecast.breach_probability, vec![0.0, 0.0]);
}

#[test]
fn stochastic_cutoff_never_hides_base_case_breaches() {
    let config = CovenantForecastConfig {
        stochastic: true,
        volatility: Some(0.5),
        reference_date: Some(date!(2025 - 03 - 31)),
        breach_probability_threshold: 0.99,
        ..Default::default()
    };
    let breaches =
        forecast_breaches_generic(&engine(leverage()), &Series, &periods(), config).unwrap();
    assert_eq!(breaches.len(), 2);
    assert!(breaches.iter().all(|b| b.breach_probability < 0.99));
}

#[test]
fn forecast_rejects_reversed_dates_future_reference_and_one_independent_sample() {
    let mut dates = periods();
    dates.reverse();
    assert!(forecast_covenant_generic(
        &leverage(),
        &Series,
        &dates,
        CovenantForecastConfig::default()
    )
    .is_err());
    for (count, antithetic) in [(1, false), (2, true)] {
        let config = CovenantForecastConfig {
            stochastic: true,
            volatility: Some(0.2),
            num_paths: count,
            antithetic,
            ..Default::default()
        };
        assert!(forecast_covenant_generic(&leverage(), &Series, &periods(), config).is_err());
    }
    let config = CovenantForecastConfig {
        reference_date: Some(date!(2027 - 01 - 01)),
        ..Default::default()
    };
    assert!(forecast_covenant_generic(&leverage(), &Series, &periods(), config).is_err());
}
