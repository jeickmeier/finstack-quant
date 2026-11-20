//! ECL calculation engine.
//!
//! Provides the core ECL computation functions:
//!
//! - [`compute_ecl_from_curve`] -- single exposure, single scenario
//! - [`compute_ecl_weighted_from_curves`] -- single exposure, probability-weighted across scenarios
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
//! - Stage 2: remaining maturity
//!
//! # References
//!
//! - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.44 -- Discount rate (effective interest rate) `docs/REFERENCES.md#ifrs-9-impairment`
//! - IFRS 9 B5.5.42 -- Probability-weighted scenarios `docs/REFERENCES.md#ifrs-9-impairment`

use finstack_quant_core::wire::DecimalWire;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::credit::lgd::DownturnLgd;
use serde::{Deserialize, Serialize};

use super::staging::StagingConfig;
use super::types::{Exposure, PdTermStructure, Stage};

// Macro scenario

/// A forward-looking macro scenario with a probability weight.
///
/// Used for probability-weighted ECL calculation per IFRS 9 B5.5.42.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MacroScenario {
    /// Scenario identifier (e.g., "base", "upside", "downside").
    pub id: String,

    /// Exact decimal probability in [0, 1]. The scenario set sums to exactly one.
    pub weight: DecimalWire,

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
///   error (see [`compute_ecl_weighted_from_curves`]).
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// Controls time bucket granularity, staging parameters, and LGD methodology.
/// Probability-weighted scenarios are supplied once, alongside their PD sources
/// to [`compute_ecl_weighted_from_curves`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EclConfig {
    /// Time bucket width in years for the PD-LGD-EAD integration.
    /// Must be finite and at least 0.0001 years, limiting a 100-year
    /// exposure to one million buckets. Default: quarterly (0.25).
    pub bucket_width_years: f64,

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
}

impl EclConfig {
    /// Validate staging thresholds, finite bucket width, LGD methodology
    /// parameters. Scenario weights are validated
    /// where PD sources are supplied.
    ///
    /// `EclConfig` exposes public fields and can be constructed directly
    /// (bypassing [`EclConfigBuilder`]), so every public entry point that
    /// consumes a config — [`compute_ecl_from_curve`] and the functions that
    /// delegate to it — validates first. A zero `bucket_width_years` would
    /// otherwise produce an unbounded bucket loop.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when any invariant is violated.
    pub fn validate(&self) -> Result<()> {
        self.staging.validate()?;
        if !self.bucket_width_years.is_finite() || self.bucket_width_years < 1e-4 {
            return Err(Error::Validation(
                "bucket_width_years must be finite and at least 0.0001 years".to_string(),
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
        Ok(())
    }
}

/// Builder for [`EclConfig`].
///
/// Validates configuration on `build()`:
/// - Bucket width must be finite and at least 0.0001 years
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
///     EclConfigBuilder, LgdType,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = EclConfigBuilder::new()
///     .bucket_width(0.25)
///     .lgd_type(LgdType::PointInTime)
///     .build()?;
///
/// assert_eq!(config.bucket_width_years, 0.25);
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
    /// A builder initialized with quarterly buckets, default IFRS 9 staging
    /// thresholds, and point-in-time LGD.
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

    /// Validate and build the configuration.
    ///
    /// # Returns
    ///
    /// A validated [`EclConfig`].
    ///
    /// # Errors
    ///
    /// Returns an error for a bucket width below 0.0001 years or non-finite,
    /// incompatible LGD methodology settings, or an invalid recovery horizon.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// One admissible macro-scenario row and its measured or missing allowance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EclScenarioOutcome {
    /// Explicit scenario identity in the portfolio policy's declared order.
    pub id: String,
    /// Exact scenario probability as supplied, including any zero-weight rows.
    pub weight: DecimalWire,
    /// Elected scenario LGD override as a decimal fraction.
    pub lgd_override: Option<f64>,
    /// Underlying reserve calculation and status.
    pub measurement: EclScenarioMeasurement,
}

/// Availability of one scenario allowance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum EclScenarioMeasurement {
    /// Weight was exactly zero. No forecast or repricing was run.
    ZeroWeight,
    /// Positive-weight scenario with measured allowance and bucket detail.
    Measured {
        /// Complete same-basis allowance from the admitted PD and LGD.
        result: Box<EclResult>,
        /// Weighted contribution to the total on the same basis.
        weighted_allowance: f64,
    },
    /// Positive-weight scenario whose measurement evidence is incomplete.
    Unavailable {
        /// Explanation of the required source evidence that is missing.
        reason: String,
    },
}

/// Probability-weighted allowance and completeness evidence across named scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WeightedEclResult {
    /// Exposure identifier.
    pub exposure_id: String,
    /// Assigned impairment stage.
    pub stage: Stage,
    /// Aggregate allowance, absent when any positive-probability scenario is missing.
    pub ecl: Option<f64>,
    /// Elected scenarios in caller order, including zero-weight entries.
    pub scenario_breakdown: Vec<EclScenarioOutcome>,
    /// Numeric and reporting provenance of the complete or partial calculation.
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
/// # Arguments
///
/// * `exposure` - Validated credit exposure providing EAD, LGD, EIR, maturity,
///   rating, and any EAD schedule for the expected-loss calculation.
/// * `stage` - Stage 1 selects the 12-month horizon and Stage 2 selects the remaining lifetime.
/// * `pd_source` - Term structure supplying cumulative default probabilities
///   for the exposure's current rating.
/// * `config` - ECL bucketing and LGD methodology.
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
///     compute_ecl_from_curve, EclConfig, Exposure, QualitativeFlags, RawPdCurve, Stage,
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
/// let result = compute_ecl_from_curve(&exposure, Stage::Stage1, &pd_curve, &EclConfig::default())?;
/// assert!(result.ecl > 0.0);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - IFRS 9 B5.5.28-33 -- Measurement of expected credit losses. `docs/REFERENCES.md#ifrs-9-impairment`
/// - Duffie & Singleton (2003), *Credit Risk: Pricing, Measurement and Management*. `docs/REFERENCES.md#duffie-singleton-1999`
pub fn compute_ecl_from_curve(
    exposure: &Exposure,
    stage: Stage,
    pd_source: &dyn PdTermStructure,
    config: &EclConfig,
) -> Result<EclResult> {
    exposure.validate()?;
    config.validate()?;

    let horizon = match stage {
        Stage::Stage1 => 1.0_f64.min(exposure.remaining_maturity_years),
        Stage::Stage2 => exposure.remaining_maturity_years,
        Stage::Stage3 => return Err(Error::Validation(
            "Stage 3 requires dated carrying and recovery evidence through compute_impaired_ecl"
                .into(),
        )),
    };

    let rating = exposure.current_rating.as_deref().unwrap_or("NR");
    let width = config.bucket_width_years;
    let regular_count = (horizon / width).ceil() as usize;
    let mut edges = Vec::with_capacity(regular_count + 1);
    edges.push(0.0);
    edges.extend(
        (1..regular_count)
            .map(|i| i as f64 * width)
            .filter(|&time| time < horizon),
    );
    for knots in [pd_source.knots(rating), exposure.ead_schedule.as_deref()]
        .into_iter()
        .flatten()
    {
        edges.extend(
            knots
                .iter()
                .map(|&(time, _)| time)
                .filter(|&time| time > 0.0 && time < horizon),
        );
    }
    edges.push(horizon);
    edges.sort_unstable_by(f64::total_cmp);
    edges.dedup_by(|next, previous| {
        // A published knot and a generated edge can represent the same date.
        // Retain the exact final horizon when they differ by rounding only.
        if (*next - *previous).abs() <= 8.0 * f64::EPSILON * next.abs().max(previous.abs()) {
            *previous = *next;
            true
        } else {
            false
        }
    });

    let lgd = effective_lgd(config, exposure.lgd)?;
    let mut ecl = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut bucket_details = Vec::with_capacity(edges.len() - 1);
    for edge in edges.windows(2) {
        let [t_start, t_end] = [edge[0], edge[1]];
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
        ecl.add(bucket_ecl)?;

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

    let ecl = ecl.get_total()?;
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
/// * `pd_sources` - Non-empty pairs defining each scenario exactly once:
///   its ID, probability weight, optional decimal LGD override in `[0, 1]`,
///   and reporting-date PD curve. Weights must be finite, non-negative, and
///   sum to 1 within `1e-6`.
/// * `config` - Integration bucket width in years, LGD methodology, staging
///   policy.
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
/// if scenario weights or LGD overrides are invalid, or if `config.lgd_type ==
/// `[`LgdType::ThroughTheCycle`] and any scenario sets `lgd_override`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
///     compute_ecl_weighted_from_curves, EclConfig, Exposure, MacroScenario, PdTermStructure,
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
/// let scenario = MacroScenario { id: "base".to_string(), weight: finstack_quant_core::wire::DecimalWire("1.0".parse()?), lgd_override: None };
/// let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
///     vec![(&scenario, &pd_curve)];
///
/// let result = compute_ecl_weighted_from_curves(&exposure, Stage::Stage1, &pd_sources, &EclConfig::default())?;
/// assert_eq!(result.scenario_breakdown.len(), 1);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - IFRS 9 B5.5.42 -- Probability-weighted scenarios. `docs/REFERENCES.md#ifrs-9-impairment`
pub fn compute_ecl_weighted_from_curves(
    exposure: &Exposure,
    stage: Stage,
    pd_sources: &[(&MacroScenario, &dyn PdTermStructure)],
    config: &EclConfig,
) -> Result<WeightedEclResult> {
    let available = pd_sources
        .iter()
        .map(|&(scenario, source)| (scenario, Some(source)))
        .collect::<Vec<_>>();
    measure_weighted(exposure, stage, &available, config)
}

pub(super) fn measure_weighted(
    exposure: &Exposure,
    stage: Stage,
    pd_sources: &[(&MacroScenario, Option<&dyn PdTermStructure>)],
    config: &EclConfig,
) -> Result<WeightedEclResult> {
    use finstack_quant_core::math::summation::NeumaierAccumulator;
    if pd_sources.is_empty() {
        return Err(Error::Validation(
            "At least one impairment scenario is required".into(),
        ));
    }
    validate_scenario_weights(pd_sources.iter().map(|(scenario, _)| *scenario))?;
    config.validate()?;
    exposure.validate()?;

    let mut total = NeumaierAccumulator::new();
    let mut complete = true;
    let mut scenario_breakdown = Vec::with_capacity(pd_sources.len());
    for &(scenario, source) in pd_sources {
        let measurement = if scenario.weight.0.is_zero() {
            EclScenarioMeasurement::ZeroWeight
        } else if let Some(source) = source {
            if config.lgd_type == LgdType::ThroughTheCycle && scenario.lgd_override.is_some() {
                return Err(Error::Validation(format!(
                    "Scenario '{}' cannot override the elected through-the-cycle LGD",
                    scenario.id
                )));
            }
            let adjusted = Exposure {
                lgd: scenario.lgd_override.unwrap_or(exposure.lgd),
                ..exposure.clone()
            };
            let result = compute_ecl_from_curve(&adjusted, stage, source, config)?;
            let weighted_allowance = weighted_allowance(scenario.weight, result.ecl)?;
            total.add(weighted_allowance)?;
            EclScenarioMeasurement::Measured {
                result: Box::new(result),
                weighted_allowance,
            }
        } else {
            complete = false;
            EclScenarioMeasurement::Unavailable {
                reason: "missing reporting-date cumulative default curve".into(),
            }
        };
        scenario_breakdown.push(EclScenarioOutcome {
            id: scenario.id.clone(),
            weight: scenario.weight,
            lgd_override: scenario.lgd_override,
            measurement,
        });
    }

    Ok(WeightedEclResult {
        exposure_id: exposure.id.clone(),
        stage,
        ecl: if complete {
            Some(total.get_total()?)
        } else {
            None
        },
        scenario_breakdown,
        meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    })
}

/// Admit ordered, distinct scenario probabilities without rounding their sum.
pub(crate) fn validate_scenario_weights<'a>(
    scenarios: impl IntoIterator<Item = &'a MacroScenario>,
) -> Result<()> {
    use rust_decimal::Decimal;

    let mut ids = std::collections::BTreeSet::new();
    let mut remaining = Decimal::ONE;
    for scenario in scenarios {
        if scenario.id.trim().is_empty() || !ids.insert(scenario.id.as_str()) {
            return Err(Error::Validation(format!(
                "Expected one nonempty and unique impairment scenario ID, got '{}'",
                scenario.id
            )));
        }
        let weight = scenario.weight.0;
        if weight < Decimal::ZERO || weight > Decimal::ONE {
            return Err(Error::Validation(format!(
                "Scenario '{}' probability must be in [0, 1]",
                scenario.id
            )));
        }
        if weight > remaining {
            return Err(Error::Validation(
                "Scenario probabilities exceed exactly one".into(),
            ));
        }
        // Each accepted subtraction remains in [0, 1] and preserves the
        // maximum input scale, including 28 fractional digits.
        remaining = remaining.checked_sub(weight).ok_or_else(|| {
            Error::Validation("Scenario probability arithmetic overflowed".into())
        })?;
        if scenario
            .lgd_override
            .is_some_and(|lgd| !(0.0..=1.0).contains(&lgd))
        {
            return Err(Error::Validation(format!(
                "Scenario '{}' LGD must be finite and in [0, 1]",
                scenario.id
            )));
        }
    }
    if remaining != Decimal::ZERO {
        return Err(Error::Validation(format!(
            "Scenario probabilities must sum to exactly one; remainder is {remaining}"
        )));
    }
    Ok(())
}

pub(super) fn weighted_allowance(weight: DecimalWire, allowance: f64) -> Result<f64> {
    use rust_decimal::prelude::ToPrimitive;
    let decimal = weight.0;
    let scalar = decimal.to_f64().ok_or_else(|| {
        Error::Validation(
            "Impairment scenario probability cannot cross the numerical solver boundary".into(),
        )
    })?;
    let product = scalar * allowance;
    if !product.is_finite() || (product == 0.0 && !decimal.is_zero() && allowance != 0.0) {
        return Err(Error::Validation(
            "Weighted allowance exceeds the representable numeric domain".into(),
        ));
    }
    Ok(product)
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
    use crate::analysis::ecl::staging::{StageResult, classify_stage_from_curves};
    use crate::analysis::ecl::types::{QualitativeFlags, RawPdCurve};

    struct Staged {
        stage_result: StageResult,
        ecl_result: WeightedEclResult,
    }

    /// Stage `exposure` against `current` vs its own `origination` snapshot,
    /// then measure ECL under a single base scenario on `current`.
    fn stage_and_measure(
        exposure: &Exposure,
        current: &dyn PdTermStructure,
        origination: &dyn PdTermStructure,
        elapsed_years: f64,
    ) -> Staged {
        let config = EclConfig::default();
        let scenario = MacroScenario {
            id: "base".into(),
            weight: finstack_quant_core::wire::DecimalWire("1.0".parse().unwrap()),
            lgd_override: None,
        };
        let stage_result = classify_stage_from_curves(
            exposure,
            current,
            origination,
            elapsed_years,
            &config.staging,
        )
        .unwrap();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&scenario, current)];
        let ecl_result =
            compute_ecl_weighted_from_curves(exposure, stage_result.stage, &pd_sources, &config)
                .unwrap();
        Staged {
            stage_result,
            ecl_result,
        }
    }

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
        RawPdCurve::new(
            "BBB",
            vec![(0.0, 0.0), (1.0, 0.02), (2.0, 0.04), (5.0, 0.10)],
        )
        .expect("valid BBB PD curve")
    }

    #[test]
    fn test_ecl_config_builder_valid() {
        let config = EclConfigBuilder::new().bucket_width(0.5).build().unwrap();
        assert_eq!(config.bucket_width_years, 0.5);
    }

    #[test]
    fn weighted_ecl_rejects_invalid_scenario_weights() {
        let scenario = MacroScenario {
            id: "base".into(),
            weight: finstack_quant_core::wire::DecimalWire("0.8".parse().unwrap()),
            lgd_override: None,
        };
        let curve = make_pd_curve();
        let result = compute_ecl_weighted_from_curves(
            &make_exposure(),
            Stage::Stage1,
            &[(&scenario, &curve)],
            &EclConfig::default(),
        );
        assert!(result.is_err());
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

        let result = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();

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

        let result = compute_ecl_from_curve(&exposure, Stage::Stage2, &curve, &config).unwrap();

        // Stage 2 horizon = remaining maturity = 5.0
        assert!((result.horizon - 5.0).abs() < 1e-10);
        assert!(result.ecl > 0.0);
        assert_eq!(result.buckets.len(), 20); // 5.0 / 0.25 = 20 buckets
    }

    #[test]
    fn performing_grid_splits_at_known_knots_and_exact_maturity() {
        let mut exposure = make_exposure();
        exposure.remaining_maturity_years = 0.75;
        exposure.eir = 0.0;
        exposure.lgd = 0.5;
        exposure.ead_schedule = Some(vec![(0.0, 100.0), (0.4, 60.0), (0.75, 60.0)]);
        let rating = exposure.current_rating.as_deref().unwrap_or("NR");
        let curve = RawPdCurve::new(rating, vec![(0.0, 0.0), (0.3, 0.03), (0.75, 0.06)]).unwrap();
        let config = EclConfigBuilder::new().bucket_width(0.5).build().unwrap();
        let result = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();
        let ends = result
            .buckets
            .iter()
            .map(|bucket| bucket.t_end)
            .collect::<Vec<_>>();
        assert_eq!(ends, [0.3, 0.4, 0.5, 0.75]);
        let expected = 0.5 * (0.03 * 85.0 + 0.03 / 0.45 * (0.1 * 65.0 + 0.35 * 60.0));
        assert!((result.ecl - expected).abs() < 1e-12);
        exposure.remaining_maturity_years = 0.0;
        let result = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();
        assert!(result.buckets.is_empty());
        assert_eq!(result.ecl, 0.0);
    }

    #[test]
    fn test_ead_schedule_reduces_lifetime_ecl() {
        let curve = make_pd_curve();
        let config = EclConfig::default();

        let constant = make_exposure();
        let mut amortizing = make_exposure();
        // Level amortization from 1,000,000 down to 0 at maturity.
        amortizing.ead_schedule = Some(vec![(0.0, 1_000_000.0), (5.0, 0.0)]);

        let ecl_constant =
            compute_ecl_from_curve(&constant, Stage::Stage2, &curve, &config).unwrap();
        let ecl_amortizing =
            compute_ecl_from_curve(&amortizing, Stage::Stage2, &curve, &config).unwrap();

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
        assert!(compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).is_err());

        exposure.ead_schedule = Some(vec![(0.0, -1.0)]);
        assert!(compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).is_err());
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
        let pit_ecl = compute_ecl_from_curve(
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
        let ttc_ecl = compute_ecl_from_curve(
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
        let ecl = compute_ecl_from_curve(
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
        let ecl = compute_ecl_from_curve(
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
            weight: finstack_quant_core::wire::DecimalWire("1.0".parse().unwrap()),
            lgd_override: Some(0.30),
        };
        let curve = one_year_2pct_curve();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&scenario, &curve)];
        let result = compute_ecl_weighted_from_curves(
            &lgd_test_exposure(),
            Stage::Stage1,
            &pd_sources,
            &config,
        )
        .unwrap();
        assert!(
            (result.ecl.unwrap() - 760.0).abs() < 1e-9,
            "got {:?}",
            result.ecl
        );
    }

    #[test]
    fn lgd_type_validation_matrix() {
        // TTC requires ttc_lgd.
        assert!(
            EclConfigBuilder::new()
                .lgd_type(LgdType::ThroughTheCycle)
                .build()
                .is_err()
        );
        // Downturn requires downturn_lgd.
        assert!(
            EclConfigBuilder::new()
                .lgd_type(LgdType::Downturn)
                .build()
                .is_err()
        );
        // ttc_lgd out of range.
        assert!(
            EclConfigBuilder::new()
                .lgd_type(LgdType::ThroughTheCycle)
                .ttc_lgd(1.5)
                .build()
                .is_err()
        );
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
            weight: finstack_quant_core::wire::DecimalWire("1.0".parse().unwrap()),
            lgd_override: Some(0.60),
        };
        let curve = one_year_2pct_curve();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![(&scenario, &curve)];
        let result = compute_ecl_weighted_from_curves(
            &lgd_test_exposure(),
            Stage::Stage1,
            &pd_sources,
            &config,
        );
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

        let s1 = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();
        let s2 = compute_ecl_from_curve(&exposure, Stage::Stage2, &curve, &config).unwrap();

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
        let curve = RawPdCurve::new("TEST", vec![(0.0, 0.0), (0.5, 0.01), (1.0, 0.02)])
            .expect("valid TEST PD curve");
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

        let result = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();

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
        let single = compute_ecl_from_curve(&exposure, Stage::Stage1, &curve, &config).unwrap();

        let scenario = MacroScenario {
            id: "base".into(),
            weight: finstack_quant_core::wire::DecimalWire("1.0".parse().unwrap()),
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];
        let weighted =
            compute_ecl_weighted_from_curves(&exposure, Stage::Stage1, &pd_sources, &config)
                .unwrap();

        assert!(
            (single.ecl - weighted.ecl.unwrap()).abs() < 1e-10,
            "Single-scenario weighted ECL should equal unweighted: {} vs {}",
            single.ecl,
            weighted.ecl.unwrap()
        );
    }

    #[test]
    fn test_scenario_weighting_two_scenarios() {
        let exposure = make_exposure();
        let curve = make_pd_curve();
        let base_scenario = MacroScenario {
            id: "base".into(),
            weight: finstack_quant_core::wire::DecimalWire("0.6".parse().unwrap()),
            lgd_override: None,
        };
        let down_scenario = MacroScenario {
            id: "downside".into(),
            weight: finstack_quant_core::wire::DecimalWire("0.4".parse().unwrap()),
            lgd_override: Some(0.60), // Higher LGD in downside
        };
        let config = EclConfig::default();

        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = vec![
            (&base_scenario, &curve as &dyn PdTermStructure),
            (&down_scenario, &curve as &dyn PdTermStructure),
        ];

        let result =
            compute_ecl_weighted_from_curves(&exposure, Stage::Stage1, &pd_sources, &config)
                .unwrap();

        // Verify manual calculation
        let measured = |index: usize| match &result.scenario_breakdown[index].measurement {
            EclScenarioMeasurement::Measured { result, .. } => result.ecl,
            other => panic!("expected a measured scenario, got {other:?}"),
        };
        let base_ecl = measured(0);
        let down_ecl = measured(1);
        let expected = 0.6 * base_ecl + 0.4 * down_ecl;

        assert!(
            (result.ecl.unwrap() - expected).abs() < 1e-6,
            "Weighted ECL mismatch: {} vs expected {}",
            result.ecl.unwrap(),
            expected
        );

        // Downside ECL should be higher due to higher LGD
        assert!(down_ecl > base_ecl);
    }

    #[test]
    fn test_compute_ecl_weighted_rejects_empty_pd_sources() {
        let exposure = make_exposure();
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> = Vec::new();
        let result = compute_ecl_weighted_from_curves(
            &exposure,
            Stage::Stage1,
            &pd_sources,
            &EclConfig::default(),
        );

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
        let ecl = compute_ecl_from_curve(
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
    fn scenario_source_controls_the_weighted_allowance() {
        let curve = one_year_2pct_curve();
        let exposure = lgd_test_exposure();
        let scenario = MacroScenario {
            id: "downside".into(),
            weight: finstack_quant_core::wire::DecimalWire("1.0".parse().unwrap()),
            lgd_override: Some(0.90),
        };
        let result = compute_ecl_weighted_from_curves(
            &exposure,
            Stage::Stage1,
            &[(&scenario, &curve)],
            &EclConfig::default(),
        )
        .unwrap();
        let expected = exposure.ead * 0.02 * 0.90;
        assert!((result.ecl.unwrap() - expected).abs() < 1e-9);
        assert_eq!(result.scenario_breakdown[0].id, "downside");
    }

    #[test]
    fn unchanged_rating_uses_origination_risk_snapshot_in_engine() {
        let current = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.04), (5.0, 0.20)]).unwrap();
        let origination =
            RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.002), (5.0, 0.01)]).unwrap();
        let exposure = Exposure {
            ead: 100.0,
            eir: 0.0,
            lgd: 0.40,
            remaining_maturity_years: 5.0,
            ..make_exposure()
        };
        let result = stage_and_measure(&exposure, &current, &origination, 0.0);
        assert_eq!(result.stage_result.stage, Stage::Stage2);
        assert!((result.ecl_result.ecl.unwrap() - 8.0).abs() < 1e-12);

        // A second origination cohort with the same rating but unchanged risk
        // must retain its own initial-recognition snapshot on the same engine.
        let higher_origination_risk = &current;
        let unchanged = stage_and_measure(&exposure, &current, higher_origination_risk, 0.0);
        assert_eq!(unchanged.stage_result.stage, Stage::Stage1);
        assert!((unchanged.ecl_result.ecl.unwrap() - 1.6).abs() < 1e-12);
    }

    #[test]
    fn aged_exposure_compares_the_conditional_origination_window() {
        let current = RawPdCurve::new("BBB", vec![(0.0, 0.0), (1.0, 0.025), (2.0, 0.05)]).unwrap();
        let origination = RawPdCurve::new(
            "BBB",
            vec![(0.0, 0.0), (2.0, 0.10), (3.0, 0.14), (5.0, 0.16)],
        )
        .unwrap();
        let exposure = Exposure {
            ead: 100.0,
            eir: 0.0,
            lgd: 0.40,
            remaining_maturity_years: 2.0,
            ..make_exposure()
        };
        // Original remaining-window risk is (16% - 14%) / (1 - 14%),
        // not the original first-two-year PD of 10%.
        let result = stage_and_measure(&exposure, &current, &origination, 3.0);
        assert_eq!(result.stage_result.stage, Stage::Stage2);
        assert!((result.ecl_result.ecl.unwrap() - 2.0).abs() < 1e-12);
        let expected_delta = 0.05 - (0.16 - 0.14) / (1.0 - 0.14);
        assert!(result.stage_result.triggers.iter().any(|trigger| matches!(
            trigger,
            super::super::staging::StagingTrigger::PdDeltaAbsolute { delta, .. }
                if (*delta - expected_delta).abs() < 1e-12
        )));
    }
}
