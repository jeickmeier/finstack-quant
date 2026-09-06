//! ECL calculation engine.
//!
//! Provides the core ECL computation functions:
//!
//! - [`compute_ecl`] -- single exposure, single scenario
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
//! - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.44 -- Discount rate (effective interest rate) `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.42 -- Probability-weighted scenarios `docs/REFERENCES.md#ifrs-9-impairment`

use finstack_quant_core::{Error, Result};
use finstack_quant_models::credit::lgd::DownturnLgd;
use serde::{Deserialize, Serialize};

use super::staging::{classify_stage, StageResult, StagingConfig};
use super::types::{Exposure, PdTermStructure, Stage};

// Macro scenario

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

// LGD type

/// LGD methodology selection.
///
/// Selects how the base LGD carried on an [`Exposure`] (or a scenario
/// [`MacroScenario::lgd_override`]) is converted into the effective LGD used
/// in the ECL calculation. Each non-default variant requires a companion
/// [`EclConfig`] field; [`EclConfig::validate`] enforces that the field is
/// present for the selected variant and absent otherwise (a "set but unused"
/// knob is rejected symmetrically with a "used but unset" one), so a
/// misconfigured methodology cannot be silently stamped into results.
///
/// # Variants and formulas
///
/// - [`LgdType::PointInTime`]: `LGD_eff = base`, where `base` is
///   [`Exposure::lgd`] or the scenario [`MacroScenario::lgd_override`] when
///   present. No companion field.
/// - [`LgdType::ThroughTheCycle`]: `LGD_eff = EclConfig::ttc_lgd`, a
///   cycle-average LGD that pins the effective LGD regardless of `base`.
///   Because the pin would silently discard a scenario `lgd_override`,
///   combining `ThroughTheCycle` with any scenario override is a validation
///   error (see [`compute_ecl_weighted`]).
/// - [`LgdType::Downturn`]: `LGD_eff = EclConfig::downturn_lgd.adjust(base)`,
///   applied via [`DownturnLgd::adjust`] on top of `base` (so a scenario
///   `lgd_override` sets the base and the downturn stress is layered on top
///   of it):
///   - Stressed (`DownturnMethod::StressedApproximation`):
///     `LGD_eff = clamp(base + s·√ρ·Φ⁻¹(q)·√(base·(1−base)), 0, 1)`, core's
///     documented mean-plus-Bernoulli-stdev approximation (see
///     `core/src/credit/lgd/downturn.rs` Methodology Note; Frye & Jacobs
///     (2012) is related literature only, not the formula implemented here).
///   - Regulatory floor (`DownturnMethod::RegulatoryFloor`):
///     `LGD_eff = clamp(max(base + add_on, floor), 0, 1)`.
///
/// # References
///
/// - Basel Framework CRE36.85-36.90 -- downturn LGD requirement and
///   regulatory floors.
/// - Frye, J. & Jacobs, M. (2012). "Credit Loss and Systematic Loss Given
///   Default." *Journal of Credit Risk*, 8(1), 109-140 (related literature
///   only; see [`DownturnLgd`] for the exact formula implemented).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LgdType {
    /// Point-in-time LGD from the exposure (or scenario override). No
    /// companion field required.
    PointInTime,
    /// Through-the-cycle average LGD. Requires [`EclConfig::ttc_lgd`].
    ThroughTheCycle,
    /// Downturn LGD (stressed scenario). Requires
    /// [`EclConfig::downturn_lgd`].
    Downturn,
}

// ECL configuration

/// Configuration for ECL calculation.
///
/// Controls time bucket granularity, scenario specifications, staging
/// parameters, and LGD methodology.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EclConfig {
    /// Time bucket width in years for the PD-LGD-EAD integration.
    /// Must be finite and at least 0.0001 years, limiting a 100-year
    /// exposure to one million buckets. Default: quarterly (0.25).
    pub bucket_width_years: f64,

    /// Macro scenario specifications with probability weights.
    /// Weights must sum to 1.0 and each weight must lie in \[0, 1\].
    ///
    /// When non-empty, [`compute_ecl_weighted`] and [`EclEngine`] require
    /// the same ids and weights (tolerance 1e-6, same order) as the
    /// `pd_sources` argument. The `pd_sources` weights remain the priced
    /// weights and are still validated independently.
    pub scenarios: Vec<MacroScenario>,

    /// Staging configuration for IFRS 9.
    pub staging: StagingConfig,

    /// LGD methodology label; see [`LgdType`] for the formulas each variant
    /// applies and the companion field it requires.
    pub lgd_type: LgdType,

    /// Cycle-average LGD used when `lgd_type == `[`LgdType::ThroughTheCycle`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttc_lgd: Option<f64>,

    /// Downturn LGD adjuster applied when `lgd_type == `[`LgdType::Downturn`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downturn_lgd: Option<DownturnLgd>,

    /// Assumed time (in years) from the reporting date to recovery
    /// realisation for Stage 3 (credit-impaired) exposures. Used to
    /// discount expected recoveries `(1 - LGD) x EAD` at the EIR.
    /// The allowance is current EAD less those discounted recoveries.
    /// Default: 1.0 year, a common practical recovery-lag assumption.
    #[serde(default = "default_stage3_time_to_recovery_years")]
    pub stage3_time_to_recovery_years: f64,
}

/// Default Stage 3 time-to-recovery assumption in years.
pub const DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS: f64 = 1.0;

fn default_stage3_time_to_recovery_years() -> f64 {
    DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS
}

impl EclConfig {
    /// Validate the configuration invariants: scenario weights sum to 1.0
    /// (within 1e-6), bucket width is finite and at least 0.0001 years, and
    /// at least one scenario is present.
    ///
    /// `EclConfig` exposes public fields and can be constructed directly
    /// (bypassing [`EclConfigBuilder`]), so every public entry point that
    /// consumes a config — [`compute_ecl`] and the functions that
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
            if scenario
                .lgd_override
                .is_some_and(|lgd| !(0.0..=1.0).contains(&lgd))
            {
                return Err(Error::Validation(
                    "scenario LGD must be finite and in [0, 1]".into(),
                ));
            }
            total_weight += scenario.weight;
        }
        if (total_weight - 1.0).abs() > 1e-6 {
            return Err(Error::Validation(format!(
                "Scenario weights must sum to 1.0, got {total_weight:.6}"
            )));
        }
        if !self.bucket_width_years.is_finite() || self.bucket_width_years < 1e-4 {
            return Err(Error::Validation(
                "bucket_width_years must be finite and at least 0.0001 years".to_string(),
            ));
        }
        if self.scenarios.is_empty() {
            return Err(Error::Validation(
                "At least one scenario is required".to_string(),
            ));
        }
        match self.lgd_type {
            LgdType::PointInTime => {
                if self.ttc_lgd.is_some() {
                    return Err(Error::Validation(
                        "ttc_lgd is only valid with lgd_type = ThroughTheCycle".to_string(),
                    ));
                }
                if self.downturn_lgd.is_some() {
                    return Err(Error::Validation(
                        "downturn_lgd is only valid with lgd_type = Downturn".to_string(),
                    ));
                }
            }
            LgdType::ThroughTheCycle => {
                let lgd = self.ttc_lgd.ok_or_else(|| {
                    Error::Validation("lgd_type = ThroughTheCycle requires ttc_lgd".to_string())
                })?;
                if !lgd.is_finite() || !(0.0..=1.0).contains(&lgd) {
                    return Err(Error::Validation(format!(
                        "ttc_lgd must be a finite value in [0, 1], got {lgd}"
                    )));
                }
                if self.scenarios.iter().any(|s| s.lgd_override.is_some()) {
                    return Err(Error::Validation(
                        "lgd_type = ThroughTheCycle pins the LGD; scenario lgd_override \
                         entries would be silently ignored — remove them or use \
                         PointInTime/Downturn"
                            .to_string(),
                    ));
                }
                if self.downturn_lgd.is_some() {
                    return Err(Error::Validation(
                        "downturn_lgd is only valid with lgd_type = Downturn".to_string(),
                    ));
                }
            }
            LgdType::Downturn => {
                let downturn_lgd = self.downturn_lgd.as_ref().ok_or_else(|| {
                    Error::Validation("lgd_type = Downturn requires downturn_lgd".to_string())
                })?;
                // `DownturnLgd` derives `Deserialize`, so a config parsed
                // from untrusted JSON can carry parameters that bypass its
                // constructors' invariant checks (e.g. a negative asset
                // correlation, or stress_quantile == 1.0). Without this,
                // such values reach `DownturnLgd::adjust` and silently
                // produce NaN/infinite LGD deep inside ECL computation.
                downturn_lgd.validate().map_err(|e| {
                    Error::Validation(format!("invalid downturn_lgd parameters: {e}"))
                })?;
                if self.ttc_lgd.is_some() {
                    return Err(Error::Validation(
                        "ttc_lgd is only valid with lgd_type = ThroughTheCycle".to_string(),
                    ));
                }
            }
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

/// Builder for [`EclConfig`].
///
/// Validates configuration on `build()`:
/// - Scenario weights must sum to 1.0 (within 1e-6 tolerance)
/// - Bucket width must be finite and at least 0.0001 years
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
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
    /// `build()` validates that the companion field each variant requires is
    /// present (and that no other variant's field is set); see [`LgdType`]
    /// for the formulas and requirements.
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

    /// Set the cycle-average LGD used with [`LgdType::ThroughTheCycle`].
    ///
    /// # Arguments
    ///
    /// * `lgd` - Cycle-average LGD in `[0, 1]`; validated on `build()`.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn ttc_lgd(mut self, lgd: f64) -> Self {
        self.config.ttc_lgd = Some(lgd);
        self
    }

    /// Set the downturn LGD adjuster used with [`LgdType::Downturn`].
    ///
    /// # Arguments
    ///
    /// * `adjuster` - Core downturn adjuster (stressed or regulatory-floor).
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn downturn_lgd(mut self, adjuster: DownturnLgd) -> Self {
        self.config.downturn_lgd = Some(adjuster);
        self
    }

    /// Set the Stage 3 time-to-recovery assumption in years.
    ///
    /// # Arguments
    ///
    /// * `years` - Time from reporting date to expected recovery realisation
    ///   used to discount Stage 3 recoveries (default 1.0).
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
    /// width is not finite and at least 0.0001 years, or when no scenarios are
    /// configured.
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

/// ECL result for a single time bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclBucket {
    /// Start of the time bucket (years).
    pub t_start: f64,
    /// End of the time bucket (years).
    pub t_end: f64,
    /// Unconditional default probability for the bucket,
    /// `cumPD(t_end) - cumPD(t_start)`. This is the quantity that
    /// multiplies `LGD * EAD * DF` for performing exposures. Stage 3
    /// sets it to one and measures EAD less discounted recoveries.
    pub marginal_pd: f64,
    /// LGD used for this bucket.
    pub lgd: f64,
    /// EAD used for this bucket.
    pub ead: f64,
    /// Discount factor at the bucket midpoint, or at recovery for Stage 3.
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
    /// Audit stamp: numeric mode, rounding context, and FX policy in force.
    #[serde(default)]
    pub meta: finstack_quant_core::config::ResultsMeta,
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
    /// Audit stamp: numeric mode, rounding context, and FX policy in force.
    #[serde(default)]
    pub meta: finstack_quant_core::config::ResultsMeta,
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
    /// Audit stamp: numeric mode, rounding context, and FX policy in force.
    #[serde(default)]
    pub meta: finstack_quant_core::config::ResultsMeta,
}

// Core computation (stateless)

/// Effective LGD after applying the configured [`LgdType`] to the base LGD
/// (the exposure LGD, or a scenario `lgd_override` already applied upstream).
fn effective_lgd(config: &EclConfig, base_lgd: f64) -> Result<f64> {
    match config.lgd_type {
        LgdType::PointInTime => Ok(base_lgd),
        LgdType::ThroughTheCycle => config.ttc_lgd.ok_or_else(|| {
            Error::Validation("lgd_type = ThroughTheCycle requires ttc_lgd".to_string())
        }),
        LgdType::Downturn => {
            let adjuster = config.downturn_lgd.as_ref().ok_or_else(|| {
                Error::Validation("lgd_type = Downturn requires downturn_lgd".to_string())
            })?;
            adjuster.adjust(base_lgd)
        }
    }
}

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
/// current carrying exposure less discounted recoveries with PD ≡ 1, i.e.
/// `ECL = EAD - (1 - LGD) x EAD x DF(t_recovery)` where `t_recovery` is
/// [`EclConfig::stage3_time_to_recovery_years`]. This matches IFRS 9
/// 5.5.33 / B5.5.33 (allowance = gross carrying amount − PV of expected
/// recoveries discounted at the EIR). The result carries a single bucket
/// with `marginal_pd = 1.0`. Note this also means a Stage 3 exposure with
/// zero remaining maturity still carries a positive allowance.
///
/// # Arguments
///
/// * `exposure` - Validated credit exposure providing EAD, LGD, EIR, maturity,
///   rating, and any EAD schedule for the expected-loss calculation.
/// * `stage` - Assigned IFRS 9 stage that selects the 12-month, lifetime, or
///   credit-impaired calculation horizon.
/// * `pd_source` - Term structure supplying cumulative default probabilities
///   for the exposure's current rating.
/// * `config` - ECL bucketing, scenarios, and stage-3 recovery-time policy.
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
/// use finstack_quant_statements_analytics::analysis::{
///     compute_ecl, EclConfig, Exposure, QualitativeFlags, RawPdCurve, Stage,
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
///     undrawn: 0.0,
///     ccf: 0.75,
/// };
/// let pd_curve = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02)])?;
///
/// let result = compute_ecl(&exposure, Stage::Stage1, &pd_curve, &EclConfig::default())?;
/// assert!(result.ecl > 0.0);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses. `docs/REFERENCES.md#ifrs-9-impairment`
/// - Duffie & Singleton (2003), *Credit Risk: Pricing, Measurement and Management*. `docs/REFERENCES.md#duffie-singleton-1999`
pub fn compute_ecl(
    exposure: &Exposure,
    stage: Stage,
    pd_source: &dyn PdTermStructure,
    config: &EclConfig,
) -> Result<EclResult> {
    exposure.validate()?;
    config.validate()?;

    // Stage 3: current carrying exposure less the present value of recoveries.
    // LGD is the undiscounted fraction lost at recovery. The PD curve is still
    // validated for the exposure rating so invalid curve/rating mappings are
    // not silently hidden by the shortcut.
    if stage == Stage::Stage3 {
        let rating = exposure.current_rating.as_deref().unwrap_or("NR");
        if !(0.0..=1.0).contains(&pd_source.cumulative_pd(rating, 0.0)?) {
            return Err(Error::Validation(
                "PD source must return finite probabilities in [0, 1]".into(),
            ));
        }
        let t_recovery = config.stage3_time_to_recovery_years;
        let lgd = effective_lgd(config, exposure.lgd)?;
        let ead = exposure.ead_at(0.0)?;
        let df = 1.0 / (1.0 + exposure.eir).powf(t_recovery);
        let ecl = ead - (1.0 - lgd) * ead * df;
        validate_finite_ecl(ecl)?;
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
            meta: finstack_quant_core::config::results_meta(
                &finstack_quant_core::config::FinstackConfig::default(),
            ),
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

    let lgd = effective_lgd(config, exposure.lgd)?;
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
        if !(0.0..=1.0).contains(&pd_start) || !(pd_start..=1.0).contains(&pd_end) {
            return Err(Error::Validation(
                "PD source must return finite non-decreasing probabilities in [0, 1]".into(),
            ));
        }
        let uncond_mpd = pd_end - pd_start;
        let ead = exposure.ead_at(t_mid)?;
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

    validate_finite_ecl(ecl)?;
    Ok(EclResult {
        exposure_id: exposure.id.clone(),
        stage,
        ecl,
        horizon,
        buckets: bucket_details,
        meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
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
/// * `exposure` - Credit exposure to value; scenario LGD overrides apply to a
///   cloned copy and do not mutate this input.
/// * `stage` - Assigned IFRS 9 stage used by every scenario calculation.
/// * `pd_sources` - Non-empty scenario and PD-term-structure pairs; scenario
///   weights must satisfy the configured probability validation.
/// * `config` - ECL bucketing and recovery-time calculation parameters.
///
/// # LGD methodology interaction
///
/// A scenario `lgd_override`, when present, becomes the *base* LGD that
/// [`LgdType`] is then applied to: under [`LgdType::Downturn`] the stress
/// adjustment is layered **on top of** the scenario override (override sets
/// the base, stress adjusts it). Under [`LgdType::ThroughTheCycle`] the
/// cycle-average LGD pins the effective LGD, so a scenario `lgd_override`
/// would be silently discarded -- combining the two is rejected as a
/// validation error instead.
///
/// # Returns
///
/// A [`WeightedEclResult`] containing the probability-weighted ECL and each
/// scenario's individual result.
///
/// # Errors
///
/// Returns an error if `pd_sources` is empty, if exposure validation fails,
/// if any scenario PD source cannot provide cumulative PDs for the exposure,
/// if a non-empty [`EclConfig::scenarios`] list does not match the priced
/// scenario ids and weights, or if `config.lgd_type ==
/// `[`LgdType::ThroughTheCycle`] and any scenario sets `lgd_override`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
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
///     undrawn: 0.0,
///     ccf: 0.75,
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
/// - IFRS 9 B5.5.42 -- Probability-weighted scenarios. `docs/REFERENCES.md#ifrs-9-impairment`
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
    validate_config_scenarios_match_pd_sources(config, pd_sources)?;

    let mut weighted_ecl = 0.0;
    let mut scenario_results = Vec::with_capacity(pd_sources.len());

    for (scenario, pd_source) in pd_sources {
        if config.lgd_type == LgdType::ThroughTheCycle && scenario.lgd_override.is_some() {
            return Err(Error::Validation(format!(
                "scenario '{}' has lgd_override but lgd_type = ThroughTheCycle pins the LGD",
                scenario.id
            )));
        }
        let lgd_adj = scenario.lgd_override.unwrap_or(exposure.lgd);
        let adj_exposure = Exposure {
            lgd: lgd_adj,
            ..exposure.clone()
        };
        let result = compute_ecl(&adj_exposure, stage, *pd_source, config)?;
        weighted_ecl += scenario.weight * result.ecl;
        scenario_results.push((scenario.id.clone(), scenario.weight, result));
    }

    validate_finite_ecl(weighted_ecl)?;
    Ok(WeightedEclResult {
        exposure_id: exposure.id.clone(),
        stage,
        ecl: weighted_ecl,
        scenario_breakdown: scenario_results,
        meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    })
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
        if scenario
            .lgd_override
            .is_some_and(|lgd| !(0.0..=1.0).contains(&lgd))
        {
            return Err(Error::Validation(
                "scenario LGD must be finite and in [0, 1]".into(),
            ));
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

/// Require `config.scenarios` ids and weights to match `pd_sources` when
/// the config list is non-empty.
fn validate_config_scenarios_match_pd_sources(
    config: &EclConfig,
    pd_sources: &[(&MacroScenario, &dyn PdTermStructure)],
) -> Result<()> {
    if config.scenarios.is_empty() {
        return Ok(());
    }
    if config.scenarios.len() != pd_sources.len() {
        return Err(Error::Validation(format!(
            "EclConfig.scenarios length ({}) does not match pd_sources ({})",
            config.scenarios.len(),
            pd_sources.len()
        )));
    }
    for (config_scenario, (priced_scenario, _)) in config.scenarios.iter().zip(pd_sources.iter()) {
        if config_scenario.id != priced_scenario.id {
            return Err(Error::Validation(format!(
                "EclConfig.scenarios id '{}' does not match pd_sources id '{}'",
                config_scenario.id, priced_scenario.id
            )));
        }
        if (config_scenario.weight - priced_scenario.weight).abs() > 1e-6 {
            return Err(Error::Validation(format!(
                "EclConfig.scenarios weight for '{}' ({}) does not match \
                 pd_sources weight ({})",
                config_scenario.id, config_scenario.weight, priced_scenario.weight
            )));
        }
    }
    Ok(())
}

// Stateful facade

/// Stateful ECL engine wrapping staging + calculation + aggregation.
///
/// Holds configuration and PD sources, provides a single entry point for
/// portfolio-level ECL computation.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
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
    /// returns an error if the source list is empty or if a non-empty
    /// [`EclConfig::scenarios`] list does not match the priced scenario
    /// ids and weights.
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
            meta: finstack_quant_core::config::results_meta(
                &finstack_quant_core::config::FinstackConfig::default(),
            ),
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

/// Reject arithmetic overflow before publishing an allowance.
pub(super) fn validate_finite_ecl(ecl: f64) -> Result<()> {
    if !ecl.is_finite() {
        return Err(Error::Validation(
            "ECL calculation produced a non-finite allowance".into(),
        ));
    }
    Ok(())
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
            undrawn: 0.0,
            ccf: 0.75,
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
    fn test_compute_ecl_stage1() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl(&exposure, Stage::Stage1, &curve, &config).unwrap();

        // Stage 1 horizon = min(1.0, 5.0) = 1.0
        assert!((result.horizon - 1.0).abs() < 1e-10);
        assert!(result.ecl > 0.0);
        assert_eq!(result.buckets.len(), 4); // 1.0 / 0.25 = 4 quarterly buckets
    }

    #[test]
    fn test_compute_ecl_stage2() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl(&exposure, Stage::Stage2, &curve, &config).unwrap();

        // Stage 2 horizon = remaining maturity = 5.0
        assert!((result.horizon - 5.0).abs() < 1e-10);
        assert!(result.ecl > 0.0);
        assert_eq!(result.buckets.len(), 20); // 5.0 / 0.25 = 20 buckets
    }

    #[test]
    fn test_compute_ecl_stage3_is_carrying_value_less_pv_recovery() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let result = compute_ecl(&exposure, Stage::Stage3, &curve, &config).unwrap();

        // PD ≡ 1: ECL = EAD - (1 - LGD) x EAD x DF(t_recovery), default t_recovery = 1.0
        let expected = 1_000_000.0 - 0.55 * 1_000_000.0 / 1.05_f64;
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

        let result = compute_ecl(&exposure, Stage::Stage3, &curve, &config).unwrap();
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

        let result = compute_ecl(&exposure, Stage::Stage3, &curve, &config).unwrap();
        let expected = 1_000_000.0 - 0.55 * 1_000_000.0 / 1.05_f64.powi(2);
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

        let ecl_constant = compute_ecl(&constant, Stage::Stage2, &curve, &config).unwrap();
        let ecl_amortizing = compute_ecl(&amortizing, Stage::Stage2, &curve, &config).unwrap();

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
        assert!(compute_ecl(&exposure, Stage::Stage1, &curve, &config).is_err());

        exposure.ead_schedule = Some(vec![(0.0, -1.0)]);
        assert!(compute_ecl(&exposure, Stage::Stage1, &curve, &config).is_err());
    }

    #[test]
    fn lgd_type_downturn_without_adjuster_rejected() {
        let config = EclConfigBuilder::new().lgd_type(LgdType::Downturn).build();
        assert!(config.is_err());
    }

    fn lgd_test_exposure() -> Exposure {
        // eir = 0 so DF = 1 and the bucket sum telescopes exactly to cumPD(1y).
        Exposure {
            ead: 100_000.0,
            eir: 0.0,
            remaining_maturity_years: 1.0,
            lgd: 0.45,
            ..make_exposure()
        }
    }

    fn one_year_2pct_curve() -> RawPdCurve {
        RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.02)]).unwrap()
    }

    #[test]
    fn ttc_lgd_replaces_exposure_lgd() {
        // PIT: ECL = cumPD(1y) × LGD × EAD = 0.02 × 0.45 × 100,000 = 900.
        // TTC (ttc_lgd = 0.40): ECL = 0.02 × 0.40 × 100,000 = 800.
        let pit = EclConfig::default();
        let pit_ecl = compute_ecl(
            &lgd_test_exposure(),
            Stage::Stage1,
            &one_year_2pct_curve(),
            &pit,
        )
        .unwrap()
        .ecl;
        assert!((pit_ecl - 900.0).abs() < 1e-9, "got {pit_ecl}");

        let ttc = EclConfigBuilder::new()
            .lgd_type(LgdType::ThroughTheCycle)
            .ttc_lgd(0.40)
            .build()
            .unwrap();
        let ttc_ecl = compute_ecl(
            &lgd_test_exposure(),
            Stage::Stage1,
            &one_year_2pct_curve(),
            &ttc,
        )
        .unwrap()
        .ecl;
        assert!((ttc_ecl - 800.0).abs() < 1e-9, "got {ttc_ecl}");
    }

    #[test]
    fn downturn_regulatory_floor_golden() {
        // base LGD 0.45, add_on 0.08, floor 0.25:
        //   LGD_eff = max(0.45 + 0.08, 0.25) = 0.53
        //   ECL = 0.02 × 0.53 × 100,000 = 1,060.
        let adjuster =
            finstack_quant_models::credit::lgd::DownturnLgd::regulatory_floor(0.08, 0.25).unwrap();
        let config = EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .downturn_lgd(adjuster)
            .build()
            .unwrap();
        let ecl = compute_ecl(
            &lgd_test_exposure(),
            Stage::Stage1,
            &one_year_2pct_curve(),
            &config,
        )
        .unwrap()
        .ecl;
        assert!((ecl - 1_060.0).abs() < 1e-9, "got {ecl}");
    }

    #[test]
    fn downturn_stressed_golden() {
        // stressed(rho=0.15, sensitivity=0.4, q=0.999), base 0.45:
        //   Φ⁻¹(0.999) = 3.090232...
        //   add-on = 0.4 × √0.15 × 3.090232 × √(0.45 × 0.55)
        //          = 0.4 × 0.3872983 × 3.090232 × 0.4974937 ≈ 0.238164
        //   LGD_eff ≈ 0.688164 → ECL ≈ 0.02 × 0.688164 × 100,000 ≈ 1,376.33
        let adjuster =
            finstack_quant_models::credit::lgd::DownturnLgd::stressed(0.15, 0.4, 0.999).unwrap();
        let expected_lgd = adjuster.adjust(0.45).unwrap();
        assert!(
            (expected_lgd - 0.688164).abs() < 1e-3,
            "hand check: {expected_lgd}"
        );

        let config = EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .downturn_lgd(adjuster)
            .build()
            .unwrap();
        let ecl = compute_ecl(
            &lgd_test_exposure(),
            Stage::Stage1,
            &one_year_2pct_curve(),
            &config,
        )
        .unwrap()
        .ecl;
        let expected = 0.02 * expected_lgd * 100_000.0;
        assert!((ecl - expected).abs() < 1e-9, "got {ecl}, want {expected}");
    }

    #[test]
    fn downturn_applies_to_stage3_shortcut() {
        // Stage 3, eir 0: ECL = LGD_eff × EAD = 0.53 × 100,000 = 53,000.
        let adjuster =
            finstack_quant_models::credit::lgd::DownturnLgd::regulatory_floor(0.08, 0.25).unwrap();
        let config = EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .downturn_lgd(adjuster)
            .build()
            .unwrap();
        let ecl = compute_ecl(
            &lgd_test_exposure(),
            Stage::Stage3,
            &one_year_2pct_curve(),
            &config,
        )
        .unwrap()
        .ecl;
        assert!((ecl - 53_000.0).abs() < 1e-9, "got {ecl}");
    }

    #[test]
    fn downturn_stress_applies_on_top_of_scenario_lgd_override() {
        // Documented precedence (see LgdType::Downturn docs above): a
        // scenario `lgd_override` sets the *base* LGD, and the Downturn
        // stress is layered on top of that base -- not on top of the
        // exposure's own lgd (0.45, per lgd_test_exposure()).
        //
        // regulatory_floor(add_on=0.08, floor=0.10) with override base 0.30:
        //   LGD_eff = max(0.30 + 0.08, 0.10) = 0.38 (floor not binding)
        //   ECL = cumPD(1y) x LGD_eff x EAD = 0.02 x 0.38 x 100,000 = 760.0
        //
        // If the override were ignored and exposure.lgd (0.45) were used as
        // the base instead, LGD_eff would be max(0.45 + 0.08, 0.10) = 0.53
        // and ECL = 1,060 (see downturn_regulatory_floor_golden) -- a
        // different number, so this assertion also pins the precedence.
        let adjuster =
            finstack_quant_models::credit::lgd::DownturnLgd::regulatory_floor(0.08, 0.10).unwrap();
        let config = EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .downturn_lgd(adjuster)
            .build()
            .unwrap();
        let scenario = MacroScenario {
            id: "base".to_string(),
            weight: 1.0,
            lgd_override: Some(0.30),
        };
        let curve = one_year_2pct_curve();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&scenario, &curve)];
        let result =
            compute_ecl_weighted(&lgd_test_exposure(), Stage::Stage1, &pd_sources, &config)
                .unwrap();
        assert!((result.ecl - 760.0).abs() < 1e-9, "got {}", result.ecl);
    }

    #[test]
    fn lgd_type_validation_matrix() {
        // TTC requires ttc_lgd.
        assert!(EclConfigBuilder::new()
            .lgd_type(LgdType::ThroughTheCycle)
            .build()
            .is_err());
        // Downturn requires downturn_lgd.
        assert!(EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .build()
            .is_err());
        // ttc_lgd out of range.
        assert!(EclConfigBuilder::new()
            .lgd_type(LgdType::ThroughTheCycle)
            .ttc_lgd(1.5)
            .build()
            .is_err());
        // Symmetric inert-config guard: PIT with a set-but-unused knob is rejected.
        assert!(EclConfigBuilder::new().ttc_lgd(0.4).build().is_err());
        let adj =
            finstack_quant_models::credit::lgd::DownturnLgd::regulatory_floor(0.05, 0.25).unwrap();
        assert!(EclConfigBuilder::new().downturn_lgd(adj).build().is_err());
    }

    #[test]
    fn eclconfig_validate_rejects_bad_downturn_lgd_from_untrusted_json() {
        // `DownturnLgd` derives Deserialize, so it can be embedded in an
        // EclConfig JSON payload with parameters that bypass the
        // `stressed` constructor's checks entirely (e.g. a negative
        // asset_correlation). Deserialization itself must succeed -- the
        // guard is EclConfig::validate, which now delegates to
        // DownturnLgd::validate.
        //
        // Start from a config that serializes/deserializes cleanly (so this
        // test doesn't hand-roll every nested field of StagingConfig), then
        // swap in a malformed `downturn_lgd` payload before re-parsing.
        let base = EclConfigBuilder::new()
            .lgd_type(LgdType::Downturn)
            .downturn_lgd(
                finstack_quant_models::credit::lgd::DownturnLgd::regulatory_floor(0.08, 0.25)
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut json = serde_json::to_value(&base).expect("serialize");
        json["downturn_lgd"] = serde_json::json!({
            "method": {
                "stressed_approximation": {
                    "asset_correlation": -0.5,
                    "lgd_sensitivity": 0.4,
                    "stress_quantile": 0.999
                }
            }
        });
        let parsed: EclConfig = serde_json::from_value(json)
            .expect("malformed downturn_lgd parameters still deserialize");
        let err = parsed
            .validate()
            .expect_err("negative asset_correlation must be rejected by EclConfig::validate");
        let msg = err.to_string();
        assert!(
            msg.contains("downturn_lgd"),
            "expected validation error to name downturn_lgd, got: {msg}"
        );
    }

    #[test]
    fn ttc_conflicts_with_scenario_lgd_override() {
        let config = EclConfigBuilder::new()
            .lgd_type(LgdType::ThroughTheCycle)
            .ttc_lgd(0.40)
            .build()
            .unwrap();
        let scenario = MacroScenario {
            id: "base".to_string(),
            weight: 1.0,
            lgd_override: Some(0.60),
        };
        let curve = one_year_2pct_curve();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&scenario, &curve)];
        let result =
            compute_ecl_weighted(&lgd_test_exposure(), Stage::Stage1, &pd_sources, &config);
        assert!(
            result.is_err(),
            "TTC pins LGD; a scenario override must not be silently ignored"
        );
    }

    #[test]
    fn lgd_config_serde_is_additive() {
        let json = serde_json::to_string(&EclConfig::default()).unwrap();
        assert!(!json.contains("ttc_lgd") && !json.contains("downturn_lgd"));
        let parsed: EclConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.ttc_lgd.is_none() && parsed.downturn_lgd.is_none());
    }

    #[test]
    fn test_stage1_ecl_less_than_stage2() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let s1 = compute_ecl(&exposure, Stage::Stage1, &curve, &config).unwrap();
        let s2 = compute_ecl(&exposure, Stage::Stage2, &curve, &config).unwrap();

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
            undrawn: 0.0,
            ccf: 0.75,
        };

        let config = EclConfigBuilder::new().bucket_width(0.5).build().unwrap();

        let result = compute_ecl(&exposure, Stage::Stage1, &curve, &config).unwrap();

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
        let single = compute_ecl(&exposure, Stage::Stage1, &curve, &config).unwrap();

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
        let config = EclConfigBuilder::new()
            .scenarios(vec![base_scenario, down_scenario])
            .build()
            .unwrap();

        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![
            (&config.scenarios[0], &curve as &dyn PdTermStructure),
            (&config.scenarios[1], &curve as &dyn PdTermStructure),
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

    #[test]
    fn undrawn_ccf_scales_stage1_ecl() {
        // drawn 1e6, undrawn 4e5, ccf 0.75 → EAD = 1.3e6
        // eir 0, 1y 2% PD, Stage 1, LGD 0.45 → ECL = 0.02 × 0.45 × 1.3e6
        let exposure = Exposure {
            ead: 1_000_000.0,
            undrawn: 400_000.0,
            ccf: 0.75,
            eir: 0.0,
            remaining_maturity_years: 1.0,
            lgd: 0.45,
            ..make_exposure()
        };
        let ecl = compute_ecl(
            &exposure,
            Stage::Stage1,
            &one_year_2pct_curve(),
            &EclConfig::default(),
        )
        .unwrap()
        .ecl;
        let expected = 0.02 * 0.45 * 1_300_000.0;
        assert!((ecl - expected).abs() < 1e-9, "got {ecl}, want {expected}");
    }

    #[test]
    fn config_scenarios_must_match_pd_sources() {
        let curve = one_year_2pct_curve();
        let mismatch = EclConfigBuilder::new()
            .scenarios(vec![
                MacroScenario {
                    id: "base".into(),
                    weight: 0.7,
                    lgd_override: None,
                },
                MacroScenario {
                    id: "down".into(),
                    weight: 0.3,
                    lgd_override: None,
                },
            ])
            .build()
            .unwrap();
        let priced = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&priced, &curve)];
        let err = compute_ecl_weighted(&lgd_test_exposure(), Stage::Stage1, &pd_sources, &mismatch)
            .unwrap_err();
        assert!(err.to_string().contains("does not match"), "got {err}");

        let matched = EclConfigBuilder::new()
            .scenarios(vec![
                MacroScenario {
                    id: "base".into(),
                    weight: 0.7,
                    lgd_override: None,
                },
                MacroScenario {
                    id: "down".into(),
                    weight: 0.3,
                    lgd_override: None,
                },
            ])
            .build()
            .unwrap();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![
            (&matched.scenarios[0], &curve),
            (&matched.scenarios[1], &curve),
        ];
        assert!(
            compute_ecl_weighted(&lgd_test_exposure(), Stage::Stage1, &pd_sources, &matched)
                .is_ok()
        );
    }
}
