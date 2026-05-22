//! Core types for the XVA (Valuation Adjustments) framework.
//!
//! Defines configuration, result containers, netting set specifications,
//! and CSA (Credit Support Annex) terms used across all XVA calculations.

use crate::registry::embedded_registry_or_panic;

/// Funding cost/benefit configuration for FVA calculation.
///
/// Models the asymmetric funding costs that arise from uncollateralized
/// derivative positions. Positive exposure requires funding (cost),
/// while negative exposure provides funding (benefit).
///
/// # References
///
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
/// - Green XVA: `docs/REFERENCES.md#green-xva`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct FundingConfig {
    /// Funding spread in basis points (cost on positive exposure).
    ///
    /// This is the spread over the risk-free rate that the institution
    /// pays to fund positive (out-of-the-money to counterparty) exposure.
    /// Typical values: 20–100 bps depending on the institution's credit quality.
    pub funding_spread_bps: f64,

    /// Funding benefit spread in basis points (benefit on negative exposure).
    ///
    /// If `None`, symmetric funding is assumed: `funding_benefit = funding_spread`.
    /// In practice, the benefit spread may be lower than the cost spread
    /// due to asymmetric funding conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_benefit_bps: Option<f64>,
}

impl FundingConfig {
    /// Returns the effective funding benefit spread in basis points.
    ///
    /// If `funding_benefit_bps` is `None`, returns `funding_spread_bps`
    /// (symmetric funding assumption).
    ///
    /// # Returns
    ///
    /// The benefit spread in basis points.
    pub fn effective_benefit_bps(&self) -> f64 {
        self.funding_benefit_bps.unwrap_or(self.funding_spread_bps)
    }
}

/// XVA calculation configuration.
///
/// Controls the time grid, recovery assumptions, and optional modeling
/// features for exposure simulation and CVA computation.
///
/// # Defaults
///
/// The default configuration provides a quarterly time grid out to 30 years
/// with a 40% recovery rate (ISDA standard for senior unsecured).
///
/// # References
///
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct XvaConfig {
    /// Time grid for exposure simulation (years from today).
    ///
    /// Determines the granularity of exposure profiles. Finer grids
    /// improve accuracy but increase computation cost.
    pub time_grid: Vec<f64>,

    /// Recovery rate for counterparty default (typically 0.40).
    ///
    /// Market standard for senior unsecured is 40%, per ISDA conventions
    /// and CDS pricing practices.
    pub recovery_rate: f64,

    /// Recovery rate for own default (used in DVA calculation).
    ///
    /// If `None`, defaults to the counterparty `recovery_rate`.
    /// May differ from counterparty recovery if the institution's
    /// seniority or credit quality warrants a different assumption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_recovery_rate: Option<f64>,

    /// Funding configuration for FVA calculation.
    ///
    /// If `None`, FVA is not computed. When provided, funding costs
    /// and benefits are calculated based on the exposure profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding: Option<FundingConfig>,
}

impl Default for XvaConfig {
    /// Creates the registry-backed default XVA configuration.
    fn default() -> Self {
        let defaults = &embedded_registry_or_panic().xva.deterministic_exposure;
        let time_grid: Vec<f64> = (1..=defaults.time_grid_points)
            .map(|i| i as f64 * defaults.time_grid_step_years)
            .collect();
        Self {
            time_grid,
            recovery_rate: defaults.recovery_rate,
            own_recovery_rate: defaults.own_recovery_rate,
            funding: None,
        }
    }
}

impl XvaConfig {
    /// Validate configuration parameters.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the configuration is internally consistent.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Time grid is empty
    /// - Time grid contains non-positive or non-finite values
    /// - Time grid is not strictly increasing
    /// - Recovery rate is not in `[0, 1]`
    pub fn validate(&self) -> finstack_core::Result<()> {
        if self.time_grid.is_empty() {
            return Err(finstack_core::Error::Validation(
                "XvaConfig: time_grid must not be empty".into(),
            ));
        }

        for (i, &t) in self.time_grid.iter().enumerate() {
            if !t.is_finite() || t <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "XvaConfig: time_grid[{i}] = {t} must be positive and finite"
                )));
            }
            if i > 0 && t <= self.time_grid[i - 1] {
                return Err(finstack_core::Error::Validation(format!(
                    "XvaConfig: time_grid must be strictly increasing at index {i}"
                )));
            }
        }

        if !(0.0..=1.0).contains(&self.recovery_rate) {
            return Err(finstack_core::Error::Validation(format!(
                "XvaConfig: recovery_rate {} must be in [0, 1]",
                self.recovery_rate
            )));
        }

        if let Some(own_r) = self.own_recovery_rate {
            if !(0.0..=1.0).contains(&own_r) {
                return Err(finstack_core::Error::Validation(format!(
                    "XvaConfig: own_recovery_rate {own_r} must be in [0, 1]"
                )));
            }
        }

        if let Some(ref funding) = self.funding {
            if !funding.funding_spread_bps.is_finite() || funding.funding_spread_bps < 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "XvaConfig: funding_spread_bps {} must be non-negative and finite",
                    funding.funding_spread_bps
                )));
            }
            if let Some(benefit) = funding.funding_benefit_bps {
                if !benefit.is_finite() || benefit < 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "XvaConfig: funding_benefit_bps {benefit} must be non-negative and finite"
                    )));
                }
                if benefit > funding.funding_spread_bps {
                    return Err(finstack_core::Error::Validation(format!(
                        "XvaConfig: funding_benefit_bps {benefit} must not exceed funding_spread_bps {}",
                        funding.funding_spread_bps
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Result of XVA calculations.
///
/// Contains the CVA value along with the full exposure profile and
/// regulatory metrics used for reporting and risk management.
///
/// # Exposure Profiles
///
/// Each profile entry is a `(time, value)` pair where time is in years
/// from the valuation date and value is in the portfolio's base currency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct XvaResult {
    /// Unilateral CVA (positive = cost to the desk).
    ///
    /// Represents the expected loss due to counterparty default,
    /// discounted to present value.
    pub cva: f64,

    /// DVA (Debit Valuation Adjustment): own-default benefit.
    ///
    /// Positive DVA represents the expected gain to the desk from
    /// the institution's own default on negative-exposure positions.
    ///
    /// `None` when DVA is not computed (unilateral CVA only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dva: Option<f64>,

    /// FVA (Funding Valuation Adjustment): net funding cost/benefit.
    ///
    /// Positive FVA represents a net funding cost; negative FVA
    /// represents a net funding benefit. Captures the cost of
    /// funding uncollateralized derivative positions.
    ///
    /// `None` when FVA is not computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fva: Option<f64>,

    /// Bilateral CVA: CVA - DVA.
    ///
    /// The net credit adjustment accounting for both counterparty
    /// default risk (CVA) and own-default benefit (DVA).
    ///
    /// `None` when bilateral CVA is not computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bilateral_cva: Option<f64>,

    /// Expected Positive Exposure profile: `(time, EPE(t))`.
    ///
    /// EPE(t) = E[max(V(t), 0)] — the average positive mark-to-market
    /// at each time point.
    pub epe_profile: Vec<(f64, f64)>,

    /// Expected Negative Exposure profile: `(time, ENE(t))`.
    ///
    /// ENE(t) = E[max(-V(t), 0)] — the average negative mark-to-market
    /// at each time point (own-default exposure).
    pub ene_profile: Vec<(f64, f64)>,

    /// Potential Future Exposure profile: `(time, PFE(t))`.
    ///
    /// **IMPORTANT** — the deterministic CVA engine has a single path,
    /// so the distribution of exposures collapses to a point mass at
    /// `max(V(t), 0)`. In that degenerate case every quantile (and the
    /// mean) equals `EPE(t)`, and this field holds the EPE path, not a
    /// tail quantile. The name is retained so downstream systems keep
    /// their column bindings; use the Monte Carlo engine
    /// ([`StochasticExposureProfile`]) when a true 97.5%-quantile PFE
    /// is required for limit monitoring.
    pub pfe_profile: Vec<(f64, f64)>,

    /// Maximum PFE across the profile (`max_t PFE(t)`).
    ///
    /// In the deterministic engine this equals `max_t EPE(t)` by
    /// construction (see [`Self::pfe_profile`]). Used for coarse credit
    /// limit monitoring where a Monte Carlo tail quantile is not
    /// available.
    pub max_pfe: f64,

    /// Effective EPE profile: `(time, Effective_EPE(t))`.
    ///
    /// Non-decreasing version of EPE, per Basel III SA-CCR:
    /// `Effective_EPE(t_k) = max(Effective_EPE(t_{k-1}), EPE(t_k))`
    ///
    /// # References
    ///
    /// - BCBS 279 (2014). "The standardised approach for measuring
    ///   counterparty credit risk exposures."
    pub effective_epe_profile: Vec<(f64, f64)>,

    /// Time-weighted average of Effective EPE (regulatory scalar metric).
    ///
    /// Computed as:
    /// ```text
    /// Effective_EPE_avg = (1 / min(1, M)) × Σₖ Effective_EPE(tₖ) × Δtₖ
    /// ```
    ///
    /// where `M` is the portfolio maturity and `Δtₖ = tₖ - tₖ₋₁`.
    /// This is the key input for EAD under SA-CCR.
    ///
    /// # References
    ///
    /// - BCBS 279 (2014). "The standardised approach for measuring
    ///   counterparty credit risk exposures."
    pub effective_epe: f64,
}

/// Diagnostics from exposure simulation capturing data quality metrics.
///
/// Populated by the exposure computation engine to let callers distinguish
/// genuine zero exposure from missing data.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExposureDiagnostics {
    /// Number of time grid points where market data could not be rolled forward.
    pub market_roll_failures: usize,

    /// Total number of individual instrument valuation failures across all time points.
    pub valuation_failures: usize,

    /// Total time grid points evaluated.
    pub total_time_points: usize,
}

/// Exposure profile computed at each time grid point.
///
/// This is the intermediate result from exposure simulation,
/// consumed by the CVA calculator.
///
/// All vectors are expressed in the netting set's reporting currency when one
/// is configured; otherwise they use the natural single-currency portfolio
/// currency inferred by the exposure engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExposureProfile {
    /// Time points in years from valuation date.
    pub times: Vec<f64>,

    /// Portfolio mark-to-market value at each time point (may be negative).
    pub mtm_values: Vec<f64>,

    /// Expected Positive Exposure at each time point: max(V(t), 0).
    pub epe: Vec<f64>,

    /// Expected Negative Exposure at each time point: max(-V(t), 0).
    pub ene: Vec<f64>,

    /// Simulation quality diagnostics (populated by the exposure engine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<ExposureDiagnostics>,
}

impl ExposureProfile {
    /// Validate that the exposure profile is internally consistent.
    ///
    /// # Returns
    ///
    /// `Ok(())` when vector lengths, time ordering, and numeric finiteness are valid.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any vector lengths are inconsistent
    /// - Times are not strictly increasing
    /// - EPE or ENE contain negative or non-finite values
    /// - MtM values are non-finite
    pub fn validate(&self) -> finstack_core::Result<()> {
        let n = self.times.len();

        if self.mtm_values.len() != n || self.epe.len() != n || self.ene.len() != n {
            return Err(finstack_core::Error::Validation(format!(
                "ExposureProfile: vector lengths must be equal \
                 (times={}, mtm={}, epe={}, ene={})",
                n,
                self.mtm_values.len(),
                self.epe.len(),
                self.ene.len()
            )));
        }

        for (i, &t) in self.times.iter().enumerate() {
            if !t.is_finite() || t <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "ExposureProfile: times[{i}] = {t} must be positive and finite"
                )));
            }
            if i > 0 && t <= self.times[i - 1] {
                return Err(finstack_core::Error::Validation(format!(
                    "ExposureProfile: times must be strictly increasing at index {i}"
                )));
            }
        }

        for (i, (&epe_v, &ene_v)) in self.epe.iter().zip(self.ene.iter()).enumerate() {
            if !epe_v.is_finite() || epe_v < 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "ExposureProfile: epe[{i}] = {epe_v} must be non-negative and finite"
                )));
            }
            if !ene_v.is_finite() || ene_v < 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "ExposureProfile: ene[{i}] = {ene_v} must be non-negative and finite"
                )));
            }
        }

        for (i, &mtm) in self.mtm_values.iter().enumerate() {
            if !mtm.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "ExposureProfile: mtm_values[{i}] = {mtm} must be finite"
                )));
            }
        }

        Ok(())
    }
}

/// Configuration for stochastic exposure simulation.
///
/// Used by the Monte Carlo-based XVA exposure engine to control simulation
/// size, reproducibility, and the PFE confidence level.
///
/// `pfe_quantile` is a decimal probability, so `0.975` means the 97.5th
/// percentile of positive exposure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StochasticExposureConfig {
    /// Number of Monte Carlo paths to simulate.
    pub num_paths: usize,

    /// Deterministic RNG seed for reproducible exposure profiles.
    pub seed: u64,

    /// Quantile used for Potential Future Exposure.
    ///
    /// Market practice is typically 95% to 99%; the XVA module defaults to 97.5%.
    pub pfe_quantile: f64,
}

impl Default for StochasticExposureConfig {
    /// Create the default stochastic exposure configuration.
    fn default() -> Self {
        let defaults = &embedded_registry_or_panic().xva.stochastic_exposure;
        Self {
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            pfe_quantile: defaults.pfe_quantile,
        }
    }
}

impl StochasticExposureConfig {
    /// Validate stochastic exposure simulation parameters.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the Monte Carlo path count and quantile are usable.
    ///
    /// # Errors
    ///
    /// Returns an error if `num_paths` is zero or `pfe_quantile` is outside `(0, 1)`.
    pub fn validate(&self) -> finstack_core::Result<()> {
        if self.num_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "StochasticExposureConfig: num_paths must be positive".into(),
            ));
        }

        if !self.pfe_quantile.is_finite() || self.pfe_quantile <= 0.0 || self.pfe_quantile >= 1.0 {
            return Err(finstack_core::Error::Validation(format!(
                "StochasticExposureConfig: pfe_quantile {} must be in (0, 1)",
                self.pfe_quantile
            )));
        }

        Ok(())
    }
}

/// Stochastic exposure profile with distribution-based PFE.
///
/// The embedded [`ExposureProfile`] contains the pathwise averages needed for CVA/DVA/FVA
/// integration, while `pfe_profile` preserves the chosen tail quantile of the simulated
/// positive exposure distribution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StochasticExposureProfile {
    /// Average MtM/EPE/ENE profile used by the XVA calculators.
    pub profile: ExposureProfile,

    /// Potential future exposure profile at the configured quantile.
    pub pfe_profile: Vec<f64>,

    /// Number of Monte Carlo paths used to estimate the profile.
    pub path_count: usize,

    /// Tail quantile used for `pfe_profile`.
    pub pfe_quantile: f64,
}

impl StochasticExposureProfile {
    /// Maximum PFE across the simulated horizon.
    ///
    /// # Returns
    ///
    /// The largest value in [`Self::pfe_profile`].
    pub fn max_pfe(&self) -> f64 {
        self.pfe_profile.iter().copied().fold(0.0, f64::max)
    }

    /// Validate internal consistency between the average profile and PFE vector.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the embedded profile and tail profile are aligned.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded average profile is invalid or if the
    /// PFE vector length or values are inconsistent.
    pub fn validate(&self) -> finstack_core::Result<()> {
        self.profile.validate()?;
        if self.pfe_profile.len() != self.profile.times.len() {
            return Err(finstack_core::Error::Validation(format!(
                "StochasticExposureProfile: pfe_profile length {} must match profile length {}",
                self.pfe_profile.len(),
                self.profile.times.len()
            )));
        }
        for (i, pfe) in self.pfe_profile.iter().enumerate() {
            if !pfe.is_finite() || *pfe < 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "StochasticExposureProfile: pfe_profile[{i}] = {pfe} must be non-negative and finite"
                )));
            }
        }
        Ok(())
    }
}

/// A netting set: collection of trades under a single ISDA master agreement.
///
/// Under a valid ISDA master agreement, upon counterparty default the
/// portfolio is closed out on a net basis — positive and negative values
/// offset each other before determining the credit exposure.
///
/// # References
///
/// - ISDA 2002 Master Agreement: `docs/REFERENCES.md#isda-2002-master-agreement`
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct XvaNettingSet {
    /// Unique identifier for this netting set.
    pub id: String,

    /// Counterparty identifier (maps to hazard curve).
    pub counterparty_id: String,

    /// CSA terms (if any) governing collateral exchange.
    ///
    /// `None` means uncollateralized — full exposure is at risk.
    pub csa: Option<CsaTerms>,

    /// Optional reporting currency for netting, collateral, and exposure profiles.
    ///
    /// When omitted, single-currency portfolios use their natural currency.
    /// Mixed-currency portfolios must set this explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<finstack_core::currency::Currency>,
}

/// Credit Support Annex terms for collateralization.
///
/// Models the key economic terms of an ISDA CSA that determine
/// how collateral reduces counterparty credit exposure.
///
/// # Collateral Mechanics
///
/// ```text
/// Net exposure = Portfolio MtM - Collateral held
/// Collateral call = max(MtM - Threshold - MTA, 0)
/// Effective exposure = max(MtM - Collateral - IA, 0)
/// ```
///
/// The independent amount (IA) is additional collateral posted by the
/// counterparty that further reduces the credit exposure beyond
/// the variation margin collateral call.
///
/// # References
///
/// - ISDA 2016 VM CSA: `docs/REFERENCES.md#isda-vm-csa-2016`
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CsaTerms {
    /// Threshold below which no collateral is required.
    ///
    /// For investment-grade counterparties, typical thresholds
    /// range from $0 (zero threshold) to $50M.
    pub threshold: f64,

    /// Minimum transfer amount (MTA).
    ///
    /// Collateral is only exchanged when the call amount exceeds
    /// this minimum. Typical values: $250K–$1M.
    pub mta: f64,

    /// Margin period of risk (MPOR) in calendar days.
    ///
    /// The time needed to close out the portfolio after default.
    /// Regulatory standard: 10 days for bilateral, 5 days for cleared.
    ///
    /// **Note**: This field is stored for future MPOR-aware exposure modeling.
    /// The current deterministic exposure engine does not yet incorporate MPOR
    /// into collateral dynamics (the gap risk during the close-out period).
    pub mpor_days: u32,

    /// Independent amount (initial margin).
    ///
    /// Additional collateral posted regardless of MtM,
    /// reducing exposure by a fixed buffer.
    pub independent_amount: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = XvaConfig::default();
        config.validate().expect("Default config should be valid");
        assert_eq!(config.time_grid.len(), 120); // quarterly to 30Y
        assert!((config.recovery_rate - 0.40).abs() < f64::EPSILON);
    }

    #[test]
    fn config_validation_rejects_empty_grid() {
        let config = XvaConfig {
            time_grid: vec![],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_rejects_non_increasing_grid() {
        let config = XvaConfig {
            time_grid: vec![1.0, 0.5, 2.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_rejects_bad_recovery() {
        let config = XvaConfig {
            time_grid: vec![1.0, 2.0],
            recovery_rate: 1.5,
            own_recovery_rate: None,
            funding: None,
        };
        assert!(config.validate().is_err());

        let config_neg = XvaConfig {
            time_grid: vec![1.0, 2.0],
            recovery_rate: -0.1,
            own_recovery_rate: None,
            funding: None,
        };
        assert!(config_neg.validate().is_err());
    }

    #[test]
    fn config_validation_rejects_non_positive_times() {
        let config = XvaConfig {
            time_grid: vec![0.0, 1.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_rejects_funding_benefit_above_cost() {
        let config = XvaConfig {
            time_grid: vec![0.5, 1.0],
            recovery_rate: 0.40,
            own_recovery_rate: None,
            funding: Some(FundingConfig {
                funding_spread_bps: 35.0,
                funding_benefit_bps: Some(40.0),
            }),
        };
        assert!(config.validate().is_err());
    }

    // ── ExposureProfile validation tests ─────────────────────────

    #[test]
    fn profile_validate_valid() {
        let profile = ExposureProfile {
            times: vec![0.25, 0.5, 1.0],
            mtm_values: vec![100.0, -50.0, 25.0],
            epe: vec![100.0, 0.0, 25.0],
            ene: vec![0.0, 50.0, 0.0],
            diagnostics: None,
        };
        profile.validate().expect("Valid profile should pass");
    }

    #[test]
    fn profile_validate_rejects_mismatched_lengths() {
        let profile = ExposureProfile {
            times: vec![0.25, 0.5],
            mtm_values: vec![100.0],
            epe: vec![100.0, 0.0],
            ene: vec![0.0, 50.0],
            diagnostics: None,
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_negative_epe() {
        let profile = ExposureProfile {
            times: vec![0.25],
            mtm_values: vec![100.0],
            epe: vec![-1.0],
            ene: vec![0.0],
            diagnostics: None,
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_nan_mtm() {
        let profile = ExposureProfile {
            times: vec![0.25],
            mtm_values: vec![f64::NAN],
            epe: vec![0.0],
            ene: vec![0.0],
            diagnostics: None,
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_non_increasing_times() {
        let profile = ExposureProfile {
            times: vec![1.0, 0.5],
            mtm_values: vec![100.0, 50.0],
            epe: vec![100.0, 50.0],
            ene: vec![0.0, 0.0],
            diagnostics: None,
        };
        assert!(profile.validate().is_err());
    }
}
