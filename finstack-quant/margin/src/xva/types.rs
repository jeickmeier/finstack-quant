//! Core types for the XVA (Valuation Adjustments) framework.
//!
//! Defines configuration, result containers, netting set specifications,
//! and CSA (Credit Support Annex) terms used across all XVA calculations.

use crate::xva::mva::ImProfile;

/// Funding cost/benefit configuration for FVA and MVA calculation.
///
/// Models the asymmetric funding costs that arise from uncollateralized
/// derivative positions. Positive exposure requires funding (cost),
/// while negative exposure provides funding (benefit).
///
/// Setting [`im_profile`](Self::im_profile) additionally turns on MVA — the
/// funding cost of the initial margin the desk posts over the life of the
/// netting set. Both adjustments are driven off the same unsecured funding
/// spread unless [`margin_funding_spread_bp`](Self::margin_funding_spread_bp)
/// overrides it.
///
/// # Examples
///
/// ```
/// use finstack_quant_margin::xva::types::FundingConfig;
///
/// // FVA only.
/// let fva_only = FundingConfig {
///     funding_spread_bp: 50.0,
///     ..Default::default()
/// };
/// assert!(fva_only.im_profile.is_none());
/// ```
///
/// # References
///
/// - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
/// - Green XVA: `docs/REFERENCES.md#green-xva`
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FundingConfig {
    /// Funding spread in basis points (cost on positive exposure).
    ///
    /// This is the spread over the risk-free rate that the institution
    /// pays to fund positive (out-of-the-money to counterparty) exposure.
    /// Typical values: 20–100 bp depending on the institution's credit quality.
    pub funding_spread_bp: f64,

    /// Funding benefit spread in basis points (benefit on negative exposure).
    ///
    /// If `None`, symmetric funding is assumed: `funding_benefit = funding_spread`.
    /// In practice, the benefit spread may be lower than the cost spread
    /// due to asymmetric funding conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding_benefit_bp: Option<f64>,

    /// Expected initial-margin profile `E[IM(t)]` that drives MVA.
    ///
    /// When `Some`, [`crate::xva::cva::compute_bilateral_xva`] prices the
    /// lifetime funding cost of posting this IM and reports it as
    /// [`XvaResult::mva`]. When `None`, MVA is not computed and
    /// [`XvaResult::mva`] is `None`.
    ///
    /// Build the profile with
    /// [`crate::xva::mva::im_profile_from_simm`] (deterministic SIMM decay), or
    /// supply the path-consistent mean IM from your own simulation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub im_profile: Option<ImProfile>,

    /// Spread in basis points applied to posted initial margin (MVA).
    ///
    /// If `None`, MVA uses `funding_spread_bp` — the desk's unsecured
    /// funding spread, which is the standard assumption (Green 2015, ch. 10).
    /// Override when IM is funded at a different (typically term or partially
    /// secured) level than uncollateralized derivative exposure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_funding_spread_bp: Option<f64>,
}

impl FundingConfig {
    /// Validate funding and initial-margin parameters.
    ///
    /// # Returns
    ///
    /// `Ok(())` when all spreads and the optional IM profile are internally
    /// consistent.
    ///
    /// # Errors
    ///
    /// Returns an error when a spread is negative or non-finite, the funding
    /// benefit exceeds the funding cost, or the optional IM profile is invalid.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.funding_spread_bp.is_finite() || self.funding_spread_bp < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FundingConfig: funding_spread_bp {} must be non-negative and finite",
                self.funding_spread_bp
            )));
        }
        if let Some(benefit) = self.funding_benefit_bp {
            if !benefit.is_finite() || benefit < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FundingConfig: funding_benefit_bp {benefit} must be non-negative and finite"
                )));
            }
            if benefit > self.funding_spread_bp {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FundingConfig: funding_benefit_bp {benefit} must not exceed \
                     funding_spread_bp {}",
                    self.funding_spread_bp
                )));
            }
        }
        if let Some(margin_spread) = self.margin_funding_spread_bp {
            if !margin_spread.is_finite() || margin_spread < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FundingConfig: margin_funding_spread_bp {margin_spread} must be \
                     non-negative and finite"
                )));
            }
        }
        if let Some(ref im_profile) = self.im_profile {
            im_profile.validate()?;
        }
        Ok(())
    }

    /// Returns the effective funding benefit spread in basis points.
    ///
    /// If `funding_benefit_bp` is `None`, returns `funding_spread_bp`
    /// (symmetric funding assumption).
    ///
    /// # Returns
    ///
    /// The benefit spread in basis points.
    pub fn effective_benefit_bp(&self) -> f64 {
        self.funding_benefit_bp.unwrap_or(self.funding_spread_bp)
    }

    /// Returns the effective IM funding spread in basis points (MVA).
    ///
    /// If `margin_funding_spread_bp` is `None`, returns `funding_spread_bp`.
    ///
    /// # Returns
    ///
    /// The IM funding spread in basis points.
    pub fn effective_margin_spread_bp(&self) -> f64 {
        self.margin_funding_spread_bp
            .unwrap_or(self.funding_spread_bp)
    }
}

/// Result of XVA calculations.
///
/// Contains the CVA value along with the full exposure profile and
/// regulatory metrics used for reporting and risk management.
///
/// # Sign convention and composition
///
/// Every adjustment is reported as a **positive number when it is a cost to
/// the desk**. They compose additively into the all-in adjustment:
///
/// ```text
/// total_xva = CVA − DVA + FVA + MVA
/// ```
///
/// `total_xva` is the quantity subtracted from the risk-free value of the
/// netting set. Uncomputed optional legs contribute zero.
///
/// # Exposure Profiles
///
/// Each profile entry is a `(time, value)` pair where time is in years
/// from the valuation date and value is in the portfolio's base currency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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

    /// MVA (Margin Valuation Adjustment): funding cost of posted initial margin.
    ///
    /// Positive MVA represents the lifetime cost of funding the initial
    /// margin the desk posts against the netting set. Uses the same sign
    /// convention as CVA and FVA.
    ///
    /// `None` when MVA is not computed — that is, when no
    /// [`FundingConfig::im_profile`] was supplied.
    ///
    /// # References
    ///
    /// - Green, A. (2015). *XVA*. Wiley. Chapter 10. `docs/REFERENCES.md#green-xva`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mva: Option<f64>,

    /// All-in valuation adjustment: `CVA − DVA + FVA + MVA`.
    ///
    /// Uncomputed components contribute zero. This is the quantity subtracted
    /// from the risk-free value of the netting set.
    ///
    /// # References
    ///
    /// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 14. `docs/REFERENCES.md#gregory-xva-challenge`
    /// - Green, A. (2015). *XVA*. Wiley. Chapters 9-10. `docs/REFERENCES.md#green-xva`
    pub total_xva: f64,

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
    /// their column bindings; supply a profile from a Monte Carlo exposure
    /// simulation when a true 97.5%-quantile PFE is required for limit
    /// monitoring.
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
    ///   counterparty credit risk exposures." `docs/REFERENCES.md#bcbs-279-saccr`
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
    ///   counterparty credit risk exposures." `docs/REFERENCES.md#bcbs-279-saccr`
    pub effective_epe: f64,

    /// Policy metadata stamped by the computing layer: numeric mode, active
    /// rounding context, any applied FX policy, and the parallel-execution
    /// flag.
    #[serde(default)]
    pub meta: finstack_quant_core::config::ResultsMeta,
}

/// Diagnostics from exposure simulation capturing data quality metrics.
///
/// Populated by the exposure computation engine to let callers distinguish
/// genuine zero exposure from missing data.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let n = self.times.len();

        if self.diagnostics.as_ref().is_some_and(|diagnostics| {
            diagnostics.market_roll_failures > 0 || diagnostics.valuation_failures > 0
        }) {
            return Err(finstack_quant_core::Error::Validation(
                "ExposureProfile contains failed market rolls or instrument valuations and is not valid for XVA"
                    .into(),
            ));
        }

        if self.mtm_values.len() != n || self.epe.len() != n || self.ene.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
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
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: times[{i}] = {t} must be positive and finite"
                )));
            }
            if i > 0 && t <= self.times[i - 1] {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: times must be strictly increasing at index {i}"
                )));
            }
        }

        for (i, (&epe_v, &ene_v)) in self.epe.iter().zip(self.ene.iter()).enumerate() {
            if !epe_v.is_finite() || epe_v < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: epe[{i}] = {epe_v} must be non-negative and finite"
                )));
            }
            if !ene_v.is_finite() || ene_v < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: ene[{i}] = {ene_v} must be non-negative and finite"
                )));
            }
        }

        for (i, &mtm) in self.mtm_values.iter().enumerate() {
            if !mtm.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: mtm_values[{i}] = {mtm} must be finite"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funding_config_defaults_leave_mva_off() {
        let fva_only = FundingConfig {
            funding_spread_bp: 50.0,
            ..Default::default()
        };
        assert!(fva_only.im_profile.is_none());
        assert!(fva_only.margin_funding_spread_bp.is_none());
        // Both derived spreads fall back to the single quoted funding spread.
        assert!((fva_only.effective_benefit_bp() - 50.0).abs() < f64::EPSILON);
        assert!((fva_only.effective_margin_spread_bp() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn funding_config_optional_mva_fields_may_be_omitted() {
        let minimal = r#"{"funding_spread_bp":50.0,"funding_benefit_bp":30.0}"#;
        let parsed: FundingConfig = serde_json::from_str(minimal).expect("minimal payload parses");
        assert!(parsed.im_profile.is_none());
        assert!(parsed.margin_funding_spread_bp.is_none());

        // Unset optionals stay off the wire.
        let json = serde_json::to_string(&parsed).expect("serialize");
        assert!(!json.contains("im_profile"), "{json}");
        assert!(!json.contains("margin_funding_spread_bp"), "{json}");
    }

    #[test]
    fn funding_config_rejects_unknown_json_fields() {
        let json = r#"{"funding_spread_bp":50.0,"funding_spred_bp":40.0}"#;
        assert!(
            serde_json::from_str::<FundingConfig>(json).is_err(),
            "misspelled funding fields must fail closed"
        );
    }

    #[test]
    fn funding_config_validates_direct_use() {
        let invalid = FundingConfig {
            funding_spread_bp: -1.0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());

        let benefit_above_cost = FundingConfig {
            funding_spread_bp: 25.0,
            funding_benefit_bp: Some(30.0),
            ..Default::default()
        };
        assert!(benefit_above_cost.validate().is_err());
    }

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
