//! ECL calculation engine.
//!
//! Provides the core ECL computation functions:
//!
//! - [`compute_ecl_single`] -- single exposure, single scenario
//! - [`compute_ecl_weighted`] -- single exposure, probability-weighted across scenarios
//! - [`EclEngine`] -- stateful facade wrapping staging + calculation
//!
//! # ECL Formula
//!
//! For each time bucket \[t_{i-1}, t_i\]:
//!
//! ```text
//! bucket_ECL = [cumPD(t_i) - cumPD(t_{i-1})] * LGD * EAD * DF(t_mid)
//! ```
//!
//! where `cumPD(t)` is the cumulative (unconditional) default probability
//! from origination to `t`, and DF(t) = 1 / (1 + EIR)^t is the IFRS 9
//! effective interest rate discount factor.
//!
//! Using the unconditional marginal `cumPD(t_i) - cumPD(t_{i-1})` is
//! equivalent to integrating the survival-weighted instantaneous loss,
//! i.e. `S(t_{i-1}) * marginal_pd(t_{i-1}, t_i)` where `S(t)=1-cumPD(t)`
//! is the survival probability. The conditional marginal PD returned by
//! [`PdTermStructure::marginal_pd`] is NOT directly summable without the
//! `S(t_{i-1})` weight, so bucket-level ECL must use the unconditional
//! form above. See Duffie & Singleton (2003), *Credit Risk: Pricing,
//! Measurement and Management*, chapter 3.
//!
//! Total ECL = sum of bucket ECLs over the appropriate horizon:
//! - Stage 1: min(1 year, remaining maturity)
//! - Stage 2/3: remaining maturity
//!
//! # References
//!
//! - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses
//! - IFRS 9 B5.5.44 -- Discount rate (effective interest rate)
//! - IFRS 9 B5.5.42 -- Probability-weighted scenarios

use finstack_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::staging::{classify_stage, StageResult, StagingConfig};
use super::types::{Exposure, PdTermStructure, RawPdCurve, Stage};

// ---------------------------------------------------------------------------
// Macro scenario
// ---------------------------------------------------------------------------

/// A forward-looking macro scenario with a probability weight.
///
/// Used for probability-weighted ECL calculation per IFRS 9 B5.5.42.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroScenario {
    /// Scenario identifier (e.g., "base", "upside", "downside").
    pub id: String,

    /// Probability weight in \[0, 1\]. All scenario weights must sum to 1.0.
    pub weight: f64,

    /// Optional LGD override for this scenario (downturn LGD).
    pub lgd_override: Option<f64>,
}

// ---------------------------------------------------------------------------
// LGD type
// ---------------------------------------------------------------------------

/// LGD methodology selection.
///
/// **Not yet implemented**: the ECL engines always use the LGD carried on
/// each [`Exposure`] (optionally overridden per scenario via
/// [`MacroScenario::lgd_override`]); no TTC averaging or downturn stress is
/// applied. The enum is retained for serde wire stability.
/// [`EclConfig::validate`] rejects any value other than
/// [`LgdType::PointInTime`] so the unimplemented methodologies cannot be
/// silently stamped into results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LgdType {
    /// Point-in-time LGD from the exposure.
    PointInTime,
    /// Through-the-cycle average LGD.
    ThroughTheCycle,
    /// Downturn LGD (stressed scenario).
    Downturn,
}

// ---------------------------------------------------------------------------
// ECL configuration
// ---------------------------------------------------------------------------

/// Configuration for ECL calculation.
///
/// Controls time bucket granularity, scenario specifications, staging
/// parameters, and LGD methodology.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EclConfig {
    /// Time bucket width in years for the PD-LGD-EAD integration.
    /// Default: quarterly (0.25).
    pub bucket_width_years: f64,

    /// Macro scenario specifications with probability weights.
    /// Weights must sum to 1.0 and each weight must lie in \[0, 1\].
    ///
    /// **Note**: this field is informational for callers that store their
    /// scenario set alongside the config. The probability weights actually
    /// applied by [`compute_ecl_weighted`] (and [`EclEngine`]) are the ones
    /// carried in the `pd_sources` argument, which are validated
    /// independently; the two sets are not cross-checked.
    pub scenarios: Vec<MacroScenario>,

    /// Staging configuration for IFRS 9.
    pub staging: StagingConfig,

    /// LGD methodology label. Only [`LgdType::PointInTime`] is supported;
    /// see [`LgdType`] for details.
    pub lgd_type: LgdType,

    /// Assumed time (in years) from the reporting date to recovery
    /// realisation for Stage 3 (credit-impaired) exposures. Used to
    /// discount the `LGD x EAD` shortfall at the EIR per IFRS 9 5.5.33 /
    /// B5.5.33. Default: 1.0 year, a common practical recovery-lag
    /// assumption.
    #[serde(default = "default_stage3_time_to_recovery_years")]
    pub stage3_time_to_recovery_years: f64,
}

/// Default Stage 3 time-to-recovery assumption in years.
pub const DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS: f64 = 1.0;

fn default_stage3_time_to_recovery_years() -> f64 {
    DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS
}

impl Default for EclConfig {
    fn default() -> Self {
        super::policy::default_ecl_config()
    }
}

impl EclConfig {
    /// Validate the configuration invariants: scenario weights sum to 1.0
    /// (within 1e-6), bucket width is strictly positive, and at least one
    /// scenario is present.
    ///
    /// `EclConfig` exposes public fields and can be constructed directly
    /// (bypassing [`EclConfigBuilder`]), so every public entry point that
    /// consumes a config — [`compute_ecl_single`] and the functions that
    /// delegate to it — validates first. A zero `bucket_width_years` would
    /// otherwise produce an unbounded bucket loop.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when any invariant is violated.
    pub fn validate(&self) -> Result<()> {
        let mut total_weight = 0.0;
        for scenario in &self.scenarios {
            if !scenario.weight.is_finite() || !(0.0..=1.0).contains(&scenario.weight) {
                return Err(Error::Validation(format!(
                    "scenario '{}' weight must be in [0, 1], got {}",
                    scenario.id, scenario.weight
                )));
            }
            total_weight += scenario.weight;
        }
        if (total_weight - 1.0).abs() > 1e-6 {
            return Err(Error::Validation(format!(
                "Scenario weights must sum to 1.0, got {total_weight:.6}"
            )));
        }
        if self.bucket_width_years <= 0.0 {
            return Err(Error::Validation(
                "bucket_width_years must be positive".to_string(),
            ));
        }
        if self.scenarios.is_empty() {
            return Err(Error::Validation(
                "At least one scenario is required".to_string(),
            ));
        }
        if self.lgd_type != LgdType::PointInTime {
            return Err(Error::Validation(format!(
                "lgd_type {:?} is not implemented; only PointInTime is supported",
                self.lgd_type
            )));
        }
        if !self.stage3_time_to_recovery_years.is_finite()
            || self.stage3_time_to_recovery_years < 0.0
        {
            return Err(Error::Validation(format!(
                "stage3_time_to_recovery_years must be finite and non-negative, got {}",
                self.stage3_time_to_recovery_years
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`EclConfig`].
///
/// Validates configuration on `build()`:
/// - Scenario weights must sum to 1.0 (within 1e-6 tolerance)
/// - Bucket width must be positive
///
/// # Examples
///
/// ```rust
/// use finstack_statements_analytics::analysis::{
///     EclConfigBuilder, LgdType, MacroScenario,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = EclConfigBuilder::new()
///     .bucket_width(0.25)
///     .scenarios(vec![
///         MacroScenario { id: "base".to_string(), weight: 0.7, lgd_override: None },
///         MacroScenario { id: "downside".to_string(), weight: 0.3, lgd_override: Some(0.55) },
///     ])
///     .lgd_type(LgdType::PointInTime)
///     .build()?;
///
/// assert_eq!(config.scenarios.len(), 2);
/// # Ok(())
/// # }
/// ```
pub struct EclConfigBuilder {
    config: EclConfig,
}

impl EclConfigBuilder {
    /// Create a new builder with default configuration.
    ///
    /// # Returns
    ///
    /// A builder initialized with quarterly buckets, one 100% base scenario,
    /// default IFRS 9 staging thresholds, and point-in-time LGD.
    pub fn new() -> Self {
        Self {
            config: EclConfig::default(),
        }
    }

    /// Set the time bucket width in years.
    ///
    /// # Arguments
    ///
    /// * `years` - Width of each ECL integration bucket in years.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn bucket_width(mut self, years: f64) -> Self {
        self.config.bucket_width_years = years;
        self
    }

    /// Set the staging configuration.
    ///
    /// # Arguments
    ///
    /// * `staging` - IFRS 9 staging thresholds and curing settings.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn staging(mut self, staging: StagingConfig) -> Self {
        self.config.staging = staging;
        self
    }

    /// Replace all scenarios.
    ///
    /// # Arguments
    ///
    /// * `scenarios` - Complete probability-weighted macro scenario set.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn scenarios(mut self, scenarios: Vec<MacroScenario>) -> Self {
        self.config.scenarios = scenarios;
        self
    }

    /// Add a single scenario.
    ///
    /// # Arguments
    ///
    /// * `scenario` - Scenario to append to the existing scenario set.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn add_scenario(mut self, scenario: MacroScenario) -> Self {
        self.config.scenarios.push(scenario);
        self
    }

    /// Set the LGD methodology.
    ///
    /// Only [`LgdType::PointInTime`] is currently supported; `build()`
    /// rejects any other value (see [`LgdType`]).
    ///
    /// # Arguments
    ///
    /// * `lgd_type` - LGD methodology label to store in the configuration.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn lgd_type(mut self, lgd_type: LgdType) -> Self {
        self.config.lgd_type = lgd_type;
        self
    }

    /// Set the Stage 3 time-to-recovery assumption in years.
    ///
    /// # Arguments
    ///
    /// * `years` - Time from reporting date to expected recovery realisation
    ///   used to discount the Stage 3 shortfall (default 1.0).
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn stage3_time_to_recovery(mut self, years: f64) -> Self {
        self.config.stage3_time_to_recovery_years = years;
        self
    }

    /// Validate and build the configuration.
    ///
    /// # Returns
    ///
    /// A validated [`EclConfig`].
    ///
    /// # Errors
    ///
    /// Returns an error when scenario weights do not sum to 1.0, when bucket
    /// width is not positive, or when no scenarios are configured.
    pub fn build(self) -> Result<EclConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for EclConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// ECL result for a single time bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclBucket {
    /// Start of the time bucket (years).
    pub t_start: f64,
    /// End of the time bucket (years).
    pub t_end: f64,
    /// Unconditional default probability for the bucket,
    /// `cumPD(t_end) - cumPD(t_start)`. This is the quantity that
    /// multiplies `LGD * EAD * DF` directly; it is *not* the
    /// conditional-on-survival marginal PD.
    pub marginal_pd: f64,
    /// LGD used for this bucket.
    pub lgd: f64,
    /// EAD used for this bucket.
    pub ead: f64,
    /// Discount factor at the bucket midpoint.
    pub discount_factor: f64,
    /// ECL contribution from this bucket.
    pub ecl: f64,
}

/// ECL result for a single exposure under a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclResult {
    /// Exposure identifier.
    pub exposure_id: String,
    /// Assigned IFRS 9 stage.
    pub stage: Stage,
    /// Total ECL for this exposure under this scenario.
    pub ecl: f64,
    /// ECL horizon in years.
    pub horizon: f64,
    /// Per-bucket breakdown.
    pub buckets: Vec<EclBucket>,
}

/// Probability-weighted ECL result across scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedEclResult {
    /// Exposure identifier.
    pub exposure_id: String,
    /// Assigned IFRS 9 stage.
    pub stage: Stage,
    /// Probability-weighted ECL.
    pub ecl: f64,
    /// Per-scenario breakdown: (scenario_id, weight, result).
    pub scenario_breakdown: Vec<(String, f64, EclResult)>,
}

/// Combined staging + ECL result for one exposure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureEclResult {
    /// Stage classification result with audit trail.
    pub stage_result: StageResult,
    /// Probability-weighted ECL result.
    pub ecl_result: WeightedEclResult,
    /// Reporting-date EAD of the exposure (copied from [`Exposure::ead`]).
    /// Used by portfolio aggregation; defaults to 0.0 when deserializing
    /// results produced before this field existed.
    #[serde(default)]
    pub ead: f64,
}

// ---------------------------------------------------------------------------
// Core computation (stateless)
// ---------------------------------------------------------------------------

/// Compute ECL for a single exposure under a single scenario.
///
/// Integrates marginal PD x LGD x EAD x DF over time buckets up to the
/// appropriate horizon (12 months for Stage 1, remaining maturity for
/// Stage 2).
///
/// # Stage 3 (credit-impaired)
///
/// For Stage 3 exposures the obligor has already defaulted, so the
/// performing PD curve does not apply: the allowance is measured as the
/// present value of the expected cash shortfall with PD ≡ 1, i.e.
/// `ECL = LGD x EAD x DF(t_recovery)` where `t_recovery` is
/// [`EclConfig::stage3_time_to_recovery_years`]. This matches IFRS 9
/// 5.5.33 / B5.5.33 (allowance = gross carrying amount − PV of expected
/// recoveries discounted at the EIR). The result carries a single bucket
/// with `marginal_pd = 1.0`. Note this also means a Stage 3 exposure with
/// zero remaining maturity still carries a positive allowance.
///
/// # Arguments
///
/// * `exposure` -- The credit exposure
/// * `stage` -- Assigned IFRS 9 stage (determines ECL horizon)
/// * `pd_source` -- PD term structure for the exposure's rating
/// * `config` -- ECL calculation parameters
///
/// # Returns
///
/// An [`EclResult`] with the total ECL, measurement horizon, and bucket-level
/// contribution detail.
///
/// # Errors
///
/// Returns an error if exposure validation fails or if the PD source cannot
/// provide a cumulative PD for the exposure rating and horizon.
///
/// # Examples
///
/// ```rust
/// use finstack_statements_analytics::analysis::{
///     compute_ecl_single, EclConfig, Exposure, QualitativeFlags, RawPdCurve, Stage,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let exposure = Exposure {
///     id: "loan-1".to_string(),
///     segments: vec![],
///     ead: 100_000.0,
///     eir: 0.05,
///     remaining_maturity_years: 1.0,
///     lgd: 0.40,
///     days_past_due: 0,
///     current_rating: Some("BBB".to_string()),
///     origination_rating: Some("BBB".to_string()),
///     qualitative_flags: QualitativeFlags::default(),
///     consecutive_performing_periods: 0,
///     previous_stage: None,
///     ead_schedule: None,
/// };
/// let pd_curve = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02)])?;
///
/// let result = compute_ecl_single(&exposure, Stage::Stage1, &pd_curve, &EclConfig::default())?;
/// assert!(result.ecl > 0.0);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses.
/// - Duffie & Singleton (2003), *Credit Risk: Pricing, Measurement and Management*.
pub fn compute_ecl_single(
    exposure: &Exposure,
    stage: Stage,
    pd_source: &dyn PdTermStructure,
    config: &EclConfig,
) -> Result<EclResult> {
    exposure.validate()?;
    config.validate()?;

    // Stage 3: credit-impaired, PD ≡ 1. The allowance is the discounted
    // expected shortfall LGD x EAD x DF(t_recovery) per IFRS 9 5.5.33 /
    // B5.5.33. The PD curve is still validated for the exposure rating so
    // invalid curve/rating mappings are not silently hidden by the shortcut.
    if stage == Stage::Stage3 {
        let rating = exposure.current_rating.as_deref().unwrap_or("NR");
        pd_source.cumulative_pd(rating, 0.0)?;
        let t_recovery = config.stage3_time_to_recovery_years;
        let lgd = exposure.lgd;
        let ead = exposure.ead_at(0.0);
        let df = 1.0 / (1.0 + exposure.eir).powf(t_recovery);
        let ecl = lgd * ead * df;
        return Ok(EclResult {
            exposure_id: exposure.id.clone(),
            stage,
            ecl,
            horizon: t_recovery,
            buckets: vec![EclBucket {
                t_start: 0.0,
                t_end: t_recovery,
                marginal_pd: 1.0,
                lgd,
                ead,
                discount_factor: df,
                ecl,
            }],
        });
    }

    let horizon = match stage {
        Stage::Stage1 => 1.0_f64.min(exposure.remaining_maturity_years),
        Stage::Stage2 | Stage::Stage3 => exposure.remaining_maturity_years,
    };

    let rating = exposure.current_rating.as_deref().unwrap_or("NR");
    let dt = config.bucket_width_years;
    let n_buckets = (horizon / dt).ceil() as usize;
    let n_buckets = n_buckets.max(1); // At least one bucket

    let mut ecl = 0.0;
    let mut bucket_details = Vec::with_capacity(n_buckets);

    for i in 0..n_buckets {
        let t_start = i as f64 * dt;
        let t_end = ((i + 1) as f64 * dt).min(horizon);
        let t_mid = (t_start + t_end) / 2.0;

        // Use the unconditional bucket default probability
        // `cumPD(t_end) - cumPD(t_start)`. This is mathematically
        // equivalent to `S(t_start) * marginal_pd(t_start, t_end)` but
        // avoids losing the survival weight at the bucket boundary,
        // which otherwise systematically overstates ECL on a compound
        // curve (see module-level docs).
        let pd_start = pd_source.cumulative_pd(rating, t_start)?;
        let pd_end = pd_source.cumulative_pd(rating, t_end)?;
        let uncond_mpd = (pd_end - pd_start).max(0.0);
        let lgd = exposure.lgd;
        let ead = exposure.ead_at(t_mid);
        let df = 1.0 / (1.0 + exposure.eir).powf(t_mid);

        let bucket_ecl = uncond_mpd * lgd * ead * df;
        ecl += bucket_ecl;

        bucket_details.push(EclBucket {
            t_start,
            t_end,
            marginal_pd: uncond_mpd,
            lgd,
            ead,
            discount_factor: df,
            ecl: bucket_ecl,
        });
    }

    Ok(EclResult {
        exposure_id: exposure.id.clone(),
        stage,
        ecl,
        horizon,
        buckets: bucket_details,
    })
}

/// Compute probability-weighted ECL across macro scenarios.
///
/// IFRS 9 B5.5.42 requires that ECL reflects an unbiased and
/// probability-weighted amount determined by evaluating a range of
/// possible outcomes.
///
/// # Arguments
///
/// * `exposure` -- The credit exposure
/// * `stage` -- Assigned IFRS 9 stage
/// * `pd_sources` -- Slice of (scenario, PD source) pairs
/// * `config` -- ECL calculation parameters
///
/// # Returns
///
/// A [`WeightedEclResult`] containing the probability-weighted ECL and each
/// scenario's individual result.
///
/// # Errors
///
/// Returns an error if `pd_sources` is empty, if exposure validation fails, or
/// if any scenario PD source cannot provide cumulative PDs for the exposure.
///
/// # Examples
///
/// ```rust
/// use finstack_statements_analytics::analysis::{
///     compute_ecl_weighted, EclConfig, Exposure, MacroScenario, PdTermStructure,
///     QualitativeFlags, RawPdCurve, Stage,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let exposure = Exposure {
///     id: "loan-1".to_string(),
///     segments: vec![],
///     ead: 100_000.0,
///     eir: 0.05,
///     remaining_maturity_years: 1.0,
///     lgd: 0.40,
///     days_past_due: 0,
///     current_rating: Some("BBB".to_string()),
///     origination_rating: Some("BBB".to_string()),
///     qualitative_flags: QualitativeFlags::default(),
///     consecutive_performing_periods: 0,
///     previous_stage: None,
///     ead_schedule: None,
/// };
/// let pd_curve = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02)])?;
/// let scenario = MacroScenario { id: "base".to_string(), weight: 1.0, lgd_override: None };
/// let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
///     vec![(&scenario, &pd_curve)];
///
/// let result = compute_ecl_weighted(&exposure, Stage::Stage1, &pd_sources, &EclConfig::default())?;
/// assert_eq!(result.scenario_breakdown.len(), 1);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - IFRS 9 B5.5.42 -- Probability-weighted scenarios.
pub fn compute_ecl_weighted(
    exposure: &Exposure,
    stage: Stage,
    pd_sources: &[(&MacroScenario, &dyn PdTermStructure)],
    config: &EclConfig,
) -> Result<WeightedEclResult> {
    if pd_sources.is_empty() {
        return Err(Error::Validation(
            "At least one PD source is required for weighted ECL".to_string(),
        ));
    }
    validate_scenario_weights(pd_sources.iter().map(|(scenario, _)| *scenario))?;

    let mut weighted_ecl = 0.0;
    let mut scenario_results = Vec::with_capacity(pd_sources.len());

    for (scenario, pd_source) in pd_sources {
        let lgd_adj = scenario.lgd_override.unwrap_or(exposure.lgd);
        let adj_exposure = Exposure {
            lgd: lgd_adj,
            ..exposure.clone()
        };
        let result = compute_ecl_single(&adj_exposure, stage, *pd_source, config)?;
        weighted_ecl += scenario.weight * result.ecl;
        scenario_results.push((scenario.id.clone(), scenario.weight, result));
    }

    Ok(WeightedEclResult {
        exposure_id: exposure.id.clone(),
        stage,
        ecl: weighted_ecl,
        scenario_breakdown: scenario_results,
    })
}

/// Compute weighted ECL from serde-friendly scenario PD schedules.
///
/// This is the canonical binding path for Python/WASM-style callers that hold
/// each macro scenario as `(weight, raw_pd_curve)` pairs rather than
/// lifetime-bound `(&MacroScenario, &dyn PdTermStructure)` references.
///
/// Each scenario curve is labelled with the exposure's own rating
/// (`current_rating`, or `"NR"` when unrated) so the lookup performed by
/// [`compute_ecl_single`] matches without requiring any magic rating label
/// on the exposure.
///
/// # Errors
///
/// Returns an error if scenario weights are invalid, any raw PD curve is
/// malformed, or ECL calculation fails for any scenario.
pub fn compute_ecl_weighted_from_schedules(
    exposure: &Exposure,
    stage: Stage,
    scenarios: &[(f64, Vec<(f64, f64)>)],
    config: &EclConfig,
) -> Result<WeightedEclResult> {
    if scenarios.is_empty() {
        return Err(Error::Validation(
            "At least one scenario is required for weighted ECL".to_string(),
        ));
    }

    let scenario_defs = scenarios
        .iter()
        .enumerate()
        .map(|(idx, (weight, _))| MacroScenario {
            id: format!("scenario_{idx}"),
            weight: *weight,
            lgd_override: None,
        })
        .collect::<Vec<_>>();
    let rating = exposure.current_rating.as_deref().unwrap_or("NR");
    let curves = scenarios
        .iter()
        .map(|(_, schedule)| RawPdCurve::new(rating, schedule.clone()))
        .collect::<Result<Vec<_>>>()?;
    let pd_sources = scenario_defs
        .iter()
        .zip(curves.iter())
        .map(|(scenario, curve)| (scenario, curve as &dyn PdTermStructure))
        .collect::<Vec<_>>();

    compute_ecl_weighted(exposure, stage, &pd_sources, config)
}

/// Validate that scenario weights are finite, non-negative, and sum to 1.0.
///
/// Shared by the IFRS 9 weighted path and the CECL engine so both apply the
/// same checks to the `pd_sources` weights actually used in pricing.
pub(crate) fn validate_scenario_weights<'a>(
    scenarios: impl IntoIterator<Item = &'a MacroScenario>,
) -> Result<()> {
    let mut total_weight = 0.0;
    for scenario in scenarios {
        if !scenario.weight.is_finite() || scenario.weight < 0.0 {
            return Err(Error::Validation(format!(
                "scenario '{}' weight must be finite and non-negative",
                scenario.id
            )));
        }
        total_weight += scenario.weight;
    }
    if (total_weight - 1.0).abs() > 1e-6 {
        return Err(Error::Validation(format!(
            "scenario weights must sum to 1.0, got {total_weight:.6}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stateful facade
// ---------------------------------------------------------------------------

/// Stateful ECL engine wrapping staging + calculation + aggregation.
///
/// Holds configuration and PD sources, provides a single entry point for
/// portfolio-level ECL computation.
///
/// # Examples
///
/// ```rust
/// use finstack_statements_analytics::analysis::{
///     EclConfig, EclEngine, MacroScenario, PdTermStructure, RawPdCurve,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = EclConfig::default();
/// let scenario = MacroScenario { id: "base".to_string(), weight: 1.0, lgd_override: None };
/// let pd_curve = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02)])?;
/// let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
///     vec![(&scenario, &pd_curve)];
///
/// let engine = EclEngine::new(config, pd_sources);
/// assert_eq!(engine.config().scenarios.len(), 1);
/// # Ok(())
/// # }
/// ```
pub struct EclEngine<'a> {
    config: EclConfig,
    pd_sources: Vec<(&'a MacroScenario, &'a dyn PdTermStructure)>,
}

impl<'a> EclEngine<'a> {
    /// Create a new engine with the given configuration and PD sources.
    ///
    /// The first element in `pd_sources` is used as the base scenario for
    /// stage classification.
    ///
    /// # Arguments
    ///
    /// * `config` - ECL bucket, staging, scenario, and LGD settings.
    /// * `pd_sources` - Probability-weighted macro scenarios paired with their
    ///   PD term structures.
    ///
    /// # Returns
    ///
    /// An engine that can classify exposures and compute weighted ECL.
    ///
    /// # Errors
    ///
    /// Construction does not validate `pd_sources`; [`Self::process_exposure`]
    /// returns an error if the source list is empty.
    pub fn new(
        config: EclConfig,
        pd_sources: Vec<(&'a MacroScenario, &'a dyn PdTermStructure)>,
    ) -> Self {
        Self { config, pd_sources }
    }

    /// Classify and compute ECL for a single exposure.
    ///
    /// # Arguments
    ///
    /// * `exposure` - Exposure to stage and measure.
    ///
    /// # Returns
    ///
    /// A combined staging and probability-weighted ECL result.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine has no PD sources, if staging fails, or
    /// if ECL calculation fails for any scenario.
    pub fn process_exposure(&self, exposure: &Exposure) -> Result<ExposureEclResult> {
        // Use base scenario PD for staging
        let base_pd = self
            .pd_sources
            .first()
            .map(|(_, pd_source)| *pd_source)
            .ok_or_else(|| {
                Error::Validation("At least one PD source is required for EclEngine".to_string())
            })?;
        let stage_result = classify_stage(exposure, base_pd, &self.config.staging)?;
        let ecl_result =
            compute_ecl_weighted(exposure, stage_result.stage, &self.pd_sources, &self.config)?;
        Ok(ExposureEclResult {
            stage_result,
            ecl_result,
            ead: exposure.ead,
        })
    }

    /// Access the engine's configuration.
    ///
    /// # Returns
    ///
    /// The validated configuration stored by the engine.
    pub fn config(&self) -> &EclConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ecl::types::{QualitativeFlags, RawPdCurve};

    fn make_exposure() -> Exposure {
        Exposure {
            id: "EXP-001".to_string(),
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

    fn make_pd_curve() -> RawPdCurve {
        RawPdCurve {
            rating: "BBB".to_string(),
            knots: vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.04), (5.0, 0.10)],
        }
    }

    #[test]
    fn test_ecl_config_builder_valid() {
        let config = EclConfigBuilder::new()
            .bucket_width(0.5)
            .scenarios(vec![
                MacroScenario {
                    id: "base".into(),
                    weight: 0.6,
                    lgd_override: None,
                },
                MacroScenario {
                    id: "down".into(),
                    weight: 0.4,
                    lgd_override: Some(0.55),
                },
            ])
            .build();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert!((config.bucket_width_years - 0.5).abs() < 1e-10);
        assert_eq!(config.scenarios.len(), 2);
    }

    #[test]
    fn test_ecl_config_builder_invalid_weights() {
        let config = EclConfigBuilder::new()
            .scenarios(vec![
                MacroScenario {
                    id: "base".into(),
                    weight: 0.5,
                    lgd_override: None,
                },
                MacroScenario {
                    id: "down".into(),
                    weight: 0.3,
                    lgd_override: None,
                },
            ])
            .build();
        assert!(config.is_err());
    }

    #[test]
    fn test_ecl_config_builder_invalid_bucket_width() {
        let config = EclConfigBuilder::new().bucket_width(0.0).build();
        assert!(config.is_err());
    }

    #[test]
    fn test_compute_ecl_single_stage1() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).unwrap();

        // Stage 1 horizon = min(1.0, 5.0) = 1.0
        assert!((result.horizon - 1.0).abs() < 1e-10);
        assert!(result.ecl > 0.0);
        assert_eq!(result.buckets.len(), 4); // 1.0 / 0.25 = 4 quarterly buckets
    }

    #[test]
    fn test_compute_ecl_single_stage2() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl_single(&exposure, Stage::Stage2, &curve, &config).unwrap();

        // Stage 2 horizon = remaining maturity = 5.0
        assert!((result.horizon - 5.0).abs() < 1e-10);
        assert!(result.ecl > 0.0);
        assert_eq!(result.buckets.len(), 20); // 5.0 / 0.25 = 20 buckets
    }

    #[test]
    fn test_compute_ecl_single_stage3_is_discounted_lgd_ead() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl_single(&exposure, Stage::Stage3, &curve, &config).unwrap();

        // PD ≡ 1: ECL = LGD x EAD x DF(t_recovery), default t_recovery = 1.0
        let expected = 0.45 * 1_000_000.0 / 1.05_f64;
        assert!(
            (result.ecl - expected).abs() < 1e-6,
            "Stage 3 ECL {} vs expected {}",
            result.ecl,
            expected
        );
        assert_eq!(result.buckets.len(), 1);
        assert!((result.buckets[0].marginal_pd - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_stage3_zero_maturity_still_has_allowance() {
        let mut exposure = make_exposure();
        exposure.remaining_maturity_years = 0.0;
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl_single(&exposure, Stage::Stage3, &curve, &config).unwrap();
        assert!(
            result.ecl > 0.0,
            "Stage 3 with zero remaining maturity must not produce ECL = 0"
        );
    }

    #[test]
    fn test_stage3_time_to_recovery_configurable() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfigBuilder::new()
            .stage3_time_to_recovery(2.0)
            .build()
            .unwrap();

        let result = compute_ecl_single(&exposure, Stage::Stage3, &curve, &config).unwrap();
        let expected = 0.45 * 1_000_000.0 / 1.05_f64.powi(2);
        assert!((result.ecl - expected).abs() < 1e-6);
    }

    #[test]
    fn test_ead_schedule_reduces_lifetime_ecl() {
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let constant = make_exposure();
        let mut amortizing = make_exposure();
        // Level amortization from 1,000,000 down to 0 at maturity.
        amortizing.ead_schedule = Some(vec![(0.0, 1_000_000.0), (5.0, 0.0)]);

        let ecl_constant = compute_ecl_single(&constant, Stage::Stage2, &curve, &config).unwrap();
        let ecl_amortizing =
            compute_ecl_single(&amortizing, Stage::Stage2, &curve, &config).unwrap();

        assert!(
            ecl_amortizing.ecl < ecl_constant.ecl,
            "Amortizing profile ({}) must reduce lifetime ECL vs constant ({})",
            ecl_amortizing.ecl,
            ecl_constant.ecl
        );
        // First bucket midpoint EAD reflects the schedule.
        let first = &ecl_amortizing.buckets[0];
        let expected_ead = 1_000_000.0 * (1.0 - (first.t_start + first.t_end) / 2.0 / 5.0);
        assert!((first.ead - expected_ead).abs() < 1e-6);
    }

    #[test]
    fn test_invalid_ead_schedule_rejected() {
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let mut exposure = make_exposure();
        exposure.ead_schedule = Some(vec![(1.0, 100.0), (1.0, 50.0)]);
        assert!(compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).is_err());

        exposure.ead_schedule = Some(vec![(0.0, -1.0)]);
        assert!(compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).is_err());
    }

    #[test]
    fn test_lgd_type_non_default_rejected() {
        let config = EclConfigBuilder::new().lgd_type(LgdType::Downturn).build();
        assert!(config.is_err());
    }

    #[test]
    fn test_stage1_ecl_less_than_stage2() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let s1 = compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).unwrap();
        let s2 = compute_ecl_single(&exposure, Stage::Stage2, &curve, &config).unwrap();

        // Stage 1 (12-month) ECL must be less than Stage 2 (lifetime)
        assert!(
            s1.ecl < s2.ecl,
            "Stage 1 ECL ({}) should be < Stage 2 ECL ({})",
            s1.ecl,
            s2.ecl
        );
    }

    #[test]
    fn test_ecl_hand_computed() {
        // Simple 2-bucket test with known marginal PDs.
        // Curve: cumulative PD at t=0 is 0, at t=0.5 is 0.01, at t=1.0 is 0.02
        let curve = RawPdCurve {
            rating: "TEST".to_string(),
            knots: vec![(0.0, 0.0), (0.5, 0.01), (1.0, 0.02)],
        };
        let exposure = Exposure {
            id: "HAND".to_string(),
            segments: vec![],
            ead: 100_000.0,
            eir: 0.0, // No discounting for simplicity
            remaining_maturity_years: 1.0,
            lgd: 0.40,
            days_past_due: 0,
            current_rating: Some("TEST".to_string()),
            origination_rating: Some("TEST".to_string()),
            qualitative_flags: QualitativeFlags::default(),
            consecutive_performing_periods: 0,
            previous_stage: None,
            ead_schedule: None,
        };

        let config = EclConfigBuilder::new().bucket_width(0.5).build().unwrap();

        let result = compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).unwrap();

        // Bucket 1: [0, 0.5]
        // uncond_mpd = cumPD(0.5) - cumPD(0.0) = 0.01 - 0.00 = 0.01
        // bucket_ecl = 0.01 * 0.40 * 100000 * 1.0 = 400.0
        //
        // Bucket 2: [0.5, 1.0]
        // uncond_mpd = cumPD(1.0) - cumPD(0.5) = 0.02 - 0.01 = 0.01
        // bucket_ecl = 0.01 * 0.40 * 100000 * 1.0 = 400.0
        //
        // Total ECL = 800.0. Using the conditional marginal PD without
        // a survival weight would incorrectly yield ~804, which is the
        // bug fixed by using unconditional PD differences.
        assert_eq!(result.buckets.len(), 2);
        assert!((result.buckets[0].ecl - 400.0).abs() < 1e-10);
        assert!((result.buckets[1].ecl - 400.0).abs() < 1e-10);
        assert!((result.ecl - 800.0).abs() < 1e-10);
    }

    #[test]
    fn test_scenario_weighting() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        // Single scenario with weight 1.0 should equal unweighted
        let single = compute_ecl_single(&exposure, Stage::Stage1, &curve, &config).unwrap();

        let scenario = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];
        let weighted =
            compute_ecl_weighted(&exposure, Stage::Stage1, &pd_sources, &config).unwrap();

        assert!(
            (single.ecl - weighted.ecl).abs() < 1e-10,
            "Single-scenario weighted ECL should equal unweighted: {} vs {}",
            single.ecl,
            weighted.ecl
        );
    }

    #[test]
    fn test_scenario_weighting_two_scenarios() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let base_scenario = MacroScenario {
            id: "base".into(),
            weight: 0.6,
            lgd_override: None,
        };
        let down_scenario = MacroScenario {
            id: "downside".into(),
            weight: 0.4,
            lgd_override: Some(0.60), // Higher LGD in downside
        };

        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![
            (&base_scenario, &curve as &dyn PdTermStructure),
            (&down_scenario, &curve as &dyn PdTermStructure),
        ];

        let result = compute_ecl_weighted(&exposure, Stage::Stage1, &pd_sources, &config).unwrap();

        // Verify manual calculation
        let base_ecl = result.scenario_breakdown[0].2.ecl;
        let down_ecl = result.scenario_breakdown[1].2.ecl;
        let expected = 0.6 * base_ecl + 0.4 * down_ecl;

        assert!(
            (result.ecl - expected).abs() < 1e-6,
            "Weighted ECL mismatch: {} vs expected {}",
            result.ecl,
            expected
        );

        // Downside ECL should be higher due to higher LGD
        assert!(down_ecl > base_ecl);
    }

    #[test]
    fn test_ecl_engine_process_exposure() {
        let curve = make_pd_curve();
        let config = EclConfig::default();
        let scenario = &config.scenarios[0];

        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(scenario, &curve as &dyn PdTermStructure)];
        let engine = EclEngine::new(config.clone(), pd_sources);

        let exposure = make_exposure();
        let result = engine.process_exposure(&exposure).unwrap();

        assert_eq!(result.stage_result.stage, Stage::Stage1);
        assert!(result.ecl_result.ecl > 0.0);
    }

    #[test]
    fn test_compute_ecl_weighted_rejects_empty_pd_sources() {
        let exposure = make_exposure();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = Vec::new();
        let result =
            compute_ecl_weighted(&exposure, Stage::Stage1, &pd_sources, &EclConfig::default());

        assert!(result.is_err());
    }

    #[test]
    fn test_ecl_engine_rejects_empty_pd_sources() {
        let engine = EclEngine::new(EclConfig::default(), Vec::new());
        let exposure = make_exposure();
        let result = engine.process_exposure(&exposure);

        assert!(result.is_err());
    }
}
