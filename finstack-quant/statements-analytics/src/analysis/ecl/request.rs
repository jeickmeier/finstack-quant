//! Canonical request surfaces for simplified ECL staging and calculation.

use finstack_quant_core::{Error, Result, wire::DecimalWire};
use serde::{Deserialize, Serialize};

use super::engine::{EclConfigBuilder, MacroScenario, WeightedEclResult, measure_weighted};
use super::staging::{StageResult, StagingConfig, classify_stage_with_pds};
use super::types::{Exposure, PdTermStructure, RawPdCurve, Stage};

const SCENARIO_RATING: &str = "scenario";

/// Classify a fully specified exposure from directly supplied lifetime PDs.
///
/// This is the staging entry point for hosts that hold a single current and
/// origination lifetime PD per exposure instead of a rating-keyed PD term
/// structure. Every field of `exposure` participates in the waterfall (days
/// past due, qualitative flags, rating labels for the downgrade-notch test,
/// previous stage and performing periods for curing), so the complete
/// [`StagingConfig`] is reachable. When the exposure carries no rating labels
/// the PD-delta test still runs against the two supplied PDs.
///
/// # Arguments
///
/// * `exposure` - Credit exposure whose DPD, qualitative flags, rating labels
///   and cure state drive the staging waterfall. Its `ead`, `lgd` and `eir`
///   are not read by staging.
/// * `current_pd` - Current remaining-lifetime probability of default as a
///   decimal in `[0, 1]`, measured from the reporting date.
/// * `origination_pd` - Initial-recognition expected PD for that same remaining
///   window, conditional on survival to the reporting date, as a decimal in
///   `[0, 1]`. For original cumulative curve `F`, age `a`, and remaining horizon
///   `h`, supply `(F(a + h) - F(a)) / (1 - F(a))`.
/// * `config` - Staging policy thresholds, backstops and curing rules.
///
/// # Returns
///
/// The assigned IFRS 9 stage with its ordered trigger audit trail.
///
/// # Errors
///
/// Rejects non-finite or out-of-range PDs, maturity and policy thresholds,
/// or an unrepresentable relative PD ratio when that trigger is enabled,
/// including positive `current_pd` against zero `origination_pd`. Both PDs
/// zero mean no increase; disabling the relative trigger allows an
/// absolute-only comparison from a zero origination baseline.
/// Current and origination PDs remain separate even when rating labels match.
pub fn classify_stage(
    exposure: &Exposure,
    current_pd: f64,
    origination_pd: f64,
    config: &StagingConfig,
) -> Result<StageResult> {
    if !(0.0..=1.0).contains(&current_pd) || !(0.0..=1.0).contains(&origination_pd) {
        return Err(Error::Validation(
            "Staging PDs must be finite probabilities in [0, 1]".into(),
        ));
    }
    classify_stage_with_pds(exposure, config, || Ok(Some((origination_pd, current_pd))))
}

/// Exact scenario weight, cumulative marginal evidence and scenario-specific LGD.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EclScenario {
    /// Nonempty scenario identifier, distinct within one scenario set.
    pub id: String,
    /// Exact decimal probability in [0, 1]; the admitted set sums to one.
    pub weight: DecimalWire,
    /// Ordered year and cumulative PD observations from valuation.
    /// Omit when the scenario evidence is missing. An explicitly present
    /// schedule must cover the measurement horizon.
    pub pd_schedule: Option<Vec<(f64, f64)>>,
    /// Elected nonnegative loss fraction in [0, 1] for this scenario.
    pub lgd_override: Option<f64>,
}

/// Compute probability-weighted ECL for an exposure from cumulative-PD schedules.
///
/// This is the canonical measurement entry point for hosts that hold one
/// cumulative-PD schedule per macro scenario. The rating-keyed term-structure
/// path with a full [`super::engine::EclConfig`] is
/// [`super::engine::compute_ecl_weighted_from_curves`].
///
/// The exposure's own `ead`, `undrawn`, `ccf`, `lgd`, `eir`,
/// `remaining_maturity_years` and `ead_schedule` drive the calculation, so the
/// priced exposure at default is `ead + undrawn * ccf` exactly as in the full
/// engine. Positive scenarios carry their own PD schedules and LGD
/// elections. An absent positive schedule leaves the aggregate unavailable
/// while retaining priced entries. Zero-probability cases are identified
/// without requesting their PD curves.
///
/// # Arguments
///
/// * `exposure` - Validated credit exposure supplying EAD (drawn plus
///   `undrawn * ccf`), LGD, EIR, remaining maturity and any EAD schedule.
/// * `stage` - Assigned IFRS 9 stage selecting the 12-month, lifetime or
///   credit-impaired horizon.
/// * `scenarios` - Uniquely named scenario evidence with exact decimal
///   weights summing to one. Positive schedules use strictly ordered
///   `(time_years, cumulative_pd)` knots starting at (0, 0) and cover the
///   applicable horizon. LGD is an optional scenario loss fraction in [0, 1].
/// * `bucket_width_years` - Integration bucket width in years (`0.25` =
///   quarterly); `None` uses the canonical policy default.
///
/// # Returns
///
/// The probability-weighted ECL with its per-scenario bucket audit trail.
///
/// # Errors
///
/// Returns an error when `scenarios` is empty, the weights do not sum to
/// `1.0`, a schedule violates the cumulative-PD invariants, or the exposure
/// fails validation.
pub fn compute_ecl_weighted(
    exposure: &Exposure,
    stage: Stage,
    scenarios: &[EclScenario],
    bucket_width_years: Option<f64>,
) -> Result<WeightedEclResult> {
    if scenarios.is_empty() {
        return Err(Error::Validation(
            "At least one scenario is required for weighted ECL".to_string(),
        ));
    }

    let macro_scenarios = scenarios
        .iter()
        .map(|scenario| MacroScenario {
            id: scenario.id.clone(),
            weight: scenario.weight,
            lgd_override: scenario.lgd_override,
        })
        .collect::<Vec<_>>();
    super::engine::validate_scenario_weights(&macro_scenarios)?;
    let mut config = EclConfigBuilder::new();
    if let Some(years) = bucket_width_years {
        config = config.bucket_width(years);
    }
    let config = config.build()?;

    let mut priced = exposure.clone();
    priced.current_rating = Some(SCENARIO_RATING.to_string());
    priced.origination_rating = Some(SCENARIO_RATING.to_string());

    let required_horizon = match stage {
        Stage::Stage1 => exposure.remaining_maturity_years.min(1.0),
        Stage::Stage2 => exposure.remaining_maturity_years,
        Stage::Stage3 => return Err(Error::Validation(
            "Stage 3 requires dated carrying and recovery evidence through compute_impaired_ecl"
                .into(),
        )),
    };
    let curves = scenarios
        .iter()
        .map(|scenario| {
            if scenario.weight.0.is_zero() {
                return Ok(None);
            }
            let Some(schedule) = &scenario.pd_schedule else {
                return Ok(None);
            };
            if schedule.first() != Some(&(0.0, 0.0)) {
                return Err(Error::Validation(format!(
                    "Scenario '{}' PD schedule must start at (0, 0)",
                    scenario.id
                )));
            }
            let curve = RawPdCurve::new(SCENARIO_RATING, schedule.clone())?;
            if !curve
                .knots
                .last()
                .is_some_and(|(years, _)| *years >= required_horizon)
            {
                return Err(Error::Validation(format!(
                    "Scenario '{}' PD schedule must cover {required_horizon} years",
                    scenario.id
                )));
            }
            Ok(Some(curve))
        })
        .collect::<Result<Vec<_>>>()?;
    let sources = macro_scenarios
        .iter()
        .zip(&curves)
        .map(|(scenario, curve)| {
            (
                scenario,
                curve.as_ref().map(|curve| curve as &dyn PdTermStructure),
            )
        })
        .collect::<Vec<_>>();
    measure_weighted(&priced, stage, &sources, &config)
}

/// Compute ECL for an exposure from a single cumulative-PD schedule.
///
/// Equivalent to [`compute_ecl_weighted`] with one scenario of weight `1.0`;
/// the result carries a one-entry scenario breakdown.
///
/// # Arguments
///
/// * `exposure` - Validated credit exposure supplying EAD (drawn plus
///   `undrawn * ccf`), LGD, EIR, remaining maturity and any EAD schedule.
/// * `stage` - Assigned IFRS 9 stage selecting the 12-month, lifetime or
///   credit-impaired horizon; classify first with [`classify_stage`].
/// * `pd_schedule` - `(time_years, cumulative_pd)` knots ascending in time and
///   non-decreasing in PD; a `(0, 0)` knot is inserted when absent.
/// * `bucket_width_years` - Integration bucket width in years (`0.25` =
///   quarterly); `None` uses the canonical policy default.
///
/// # Returns
///
/// The ECL with a single-scenario bucket audit trail.
///
/// # Errors
///
/// Returns an error when the schedule violates the cumulative-PD invariants
/// or the exposure fails validation.
pub fn compute_ecl(
    exposure: &Exposure,
    stage: Stage,
    pd_schedule: Vec<(f64, f64)>,
    bucket_width_years: Option<f64>,
) -> Result<WeightedEclResult> {
    compute_ecl_weighted(
        exposure,
        stage,
        &[EclScenario {
            id: "base".to_owned(),
            weight: DecimalWire(rust_decimal::Decimal::ONE),
            pd_schedule: Some(RawPdCurve::anchor_knots(pd_schedule)),
            lgd_override: None,
        }],
        bucket_width_years,
    )
}

#[cfg(test)]
mod tests {
    use super::super::types::QualitativeFlags;
    use super::*;
    use crate::analysis::StagingTrigger;

    #[test]
    fn exact_named_ecl_scenarios_preserve_joint_loss_and_missing_evidence() {
        let exposure = Exposure {
            id: "secured".into(),
            segments: vec![],
            ead: 100.0,
            eir: 0.0,
            remaining_maturity_years: 2.0,
            lgd: 0.35,
            days_past_due: 0,
            current_rating: None,
            origination_rating: None,
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
            undrawn: 0.0,
            ccf: 0.0,
        };
        let mut scenarios = [
            EclScenario {
                id: "base".into(),
                weight: DecimalWire("0.75".parse().unwrap()),
                pd_schedule: Some(vec![(0.0, 0.0), (2.0, 0.04)]),
                lgd_override: Some(0.25),
            },
            EclScenario {
                id: "downturn".into(),
                weight: DecimalWire("0.25".parse().unwrap()),
                pd_schedule: Some(vec![(0.0, 0.0), (2.0, 0.16)]),
                lgd_override: Some(0.75),
            },
            EclScenario {
                id: "excluded".into(),
                weight: DecimalWire(rust_decimal::Decimal::ZERO),
                pd_schedule: Some(vec![(0.0, f64::NAN)]),
                lgd_override: None,
            },
        ];
        let complete = compute_ecl_weighted(&exposure, Stage::Stage2, &scenarios, None).unwrap();
        assert!((complete.ecl.unwrap() - 3.75).abs() < 1.0e-12);
        assert!(matches!(
            complete.scenario_breakdown[2].measurement,
            super::super::engine::EclScenarioMeasurement::ZeroWeight
        ));

        scenarios[1].pd_schedule = None;
        let partial = compute_ecl_weighted(&exposure, Stage::Stage2, &scenarios, None).unwrap();
        assert_eq!(partial.ecl, None);
        assert!(matches!(
            partial.scenario_breakdown[0].measurement,
            super::super::engine::EclScenarioMeasurement::Measured { .. }
        ));
        assert!(matches!(
            partial.scenario_breakdown[1].measurement,
            super::super::engine::EclScenarioMeasurement::Unavailable { .. }
        ));

        scenarios[1].weight = DecimalWire("0.2500000000000000000000000001".parse().unwrap());
        assert!(compute_ecl_weighted(&exposure, Stage::Stage2, &scenarios, None).is_err());
    }

    #[test]
    fn classify_stage_reaches_qualitative_and_rating_triggers() {
        let mut exposure = Exposure {
            id: "loan".to_string(),
            segments: vec![],
            ead: 100.0,
            eir: 0.05,
            remaining_maturity_years: 5.0,
            lgd: 0.4,
            days_past_due: 0,
            current_rating: Some("BB".to_string()),
            origination_rating: Some("A".to_string()),
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
            undrawn: 0.0,
            ccf: 0.75,
        };
        let config = StagingConfig {
            rating_downgrade_notches: 2,
            ..StagingConfig::default()
        };
        let result = classify_stage(&exposure, 0.02, 0.02, &config).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(matches!(
            result.triggers.first(),
            Some(StagingTrigger::RatingDowngrade { .. })
        ));

        exposure.current_rating = None;
        exposure.origination_rating = None;
        exposure.qualitative_flags.watchlist = true;
        let result = classify_stage(&exposure, 0.02, 0.02, &StagingConfig::default()).unwrap();
        assert_eq!(result.stage, Stage::Stage2);
        assert!(
            result
                .triggers
                .iter()
                .any(|t| matches!(t, StagingTrigger::Qualitative { .. }))
        );

        exposure.qualitative_flags.watchlist = false;
        let result = classify_stage(&exposure, 0.03, 0.015, &StagingConfig::default()).unwrap();
        assert!(matches!(
            result.triggers.first(),
            Some(StagingTrigger::PdDeltaAbsolute { .. })
        ));
    }

    #[test]
    fn compute_ecl_weighted_prices_undrawn_commitment() {
        let base = Exposure {
            id: "revolver".to_string(),
            segments: vec![],
            ead: 100.0,
            eir: 0.0,
            remaining_maturity_years: 2.0,
            lgd: 0.4,
            days_past_due: 0,
            current_rating: Some("BBB".to_string()),
            origination_rating: Some("BBB".to_string()),
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
            undrawn: 0.0,
            ccf: 0.75,
        };
        let scenarios = [EclScenario {
            id: "only".into(),
            weight: DecimalWire(rust_decimal::Decimal::ONE),
            pd_schedule: Some(vec![(0.0, 0.0), (1.0, 0.1), (2.0, 0.2)]),
            lgd_override: None,
        }];
        let drawn_only = compute_ecl_weighted(&base, Stage::Stage2, &scenarios, None).unwrap();

        let revolver = Exposure {
            undrawn: 100.0,
            ccf: 0.5,
            ..base
        };
        let priced = compute_ecl_weighted(&revolver, Stage::Stage2, &scenarios, None).unwrap();
        assert!((priced.ecl.unwrap() - 1.5 * drawn_only.ecl.unwrap()).abs() < 1e-9);
        assert_eq!(priced.scenario_breakdown.len(), 1);

        let single = compute_ecl(
            &revolver,
            Stage::Stage2,
            scenarios[0].pd_schedule.clone().unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(single.ecl, priced.ecl);
        assert_eq!(single.scenario_breakdown.len(), 1);
    }
}
