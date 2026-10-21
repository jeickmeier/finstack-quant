//! Canonical request surfaces for simplified ECL staging and calculation.

use finstack_quant_core::{Error, Result};

use super::engine::{
    compute_ecl_weighted_from_curves, EclConfigBuilder, MacroScenario, WeightedEclResult,
};
use super::staging::{classify_stage_with_pds, StageResult, StagingConfig};
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

/// Compute probability-weighted ECL for an exposure from cumulative-PD schedules.
///
/// This is the canonical measurement entry point for hosts that hold one
/// cumulative-PD schedule per macro scenario. The rating-keyed term-structure
/// path with a full [`super::engine::EclConfig`] is
/// [`compute_ecl_weighted_from_curves`].
///
/// The exposure's own `ead`, `undrawn`, `ccf`, `lgd`, `eir`,
/// `remaining_maturity_years` and `ead_schedule` drive the calculation, so the
/// priced exposure at default is `ead + undrawn * ccf` exactly as in the full
/// engine. Schedules are anchored at `(0, 0)` when that knot is absent and
/// omitted policy values fall back to the canonical IFRS 9 defaults.
///
/// # Arguments
///
/// * `exposure` - Validated credit exposure supplying EAD (drawn plus
///   `undrawn * ccf`), LGD, EIR, remaining maturity and any EAD schedule.
/// * `stage` - Assigned IFRS 9 stage selecting the 12-month, lifetime or
///   credit-impaired horizon.
/// * `scenarios` - Probability-weighted `(weight, schedule)` pairs whose
///   weights sum to `1.0`; each schedule holds `(time_years, cumulative_pd)`
///   knots ascending in time and non-decreasing in PD.
/// * `bucket_width_years` - Integration bucket width in years (`0.25` =
///   quarterly); `None` uses the canonical policy default.
/// * `stage3_time_to_recovery_years` - Stage 3 discounting horizon to expected
///   recovery in years; `None` uses the canonical policy default.
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
    scenarios: &[(f64, Vec<(f64, f64)>)],
    bucket_width_years: Option<f64>,
    stage3_time_to_recovery_years: Option<f64>,
) -> Result<WeightedEclResult> {
    if scenarios.is_empty() {
        return Err(Error::Validation(
            "At least one scenario is required for weighted ECL".to_string(),
        ));
    }

    let macro_scenarios = scenarios
        .iter()
        .enumerate()
        .map(|(index, (weight, _))| MacroScenario {
            id: format!("scenario_{index}"),
            weight: *weight,
            lgd_override: None,
        })
        .collect::<Vec<_>>();
    let mut config = EclConfigBuilder::new();
    if let Some(years) = bucket_width_years {
        config = config.bucket_width(years);
    }
    if let Some(years) = stage3_time_to_recovery_years {
        config = config.stage3_time_to_recovery(years);
    }
    let config = config.build()?;

    let mut priced = exposure.clone();
    priced.current_rating = Some(SCENARIO_RATING.to_string());
    priced.origination_rating = Some(SCENARIO_RATING.to_string());

    let curves = scenarios
        .iter()
        .map(|(_, schedule)| {
            RawPdCurve::new(SCENARIO_RATING, RawPdCurve::anchor_knots(schedule.clone()))
        })
        .collect::<Result<Vec<_>>>()?;
    let pd_sources = macro_scenarios
        .iter()
        .zip(curves.iter())
        .map(|(scenario, curve)| (scenario, curve as &dyn PdTermStructure))
        .collect::<Vec<_>>();

    compute_ecl_weighted_from_curves(&priced, stage, &pd_sources, &config)
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
/// * `stage3_time_to_recovery_years` - Stage 3 discounting horizon to expected
///   recovery in years; `None` uses the canonical policy default.
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
    stage3_time_to_recovery_years: Option<f64>,
) -> Result<WeightedEclResult> {
    compute_ecl_weighted(
        exposure,
        stage,
        &[(1.0, pd_schedule)],
        bucket_width_years,
        stage3_time_to_recovery_years,
    )
}

#[cfg(test)]
mod tests {
    use super::super::types::QualitativeFlags;
    use super::*;
    use crate::analysis::StagingTrigger;

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
        assert!(result
            .triggers
            .iter()
            .any(|t| matches!(t, StagingTrigger::Qualitative { .. })));

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
        let scenarios = vec![(1.0, vec![(1.0, 0.1), (2.0, 0.2)])];
        let drawn_only =
            compute_ecl_weighted(&base, Stage::Stage2, &scenarios, None, None).unwrap();

        let revolver = Exposure {
            undrawn: 100.0,
            ccf: 0.5,
            ..base
        };
        let priced =
            compute_ecl_weighted(&revolver, Stage::Stage2, &scenarios, None, None).unwrap();
        assert!((priced.ecl - 1.5 * drawn_only.ecl).abs() < 1e-9);
        assert_eq!(priced.scenario_breakdown.len(), 1);

        let single =
            compute_ecl(&revolver, Stage::Stage2, scenarios[0].1.clone(), None, None).unwrap();
        assert_eq!(single.ecl, priced.ecl);
        assert_eq!(single.scenario_breakdown.len(), 1);
    }
}
