//! CECL (ASC 326) variant of expected credit loss computation.
//!
//! CECL (Current Expected Credit Losses) under US GAAP ASC 326 uses a
//! different model from IFRS 9:
//!
//! - **Always lifetime**: No staging; all exposures use remaining maturity
//! - **Reasonable-and-supportable (R&S) forecast**: Forward-looking PD applies
//!   only within the R&S horizon, then reverts to historical loss rates
//! - **Reversion methods**: Immediate or linear blending from forecast to
//!   historical
//!
//! # Scope: no collective (pooled) assessment
//!
//! ASC 326-20-30-2 requires measuring expected credit losses on a
//! collective (pool) basis when financial assets share similar risk
//! characteristics. This module prices each [`Exposure`] individually;
//! grouping exposures into pools with shared parameters (PD/LGD segments,
//! pooled loss rates) is the **caller's responsibility** — e.g. by mapping
//! each pool to a representative exposure or by aggregating per-exposure
//! results along `Exposure::segments`.
//!
//! # References
//!
//! - ASC 326-20 -- Financial Instruments: Credit Losses `docs/REFERENCES.md#asc-326-cecl`
//! - FASB Staff Q&A 2019 -- Reasonable and Supportable Forecast Periods

use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::engine::MacroScenario;
use super::types::{Exposure, PdTermStructure};

/// How the PD curve reverts from forecast to historical after the R&S period.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReversionMethod {
    /// Immediate: PD jumps to historical at the R&S boundary.
    Immediate,
    /// Linear: PD linearly interpolates from forecast to historical over
    /// a specified reversion period.
    Linear {
        /// Reversion period in years (e.g., 1.0 = 1-year linear fade).
        reversion_years: f64,
    },
}

/// CECL calculation methodology.
///
/// [`CeclMethodology::PdLgdEad`] is the discounted PD-LGD-EAD integration;
/// [`CeclMethodology::Warm`] is the undiscounted weighted-average remaining
/// maturity loss-rate method. (A `Vintage` variant existed through 0.6.x but
/// was removed before it was ever implemented — no cohort data model exists
/// to support it honestly.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeclMethodology {
    /// PD-LGD-EAD approach (same formula as IFRS 9, always lifetime).
    PdLgdEad,
    /// Weighted-Average Remaining Maturity (WARM) loss-rate method.
    ///
    /// `ECL = warm_annual_loss_rate × Σ EAD(t_mid) × Δt` — the average annual
    /// charge-off rate applied to the amortization-adjusted exposure-years
    /// over the remaining contractual life (`Exposure::ead_at` over
    /// `remaining_maturity_years`). Undiscounted; PD term structures,
    /// scenarios, LGD, and EIR are not used. Credit-impaired exposures
    /// requiring individual assessment should use
    /// [`CeclMethodology::PdLgdEad`].
    ///
    /// # References
    ///
    /// - ASC 326-20-30-3 (methodology flexibility); FASB Staff Q&A (2019),
    ///   "Whether the WARM Method Is Acceptable" — WARM is an acceptable
    ///   CECL loss-rate method for less complex pools. `docs/REFERENCES.md#asc-326-cecl`
    /// - Fed/FASB/SEC interagency webinar (April 2019): "CECL:
    ///   Weighted-Average Remaining Maturity (WARM) Method." `docs/REFERENCES.md#asc-326-cecl`
    Warm,
}

/// Configuration for CECL (US GAAP ASC 326) calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CeclConfig {
    /// Time bucket width in years (same as IFRS 9). Default: 0.25.
    /// Must be finite and at least 0.0001 years (at most one million buckets
    /// for the maximum supported 100-year maturity).
    pub bucket_width_years: f64,

    /// Reasonable and supportable (R&S) forecast period in years.
    /// Beyond this horizon, PD reverts to historical average.
    /// Typical range: 1-3 years.
    pub forecast_horizon_years: f64,

    /// Reversion method: how PD transitions from R&S to historical.
    pub reversion_method: ReversionMethod,

    /// Historical long-run annual PD used after the R&S period.
    pub historical_annual_pd: f64,

    /// DPD threshold at or above which an exposure is treated as
    /// credit-impaired under CECL. Default: 90.
    #[serde(default = "default_impaired_dpd_threshold")]
    pub impaired_dpd_threshold: u32,

    /// Whether objective qualitative default evidence triggers the
    /// credit-impaired CECL shortcut. Default: true.
    #[serde(default = "default_impaired_qualitative_triggers_enabled")]
    pub impaired_qualitative_triggers_enabled: bool,

    /// Expected time to recovery / resolution for credit-impaired exposures,
    /// in years. Impaired allowance is current EAD less expected recovery
    /// `(1 - LGD) * EAD`, discounted to this horizon when discounting is enabled.
    /// Default: 1.0.
    #[serde(default = "default_impaired_time_to_recovery_years")]
    pub impaired_time_to_recovery_years: f64,

    /// Whether expected losses are discounted at the exposure EIR. Default:
    /// true for continuity with the existing PD-LGD-EAD implementation.
    #[serde(default = "default_discount_expected_losses")]
    pub discount_expected_losses: bool,

    /// Macro scenario specifications (same structure as IFRS 9).
    pub scenarios: Vec<MacroScenario>,

    /// CECL methodology selection.
    pub methodology: CeclMethodology,

    /// Average annual net charge-off rate (decimal) for the WARM method.
    /// Required when `methodology == CeclMethodology::Warm`; rejected otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warm_annual_loss_rate: Option<f64>,
}

fn default_impaired_dpd_threshold() -> u32 {
    90
}

fn default_impaired_qualitative_triggers_enabled() -> bool {
    true
}

fn default_impaired_time_to_recovery_years() -> f64 {
    1.0
}

fn default_discount_expected_losses() -> bool {
    true
}

impl CeclConfig {
    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for an invalid bucket width, forecast
    /// horizon, historical annual PD, impaired recovery horizon, scenario
    /// weights, or linear-reversion horizon.
    pub fn validate(&self) -> Result<()> {
        if !self.bucket_width_years.is_finite() || self.bucket_width_years < 1e-4 {
            return Err(Error::Validation(
                "bucket_width_years must be finite and at least 0.0001 years".to_string(),
            ));
        }
        if !self.forecast_horizon_years.is_finite() || self.forecast_horizon_years < 0.0 {
            return Err(Error::Validation(
                "forecast_horizon_years must be non-negative".to_string(),
            ));
        }
        if !self.historical_annual_pd.is_finite()
            || self.historical_annual_pd < 0.0
            || self.historical_annual_pd > 1.0
        {
            return Err(Error::Validation(
                "historical_annual_pd must be a finite value in [0, 1]".to_string(),
            ));
        }
        if !self.impaired_time_to_recovery_years.is_finite()
            || self.impaired_time_to_recovery_years < 0.0
        {
            return Err(Error::Validation(
                "impaired_time_to_recovery_years must be finite and non-negative".to_string(),
            ));
        }
        super::engine::validate_scenario_weights(&self.scenarios)?;
        if let ReversionMethod::Linear { reversion_years } = self.reversion_method {
            if !reversion_years.is_finite() || reversion_years <= 0.0 {
                return Err(Error::Validation(
                    "Linear reversion_years must be positive".to_string(),
                ));
            }
        }
        match self.methodology {
            CeclMethodology::Warm => {
                let rate = self.warm_annual_loss_rate.ok_or_else(|| {
                    Error::Validation(
                        "CECL WARM methodology requires warm_annual_loss_rate".to_string(),
                    )
                })?;
                if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
                    return Err(Error::Validation(format!(
                        "warm_annual_loss_rate must be a finite value in [0, 1], got {rate}"
                    )));
                }
            }
            CeclMethodology::PdLgdEad => {
                if self.warm_annual_loss_rate.is_some() {
                    return Err(Error::Validation(
                        "warm_annual_loss_rate is only valid with the Warm methodology; \
                         remove it or select CeclMethodology::Warm"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// CECL result for a single exposure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CeclResult {
    /// Exposure identifier.
    pub exposure_id: String,
    /// Total lifetime ECL.
    pub ecl: f64,
    /// ECL horizon in years (always remaining maturity).
    pub horizon: f64,
    /// Methodology used.
    pub methodology: CeclMethodology,
}

/// CECL engine computing lifetime ECL with R&S forecast and historical reversion.
pub struct CeclEngine<'a> {
    config: CeclConfig,
    pd_sources: Vec<(&'a MacroScenario, &'a dyn PdTermStructure)>,
}

impl<'a> CeclEngine<'a> {
    /// Create a new CECL engine.
    ///
    /// `pd_sources` requirements apply only to [`CeclMethodology::PdLgdEad`];
    /// the [`CeclMethodology::Warm`] path ignores PD sources entirely.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the configuration is invalid, or,
    /// for [`CeclMethodology::PdLgdEad`], when `pd_sources` is empty or its
    /// scenario weights are not finite, non-negative, and summing to 1.0.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration object controlling validation, rounding, or solver behavior
    /// * `pd_sources` - Probability-of-default sources keyed to the configured scenarios.
    pub fn new(
        config: CeclConfig,
        pd_sources: Vec<(&'a MacroScenario, &'a dyn PdTermStructure)>,
    ) -> Result<Self> {
        config.validate()?;
        if config.methodology == CeclMethodology::PdLgdEad {
            if pd_sources.is_empty() {
                return Err(Error::Validation(
                    "At least one PD source is required for CeclEngine".to_string(),
                ));
            }
            super::engine::validate_scenario_weights(
                pd_sources.iter().map(|(scenario, _)| *scenario),
            )?;
        }
        Ok(Self { config, pd_sources })
    }

    /// Compute CECL for a single exposure.
    ///
    /// Always uses the full remaining maturity (no staging).
    /// PD term structure is blended: forecast PD for the R&S period,
    /// then reverts to historical.
    ///
    /// The bucket integration uses the unconditional bucket default
    /// probability `S(t_start) * P(default in [t_start, t_end] | survive to t_start)`,
    /// with the running survival `S` carried across the forecast → historical
    /// boundary so the reverted portion remains properly conditional on
    /// surviving the R&S window.
    ///
    /// # Errors
    ///
    /// Propagates exposure validation and PD-term-structure lookup errors for
    /// the exposure rating. Errors from the configured PD sources are surfaced
    /// before calculating lifetime loss.
    pub fn compute_cecl(&self, exposure: &Exposure) -> Result<CeclResult> {
        exposure.validate()?;
        if self.config.methodology == CeclMethodology::Warm {
            return self.compute_warm(exposure);
        }
        let horizon = exposure.remaining_maturity_years;
        let rating = exposure.current_rating.as_deref().unwrap_or("NR");
        let dt = self.config.bucket_width_years;
        let n_buckets = (horizon / dt).ceil() as usize;
        let n_buckets = n_buckets.max(1);

        for (_, pd_source) in &self.pd_sources {
            if !(0.0..=1.0).contains(&pd_source.cumulative_pd(rating, 0.0)?) {
                return Err(Error::Validation(
                    "PD source must return finite probabilities in [0, 1]".into(),
                ));
            }
        }

        if self.is_credit_impaired(exposure) {
            let t_recovery = self.config.impaired_time_to_recovery_years;
            let mut weighted_ecl = 0.0;
            for (scenario, _) in &self.pd_sources {
                let lgd = scenario.lgd_override.unwrap_or(exposure.lgd);
                let df = self.discount_factor(exposure, t_recovery);
                let ead = exposure.ead_at(0.0)?;
                weighted_ecl += scenario.weight * (ead - (1.0 - lgd) * ead * df);
            }
            super::engine::validate_finite_ecl(weighted_ecl)?;
            return Ok(CeclResult {
                exposure_id: exposure.id.clone(),
                ecl: weighted_ecl,
                horizon: t_recovery,
                methodology: self.config.methodology,
            });
        }

        let mut weighted_ecl = 0.0;

        for (scenario, pd_source) in &self.pd_sources {
            let mut scenario_ecl = 0.0;
            let mut survival = 1.0_f64;
            for i in 0..n_buckets {
                let t_start = i as f64 * dt;
                let t_end = ((i + 1) as f64 * dt).min(horizon);
                let t_mid = (t_start + t_end) / 2.0;

                // Conditional probability of default within the bucket
                // given survival to t_start. Forecast region uses the
                // pd_source's conditional marginal; reverted region uses
                // a hazard-based constant intensity derived from the
                // historical annual PD; the straddling bucket composes
                // both sub-intervals multiplicatively so that no
                // survival weight is lost at the R&S boundary.
                let cond_mpd = self.blended_conditional_mpd(*pd_source, rating, t_start, t_end)?;

                if !(0.0..=1.0).contains(&cond_mpd) {
                    return Err(Error::Validation(
                        "Conditional PD must be finite and in [0, 1]".into(),
                    ));
                }

                // Unconditional bucket default probability: S(t_start) * cond_mpd.
                let uncond_mpd = (survival * cond_mpd).max(0.0);
                survival = (survival * (1.0 - cond_mpd)).max(0.0);

                let lgd = scenario.lgd_override.unwrap_or(exposure.lgd);
                let df = self.discount_factor(exposure, t_mid);
                scenario_ecl += uncond_mpd * lgd * exposure.ead_at(t_mid)? * df;
            }
            weighted_ecl += scenario.weight * scenario_ecl;
        }

        super::engine::validate_finite_ecl(weighted_ecl)?;
        Ok(CeclResult {
            exposure_id: exposure.id.clone(),
            ecl: weighted_ecl,
            horizon,
            methodology: self.config.methodology,
        })
    }

    /// WARM loss-rate ECL: `rate × Σ EAD(t_mid) × Δt` over the remaining life.
    ///
    /// Undiscounted by design (loss-rate method); see
    /// [`CeclMethodology::Warm`] for methodology references.
    fn compute_warm(&self, exposure: &Exposure) -> Result<CeclResult> {
        let rate = self.config.warm_annual_loss_rate.ok_or_else(|| {
            // Unreachable after validate(), but fail closed for direct
            // struct-literal configs.
            Error::Validation("CECL WARM methodology requires warm_annual_loss_rate".to_string())
        })?;
        let horizon = exposure.remaining_maturity_years;
        let dt = self.config.bucket_width_years;
        let n_buckets = ((horizon / dt).ceil() as usize).max(1);

        let mut exposure_years = 0.0_f64;
        for i in 0..n_buckets {
            let t_start = i as f64 * dt;
            let t_end = ((i + 1) as f64 * dt).min(horizon);
            let t_mid = (t_start + t_end) / 2.0;
            exposure_years += exposure.ead_at(t_mid)? * (t_end - t_start);
        }

        super::engine::validate_finite_ecl(rate * exposure_years)?;
        Ok(CeclResult {
            exposure_id: exposure.id.clone(),
            ecl: rate * exposure_years,
            horizon,
            methodology: CeclMethodology::Warm,
        })
    }

    fn is_credit_impaired(&self, exposure: &Exposure) -> bool {
        exposure.days_past_due >= self.config.impaired_dpd_threshold
            || (self.config.impaired_qualitative_triggers_enabled
                && exposure.qualitative_flags.has_default_evidence())
    }

    fn discount_factor(&self, exposure: &Exposure, t: f64) -> f64 {
        if self.config.discount_expected_losses {
            1.0 / (1.0 + exposure.eir).powf(t)
        } else {
            1.0
        }
    }

    /// Blended conditional marginal PD for `[t1, t2]` given survival to `t1`.
    ///
    /// - Fully within R&S period: use forecast conditional PD from `pd_source`.
    /// - Fully beyond R&S: use hazard-based historical conditional PD.
    /// - Straddling boundary: compose the forecast `[t1, rs]` and the
    ///   reverted `[rs, t2]` segments multiplicatively:
    ///   `1 - (1 - mpd_forecast) * (1 - mpd_reverted)`.
    ///   This preserves survival consistency across the boundary; a
    ///   weighted arithmetic blend would systematically under- or
    ///   over-weight the post-boundary hazard.
    fn blended_conditional_mpd(
        &self,
        pd_source: &dyn PdTermStructure,
        rating: &str,
        t1: f64,
        t2: f64,
    ) -> Result<f64> {
        let rs = self.config.forecast_horizon_years;

        if t2 <= rs {
            return pd_source.marginal_pd(rating, t1, t2);
        }

        if t1 >= rs {
            return self.reverted_conditional_mpd(pd_source, rating, t1, t2);
        }

        // Straddles the boundary: compose forecast(t1,rs) and
        // reverted(rs,t2) multiplicatively.
        let mpd_forecast = pd_source.marginal_pd(rating, t1, rs)?;
        let mpd_reverted = self.reverted_conditional_mpd(pd_source, rating, rs, t2)?;
        Ok((1.0 - (1.0 - mpd_forecast) * (1.0 - mpd_reverted)).clamp(0.0, 1.0))
    }

    /// Reverted conditional marginal PD for a bucket fully beyond the R&S
    /// period (or the post-boundary sub-interval of a straddling bucket).
    ///
    /// Uses a constant-intensity (hazard rate) translation of the historical
    /// annual PD: `lambda = -ln(1 - annual_pd)` and
    /// `cond_mpd = 1 - exp(-lambda * dt)`. For linear reversion the hazard
    /// is a convex combination of the local forecast hazard (approximated
    /// from `pd_source.marginal_pd` over the bucket) and the historical
    /// hazard, blended linearly from 0 to 1 over the reversion window.
    fn reverted_conditional_mpd(
        &self,
        pd_source: &dyn PdTermStructure,
        rating: &str,
        t1: f64,
        t2: f64,
    ) -> Result<f64> {
        let annual_pd = self.config.historical_annual_pd;
        let dt = t2 - t1;
        // Zero-width bucket: no default can accrue; short-circuit to avoid
        // `+inf / 0` hazard and `exp(-inf * 0) = NaN` downstream.
        if dt <= f64::EPSILON {
            return Ok(0.0);
        }
        let lambda_hist = if annual_pd < 1.0 {
            -(1.0 - annual_pd).ln()
        } else {
            f64::INFINITY
        };
        let historical_mpd = 1.0 - (-lambda_hist * dt).exp();

        match self.config.reversion_method {
            ReversionMethod::Immediate => Ok(historical_mpd),
            ReversionMethod::Linear { reversion_years } => {
                let rs = self.config.forecast_horizon_years;
                let reversion_end = rs + reversion_years;
                let t_mid = (t1 + t2) / 2.0;

                if t_mid >= reversion_end {
                    Ok(historical_mpd)
                } else {
                    // Convert the forecast conditional MPD at the R&S
                    // boundary to a hazard, blend toward the historical
                    // hazard, and convert back. Do not query the forecast
                    // curve over post-R&S buckets; after the supportable
                    // horizon the forecast assumption is held at its boundary
                    // hazard and then reverted.
                    let blend = ((t_mid - rs) / reversion_years).clamp(0.0, 1.0);
                    let lambda_fcst = self.forecast_boundary_hazard(pd_source, rating, dt)?;
                    let lambda_blend = (1.0 - blend) * lambda_fcst + blend * lambda_hist;
                    Ok(1.0 - (-lambda_blend * dt).exp())
                }
            }
        }
    }

    fn forecast_boundary_hazard(
        &self,
        pd_source: &dyn PdTermStructure,
        rating: &str,
        bucket_width: f64,
    ) -> Result<f64> {
        let rs = self.config.forecast_horizon_years;
        let (start, end) = if rs > f64::EPSILON {
            ((rs - bucket_width).max(0.0), rs)
        } else {
            (0.0, bucket_width)
        };
        let dt = end - start;
        if dt <= f64::EPSILON {
            return Ok(0.0);
        }
        let fcst_mpd = pd_source.marginal_pd(rating, start, end)?;
        Ok(if fcst_mpd < 1.0 {
            -(1.0 - fcst_mpd).ln() / dt
        } else {
            f64::INFINITY
        })
    }

    /// Access the engine's configuration.
    pub fn config(&self) -> &CeclConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ecl::types::{QualitativeFlags, RawPdCurve};

    fn make_exposure() -> Exposure {
        Exposure {
            id: "CECL-001".to_string(),
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
    fn test_cecl_always_lifetime() {
        let curve = make_pd_curve();
        let scenario = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];

        let config = CeclConfig::default();
        let engine = CeclEngine::new(config, pd_sources).unwrap();
        let exposure = make_exposure();
        let result = engine.compute_cecl(&exposure).unwrap();

        // CECL always uses remaining maturity
        assert!(
            (result.horizon - 5.0).abs() < 1e-10,
            "CECL horizon should equal remaining maturity"
        );
        assert!(result.ecl > 0.0);
    }

    #[test]
    fn test_cecl_immediate_reversion() {
        let curve = make_pd_curve();
        let scenario = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];

        let config = CeclConfig {
            forecast_horizon_years: 1.0,
            reversion_method: ReversionMethod::Immediate,
            historical_annual_pd: 0.03,
            ..CeclConfig::default()
        };
        let engine = CeclEngine::new(config, pd_sources).unwrap();
        let exposure = make_exposure();
        let result = engine.compute_cecl(&exposure).unwrap();

        assert!(result.ecl > 0.0);
    }

    #[test]
    fn test_cecl_linear_reversion() {
        let curve = make_pd_curve();
        let scenario = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let pd_sources: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&scenario, &curve as &dyn PdTermStructure)];

        let config = CeclConfig {
            forecast_horizon_years: 1.0,
            reversion_method: ReversionMethod::Linear {
                reversion_years: 1.0,
            },
            historical_annual_pd: 0.03,
            ..CeclConfig::default()
        };
        let engine = CeclEngine::new(config, pd_sources).unwrap();
        let exposure = make_exposure();
        let result = engine.compute_cecl(&exposure).unwrap();

        assert!(result.ecl > 0.0);
    }

    #[test]
    fn test_cecl_validation_negative_historical_pd() {
        let config = CeclConfig {
            historical_annual_pd: -0.01,
            ..CeclConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn cecl_config_rejects_nan_historical_pd() {
        // A non-finite PD must be rejected rather than silently propagating a
        // NaN allowance: `x < 0.0 || x > 1.0` is false for NaN, so the bare
        // range check alone would accept it.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let config = CeclConfig {
                historical_annual_pd: bad,
                ..CeclConfig::default()
            };
            assert!(
                config.validate().is_err(),
                "historical_annual_pd = {bad} must fail validation"
            );
        }
    }

    #[test]
    fn test_cecl_validation_invalid_weights() {
        let config = CeclConfig {
            scenarios: vec![
                MacroScenario {
                    id: "a".into(),
                    weight: 0.5,
                    lgd_override: None,
                },
                MacroScenario {
                    id: "b".into(),
                    weight: 0.3,
                    lgd_override: None,
                },
            ],
            ..CeclConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cecl_vs_ifrs9_stage2() {
        // CECL should produce similar results to IFRS 9 Stage 2 (both lifetime)
        // when using same PD curve and no reversion (R&S covers full horizon).
        let curve = make_pd_curve();
        let exposure = make_exposure();

        // CECL with R&S covering full maturity
        let cecl_scenario = MacroScenario {
            id: "base".into(),
            weight: 1.0,
            lgd_override: None,
        };
        let cecl_pd: Vec<(&MacroScenario, &dyn PdTermStructure)> =
            vec![(&cecl_scenario, &curve as &dyn PdTermStructure)];
        let cecl_config = CeclConfig {
            forecast_horizon_years: 10.0, // Covers full 5-year maturity
            reversion_method: ReversionMethod::Immediate,
            historical_annual_pd: 0.02,
            ..CeclConfig::default()
        };
        let cecl_engine = CeclEngine::new(cecl_config, cecl_pd).unwrap();
        let cecl_result = cecl_engine.compute_cecl(&exposure).unwrap();

        // IFRS 9 Stage 2
        let ifrs9_config = super::super::engine::EclConfig::default();
        let ifrs9_result = super::super::engine::compute_ecl(
            &exposure,
            crate::analysis::ecl::types::Stage::Stage2,
            &curve,
            &ifrs9_config,
        )
        .unwrap();

        // Both should be close (same formula, same horizon)
        assert!(
            (cecl_result.ecl - ifrs9_result.ecl).abs() / ifrs9_result.ecl < 0.01,
            "CECL ({}) and IFRS 9 Stage 2 ({}) should be close when R&S covers maturity",
            cecl_result.ecl,
            ifrs9_result.ecl
        );
    }

    #[test]
    fn cecl_methodology_vintage_is_removed_from_the_wire() {
        // Vintage was removed 2026-07-24: no cohort data model exists to support
        // it honestly. Inbound JSON must fail to parse rather than silently map
        // anywhere.
        let parsed: std::result::Result<CeclMethodology, _> = serde_json::from_str("\"Vintage\"");
        assert!(parsed.is_err(), "removed variant must not deserialize");
    }

    #[test]
    fn warm_constant_ead_golden() {
        // WARM = rate × Σ EAD(t_mid) × Δt.
        // Constant EAD 1,000,000 over 5.0y with 0.25y buckets:
        //   Σ EAD × Δt = 1,000,000 × 5.0 = 5,000,000 exposure-years.
        // rate 0.005 → ECL = 0.005 × 5,000,000 = 25,000. Undiscounted (eir ignored).
        let config = CeclConfig {
            methodology: CeclMethodology::Warm,
            warm_annual_loss_rate: Some(0.005),
            ..CeclConfig::default()
        };
        let engine = CeclEngine::new(config, vec![]).unwrap();
        let exposure = make_exposure(); // ead 1_000_000, maturity 5.0, eir 0.05
        let result = engine.compute_cecl(&exposure).unwrap();
        assert!((result.ecl - 25_000.0).abs() < 1e-9, "got {}", result.ecl);
        assert_eq!(result.methodology, CeclMethodology::Warm);
        assert!((result.horizon - 5.0).abs() < 1e-12);
    }

    #[test]
    fn warm_amortizing_ead_golden() {
        // Linear amortization 1,000,000 → 0 over 5y. Midpoint-rule sum is exact
        // for a linear profile: ∫₀⁵ EAD(t) dt = 1,000,000 × 5 / 2 = 2,500,000.
        // rate 0.005 → ECL = 12,500.
        let config = CeclConfig {
            methodology: CeclMethodology::Warm,
            warm_annual_loss_rate: Some(0.005),
            ..CeclConfig::default()
        };
        let engine = CeclEngine::new(config, vec![]).unwrap();
        let mut exposure = make_exposure();
        exposure.ead_schedule = Some(vec![(0.0, 1_000_000.0), (5.0, 0.0)]);
        let result = engine.compute_cecl(&exposure).unwrap();
        assert!((result.ecl - 12_500.0).abs() < 1e-9, "got {}", result.ecl);
    }

    #[test]
    fn warm_requires_loss_rate_and_rejects_it_under_pdlgdead() {
        // Warm without a rate must fail closed.
        let missing = CeclConfig {
            methodology: CeclMethodology::Warm,
            warm_annual_loss_rate: None,
            ..CeclConfig::default()
        };
        assert!(missing.validate().is_err());

        // Out-of-range rate rejected.
        for bad in [-0.01, 1.5, f64::NAN, f64::INFINITY] {
            let config = CeclConfig {
                methodology: CeclMethodology::Warm,
                warm_annual_loss_rate: Some(bad),
                ..CeclConfig::default()
            };
            assert!(
                config.validate().is_err(),
                "rate {bad} must fail validation"
            );
        }

        // Symmetric inert-config guard: a WARM knob under PdLgdEad is exactly the
        // silently-ignored-config pattern this change removes. Reject it.
        let inert = CeclConfig {
            methodology: CeclMethodology::PdLgdEad,
            warm_annual_loss_rate: Some(0.005),
            ..CeclConfig::default()
        };
        assert!(inert.validate().is_err());
    }

    #[test]
    fn warm_config_serde_is_additive() {
        // Pre-change CeclConfig JSON (no warm_annual_loss_rate) must still parse.
        let json = serde_json::to_string(&CeclConfig::default()).unwrap();
        assert!(
            !json.contains("warm_annual_loss_rate"),
            "None must be skipped on the wire"
        );
        let parsed: CeclConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.warm_annual_loss_rate.is_none());
    }
}
