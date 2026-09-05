//! Point-in-time covenant evaluation, breach tracking, and consequence application.
//!
//! # Scope and conventions
//!
//! - **Test dates are caller-controlled.** [`Covenant::test_frequency`] is
//!   descriptive metadata only: the engine evaluates whenever the caller
//!   invokes [`CovenantEngine::evaluate`] with a `test_date` and does not
//!   itself generate or enforce a testing schedule.
//! - **Equity cures are not modeled.** A breach can only be neutralized via
//!   a [`CovenantWaiver`] (full waiver or amended threshold) or by the metric
//!   recovering before the cure deadline; there is no mechanism for injecting
//!   sponsor equity into the tested metric.
//! - **Metric values are taken as-is (LTM contract).** The engine performs no
//!   trailing-twelve-month or other window aggregation. If a covenant is
//!   defined on an LTM basis (as most leverage/coverage covenants are), the
//!   supplied metric node must already encode it — e.g. a statements node
//!   defined via `ttm(ebitda)` — before being exposed through
//!   [`crate::metric::CovenantMetricSource`].

mod covenant_engine;
mod helpers;
mod types;

pub use covenant_engine::CovenantEngine;
pub use helpers::InstrumentMutator;
pub(crate) use helpers::{
    headroom_for, is_covenant_breached, spec_metric_names, springing_condition_met,
};
pub use types::{
    BoundKind, ConsequenceApplication, Covenant, CovenantBreach, CovenantConsequence,
    CovenantScope, CovenantSpec, CovenantType, CovenantWaiver, CovenantWindow, SpringingCondition,
    ThresholdTest,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metric::HashMapMetricSource;
    use finstack_quant_core::dates::{Date, Tenor};

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    /// A NaN springing metric activates the covenant rather than treating an
    /// undefined trigger as unmet and reporting a pass.
    #[test]
    fn nan_springing_metric_activates_rather_than_silently_passing() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "springing_leverage",
        )
        .with_springing_condition(SpringingCondition {
            metric_id: "revolver_utilization".into(),
            test: ThresholdTest::Minimum(0.35),
        });
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);

        let metrics = HashMapMetricSource::from_pairs([
            ("revolver_utilization", f64::NAN),
            ("debt_to_ebitda", 3.2),
        ]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["springing_leverage"];
        assert_eq!(
            report.actual_value,
            Some(3.2),
            "the covenant must be ACTIVATED and evaluated, not skipped; details {:?}",
            report.details
        );
        assert_ne!(
            report.details.as_deref(),
            Some("Springing condition not met"),
            "a NaN springing metric must not be reported as an unmet condition"
        );

        let breaching = HashMapMetricSource::from_pairs([
            ("revolver_utilization", f64::NAN),
            ("debt_to_ebitda", 5.0),
        ]);
        let reports = engine.evaluate(&breaching, date(2024, 3, 31)).unwrap();
        let report = &reports["springing_leverage"];
        assert!(
            !report.passed,
            "NaN springing + breaching metric must report a breach, not a pass"
        );
    }

    #[test]
    fn springing_condition_still_gates_on_finite_metrics() {
        let build = |utilization: f64| {
            let covenant = Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.5 },
                Tenor::quarterly(),
                "springing_leverage",
            )
            .with_springing_condition(SpringingCondition {
                metric_id: "revolver_utilization".into(),
                test: ThresholdTest::Minimum(0.35),
            });
            let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
            let mut engine = CovenantEngine::new();
            engine.add_spec(spec);
            let metrics = HashMapMetricSource::from_pairs([
                ("revolver_utilization", utilization),
                ("debt_to_ebitda", 5.0),
            ]);
            let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
            reports["springing_leverage"].clone()
        };

        let inactive = build(0.10);
        assert!(inactive.passed);
        assert_eq!(inactive.actual_value, None);

        let active = build(0.50);
        assert!(!active.passed);
        assert_eq!(active.actual_value, Some(5.0));
    }

    #[test]
    fn max_leverage_passes_when_below_threshold() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.2)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(report.passed);
        assert_eq!(report.actual_value, Some(3.2));
        assert_eq!(report.threshold, Some(4.5));
    }

    #[test]
    fn max_leverage_breaches_when_above_threshold() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(!report.passed);
        assert_eq!(report.actual_value, Some(5.0));
    }

    #[test]
    fn negative_ratio_treated_as_breach() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", -1.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(!report.passed);
    }

    #[test]
    fn min_coverage_passes_when_above_threshold() {
        let covenant = Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 2.0 },
            Tenor::quarterly(),
            "min_interest_coverage",
        );
        let spec = CovenantSpec::with_metric(covenant, "interest_coverage");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("interest_coverage", 3.5)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["min_interest_coverage"];
        assert!(report.passed);
    }

    #[test]
    fn min_coverage_breaches_when_below_threshold() {
        let covenant = Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 2.0 },
            Tenor::quarterly(),
            "min_interest_coverage",
        );
        let spec = CovenantSpec::with_metric(covenant, "interest_coverage");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("interest_coverage", 1.5)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["min_interest_coverage"];
        assert!(!report.passed);
    }

    #[test]
    fn inactive_covenant_auto_passes() {
        let mut covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        covenant.is_active = false;
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 99.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(report.passed);
    }

    #[test]
    fn full_waiver_skips_evaluation() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        engine.add_waiver(CovenantWaiver {
            covenant_id: "max_debt_ebitda".to_string(),
            effective_date: date(2024, 1, 1),
            expiry_date: Some(date(2024, 12, 31)),
            amended_threshold: None,
            description: "Full waiver".to_string(),
        });
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 10.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 6, 30)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(report.passed);
    }

    #[test]
    fn amended_threshold_waiver_overrides_static() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        engine.add_waiver(CovenantWaiver {
            covenant_id: "max_debt_ebitda".to_string(),
            effective_date: date(2024, 1, 1),
            expiry_date: Some(date(2024, 12, 31)),
            amended_threshold: Some(6.0),
            description: "Amended to 6.0x".to_string(),
        });
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.5)]);
        let reports = engine.evaluate(&metrics, date(2024, 6, 30)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(report.passed);
        assert_eq!(report.threshold, Some(6.0));
    }

    #[test]
    fn headroom_positive_when_passing_max_covenant() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(report.passed);
        let headroom = report.headroom.unwrap();
        assert!(headroom > 0.0);
    }

    #[test]
    fn headroom_negative_when_breaching_max_covenant() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        let report = &reports["max_debt_ebitda"];
        assert!(!report.passed);
        let headroom = report.headroom.unwrap();
        assert!(headroom < 0.0);
    }

    #[test]
    fn duplicate_instance_keys_rejected() {
        let cov1 = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let cov2 = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 5.0 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec1 = CovenantSpec::with_metric(cov1, "debt_to_ebitda");
        let spec2 = CovenantSpec::with_metric(cov2, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec1);
        engine.add_spec(spec2);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.0)]);
        let result = engine.evaluate(&metrics, date(2024, 3, 31));
        assert!(result.is_err());
    }

    #[test]
    fn labeled_covenants_coexist() {
        let cov1 = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "senior",
        );
        let cov2 = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 6.0 },
            Tenor::quarterly(),
            "total",
        );
        let spec1 = CovenantSpec::with_metric(cov1, "debt_to_ebitda");
        let spec2 = CovenantSpec::with_metric(cov2, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec1);
        engine.add_spec(spec2);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]);
        let reports = engine.evaluate(&metrics, date(2024, 3, 31)).unwrap();
        assert!(reports.contains_key("senior"));
        assert!(reports.contains_key("total"));
        assert!(!reports["senior"].passed);
        assert!(reports["total"].passed);
    }

    #[test]
    fn is_covenant_breached_nan_is_breach() {
        let ct = CovenantType::MaxDebtToEbitda { threshold: 4.5 };
        assert!(is_covenant_breached(&ct, f64::NAN, 4.5));
    }

    #[test]
    fn is_covenant_breached_at_most() {
        let ct = CovenantType::MaxDebtToEbitda { threshold: 4.5 };
        assert!(!is_covenant_breached(&ct, 4.5, 4.5));
        assert!(is_covenant_breached(&ct, 4.51, 4.5));
        assert!(!is_covenant_breached(&ct, 4.49, 4.5));
    }

    #[test]
    fn is_covenant_breached_at_least() {
        let ct = CovenantType::MinInterestCoverage { threshold: 2.0 };
        assert!(!is_covenant_breached(&ct, 2.0, 2.0));
        assert!(is_covenant_breached(&ct, 1.99, 2.0));
        assert!(!is_covenant_breached(&ct, 2.01, 2.0));
    }

    #[test]
    fn headroom_for_at_most_positive_when_below() {
        let hr = headroom_for(Some(BoundKind::AtMost), 3.0, 4.5);
        assert!(hr > 0.0);
    }

    #[test]
    fn headroom_for_at_least_positive_when_above() {
        let hr = headroom_for(Some(BoundKind::AtLeast), 3.0, 2.0);
        assert!(hr > 0.0);
    }

    #[test]
    fn headroom_for_none_returns_zero() {
        let hr = headroom_for(None, 3.0, 4.5);
        assert_eq!(hr, 0.0);
    }

    #[test]
    fn headroom_for_nan_inputs_returns_nan() {
        let hr = headroom_for(Some(BoundKind::AtMost), f64::NAN, 4.5);
        assert!(hr.is_nan());
    }

    #[test]
    fn evaluate_and_track_records_breach() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]);
        let reports = engine
            .evaluate_and_track(&metrics, date(2024, 3, 31))
            .unwrap();
        assert!(!reports["max_debt_ebitda"].passed);
        assert_eq!(engine.breach_history.len(), 1);
        assert_eq!(engine.breach_history[0].covenant_id, "max_debt_ebitda");
        assert!(!engine.breach_history[0].is_cured);
    }

    #[test]
    fn evaluate_and_track_cures_on_recovery() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        )
        .with_cure_period(Some(90));
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 5.0)]);
        engine
            .evaluate_and_track(&metrics, date(2024, 3, 31))
            .unwrap();
        assert_eq!(engine.breach_history.len(), 1);
        assert!(!engine.breach_history[0].is_cured);
        let metrics2 = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.0)]);
        engine
            .evaluate_and_track(&metrics2, date(2024, 5, 15))
            .unwrap();
        assert!(engine.breach_history[0].is_cured);
    }

    #[test]
    fn validate_rejects_negative_cure_period() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        )
        .with_cure_period(Some(-1));
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_spec(spec);
        assert!(engine.validate().is_err());
    }

    #[test]
    fn validate_rejects_overlapping_windows() {
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec::with_metric(covenant, "debt_to_ebitda");
        let mut engine = CovenantEngine::new();
        engine.add_window(CovenantWindow {
            start: date(2024, 1, 1),
            end: date(2024, 6, 30),
            covenants: vec![spec.clone()],
        });
        engine.add_window(CovenantWindow {
            start: date(2024, 4, 1),
            end: date(2024, 12, 31),
            covenants: vec![spec],
        });
        assert!(engine.validate().is_err());
    }

    #[test]
    fn covenant_type_display_formats_correctly() {
        let ct = CovenantType::MaxDebtToEbitda { threshold: 4.5 };
        assert_eq!(ct.to_string(), "Debt/EBITDA <= 4.50x");
        let ct = CovenantType::MinInterestCoverage { threshold: 2.0 };
        assert_eq!(ct.to_string(), "Interest Coverage >= 2.00x");
    }

    #[test]
    fn covenant_type_covenant_id_is_stable() {
        let ct = CovenantType::MaxDebtToEbitda { threshold: 4.5 };
        assert_eq!(ct.covenant_id(), "max_debt_ebitda");
        let ct = CovenantType::MinInterestCoverage { threshold: 2.0 };
        assert_eq!(ct.covenant_id(), "min_interest_coverage");
    }

    #[test]
    fn covenant_type_bound_kind() {
        assert_eq!(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 }.bound_kind(),
            Some(BoundKind::AtMost)
        );
        assert_eq!(
            CovenantType::MinInterestCoverage { threshold: 2.0 }.bound_kind(),
            Some(BoundKind::AtLeast)
        );
        assert_eq!(
            CovenantType::Negative {
                restriction: "no debt".into()
            }
            .bound_kind(),
            None
        );
    }

    #[test]
    fn instance_key_is_the_declared_label() {
        let cov = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.5 },
            Tenor::quarterly(),
            "senior",
        );
        assert_eq!(cov.instance_key(), "senior");
    }
}
