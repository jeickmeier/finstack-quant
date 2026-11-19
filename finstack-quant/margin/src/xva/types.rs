//! Core types for the XVA (Valuation Adjustments) framework.
//!
//! Defines configuration, result containers, netting set specifications,
//! and CSA (Credit Support Annex) terms used across all XVA calculations.

use super::mva::{ImProfile, MarginFundingSpread, MvaResult};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;

/// Funding cost/benefit configuration for FVA and MVA calculation.
///
/// Models the asymmetric funding costs that arise from uncollateralized
/// derivative positions. Positive exposure requires funding (cost),
/// while negative exposure provides funding (benefit).
///
/// Setting [`im_profile`](Self::im_profile) additionally turns on MVA — the
/// funding cost of the initial margin the desk posts over the life of the
/// netting set. Elect a separate [funding spread](Self::margin_funding_spread)
/// for the posted margin profile.
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

    /// Explicit, separately elected funding spread for posted IM.
    /// Supply it together with the IM profile to enable MVA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_funding_spread: Option<MarginFundingSpread>,
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
        match (&self.im_profile, &self.margin_funding_spread) {
            (None, None) => Ok(()),
            (Some(profile), Some(spread)) => {
                profile.validate()?;
                let end = profile.dates[profile.dates.len() - 1];
                spread.add_knots(profile.as_of, end, &mut Vec::new())
            }
            _ => Err(finstack_quant_core::Error::Validation(
                "FundingConfig requires the posted IM profile and its elected funding spread together".into(),
            )),
        }
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

    /// Dated expected IM, average IM and replayable funding intervals when MVA was computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mva_detail: Option<MvaResult>,

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

    /// Maximum expected positive exposure across the profile, in reporting currency.
    pub max_epe: f64,

    /// Effective EPE profile: `(time, Effective_EPE(t))`.
    ///
    /// Running maximum of expected positive exposure:
    /// `Effective_EPE(t_k) = max(Effective_EPE(t_{k-1}), EPE(t_k))`
    ///
    /// # References
    ///
    /// - BCBS 279 (2014). "The standardised approach for measuring
    ///   counterparty credit risk exposures." `docs/REFERENCES.md#bcbs-279-saccr`
    pub effective_epe_profile: Vec<(f64, f64)>,

    /// Time-weighted diagnostic of the right-endpoint running maximum of EPE.
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

    /// Per-time-bucket terms of the CVA sum, one row per exposure-profile
    /// node, in time order. Summing `contribution` in row order reproduces
    /// `cva`. Empty only when deserialized from JSON written before this
    /// field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cva_rows: Vec<CreditXvaRow>,

    /// Per-time-bucket terms of the DVA sum, in time order; `contribution`
    /// sums to `dva`. `Some` exactly when `dva` is `Some` (bilateral XVA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dva_rows: Option<Vec<CreditXvaRow>>,

    /// Per-time-bucket terms of the FVA sum, in time order; `contribution`
    /// sums to `fva`. `Some` exactly when `fva` is `Some` (bilateral XVA with
    /// a funding configuration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fva_rows: Option<Vec<FundingXvaRow>>,

    /// Policy metadata stamped by the computing layer: numeric mode, active
    /// rounding context, any applied FX policy, and the parallel-execution
    /// flag.
    #[serde(default)]
    pub meta: finstack_quant_core::config::ResultsMeta,
}

/// One time bucket of a CVA or DVA sum.
///
/// The bucket ends at `time` and starts at the previous row's `time` (the
/// first bucket starts at the valuation date, where survival and the discount
/// factor are `1` and the exposure is held flat at the first profile node).
/// The row records the factors exactly as multiplied:
///
/// ```text
/// contribution = loss_given_default × exposure_mid
///              × marginal_default_probability × discount_factor_mid
///              × survival_weight
/// ```
///
/// For CVA the defaulting party is the counterparty and the exposure is EPE;
/// for DVA the defaulting party is the institution and the exposure is ENE
/// net of any posted initial margin, floored at zero.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditXvaRow {
    /// Bucket end in years from the valuation date.
    pub time: f64,
    /// Discount factor `DF(time)` read from the discount curve.
    pub discount_factor: f64,
    /// Survival probability `S(time)` of the defaulting party read from its
    /// hazard curve, in `[0, 1]`.
    pub survival_probability: f64,
    /// Probability that the defaulting party defaults inside the bucket:
    /// `max(S(previous time) − S(time), 0)`.
    pub marginal_default_probability: f64,
    /// Average of the exposure at the bucket start and end, in the exposure
    /// profile's currency (non-negative).
    pub exposure_mid: f64,
    /// Average of the discount factors at the bucket start and end.
    pub discount_factor_mid: f64,
    /// Average over the bucket of the *other* party's survival probability
    /// (first-to-default weighting): the institution's for CVA, the
    /// counterparty's for DVA. Exactly `1` for a unilateral adjustment.
    pub survival_weight: f64,
    /// Loss given default `1 − recovery rate` of the defaulting party, as a
    /// decimal fraction; identical on every row.
    pub loss_given_default: f64,
    /// This bucket's term of the adjustment, in the exposure profile's
    /// currency (non-negative).
    pub contribution: f64,
}

/// One time bucket of an FVA sum.
///
/// The bucket ends at `time` and starts at the previous row's `time` (the
/// first bucket starts at the valuation date). The row records the factors
/// exactly as multiplied:
///
/// ```text
/// contribution = (epe_mid × funding_spread − ene_mid × funding_benefit_spread)
///              × discount_factor_mid × dt × joint_survival_mid
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FundingXvaRow {
    /// Bucket end in years from the valuation date.
    pub time: f64,
    /// Bucket length in years: `time` minus the previous row's `time`.
    pub dt: f64,
    /// Discount factor `DF(time)` read from the discount curve.
    pub discount_factor: f64,
    /// Average expected positive exposure over the bucket, in the exposure
    /// profile's currency.
    pub epe_mid: f64,
    /// Average expected negative exposure over the bucket, in the exposure
    /// profile's currency (non-negative magnitude).
    pub ene_mid: f64,
    /// Average of the discount factors at the bucket start and end.
    pub discount_factor_mid: f64,
    /// Product of the bucket-average counterparty and own survival
    /// probabilities. Exactly `1` when no hazard curves are applied.
    pub joint_survival_mid: f64,
    /// Funding cost spread applied to `epe_mid`, as a decimal fraction per
    /// year (basis points / 10 000); identical on every row.
    pub funding_spread: f64,
    /// Funding benefit spread applied to `ene_mid`, as a decimal fraction per
    /// year (basis points / 10 000); identical on every row.
    pub funding_benefit_spread: f64,
    /// This bucket's term of the FVA, in the exposure profile's currency
    /// (positive = funding cost).
    pub contribution: f64,
}

/// Diagnostics from exposure simulation capturing data quality metrics.
///
/// Populated by the exposure computation engine to let callers distinguish
/// genuine zero exposure from missing data.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExposureDiagnostics {
    /// Number of time grid points where market data could not be rolled forward.
    pub market_roll_failures: usize,

    /// Total number of individual instrument valuation failures across all time points.
    pub valuation_failures: usize,

    /// Total time grid points evaluated.
    pub total_time_points: usize,
}

/// Provenance and basis of all observations in an XVA exposure profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExposureBasis {
    /// External complete exposure observations, supplied on a documented common basis.
    External {
        /// Stable identifier of the source data and its completeness attestation.
        source_id: String,
        /// Identifier of the common netting, collateral and model-time policy.
        policy_id: String,
    },
    /// A portfolio repriced on every path and date before taking positive and negative parts.
    SimulatedUncollateralized {
        /// Fingerprint of the pricing, market, portfolio and process inputs.
        source_id: String,
        /// Number of complete simulated paths including any antithetic partners.
        #[serde(with = "finstack_quant_core::wire::count")]
        #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
        path_count: usize,
    },
    /// Repriced paths with elected contractual cash variation margin.
    SimulatedCashVm {
        /// Fingerprint of the market, priced book, simulation and CSA.
        source_id: String,
        /// Number of complete paths including antithetic partners.
        #[serde(with = "finstack_quant_core::wire::count")]
        #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
        path_count: usize,
    },
}

/// Expected exposure observations on a complete dated grid.
///
/// All marks and exposures use major units of the reporting currency.
/// Integrators query each discount or survival curve using the calendar
/// dates. Exposure year fractions define the profile's own integration clock.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExposureProfile {
    /// Valuation cut and required opening observation date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,

    /// Reporting currency shared by all marks, exposures and adjustments.
    pub currency: Currency,

    /// Provenance of the exposure model and the complete observation grid.
    pub basis: ExposureBasis,

    /// Strictly increasing calendar observations starting on the valuation cut.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,

    /// Aligned model year fractions, starting at zero and strictly increasing.
    pub times: Vec<f64>,

    /// Portfolio mark-to-market value at each time point (may be negative).
    pub mtm_values: Vec<f64>,

    /// Expected positive part of the exposed mark at each date, after any
    /// settled collateral in the elected basis.
    pub epe: Vec<f64>,

    /// Expected positive part of the negated exposed mark at each date,
    /// after any settled collateral in the elected basis.
    pub ene: Vec<f64>,

    /// Completeness counters for the admitted grid. Failed points or valuations invalidate it.
    pub diagnostics: ExposureDiagnostics,
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

        let basis_is_identified = match &self.basis {
            ExposureBasis::External {
                source_id,
                policy_id,
            } => !source_id.trim().is_empty() && !policy_id.trim().is_empty(),
            ExposureBasis::SimulatedUncollateralized {
                source_id,
                path_count,
            }
            | ExposureBasis::SimulatedCashVm {
                source_id,
                path_count,
            } => !source_id.trim().is_empty() && *path_count > 0,
        };
        if !basis_is_identified {
            return Err(finstack_quant_core::Error::Validation(
                "ExposureProfile requires identified and complete exposure provenance".into(),
            ));
        }
        if self.dates.first() != Some(&self.as_of) || self.times.first() != Some(&0.0) {
            return Err(finstack_quant_core::Error::Validation(
                "ExposureProfile must start at the valuation cut with an explicit time-zero node"
                    .into(),
            ));
        }
        if self.diagnostics.total_time_points != n
            || self.diagnostics.market_roll_failures > 0
            || self.diagnostics.valuation_failures > 0
        {
            return Err(finstack_quant_core::Error::Validation(
                "ExposureProfile diagnostics must reconcile with the grid and show every roll and valuation completed".into(),
            ));
        }

        if self.dates.len() != n
            || self.mtm_values.len() != n
            || self.epe.len() != n
            || self.ene.len() != n
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ExposureProfile: vector lengths must be equal \
                 (dates={}, times={}, mtm={}, epe={}, ene={})",
                self.dates.len(),
                n,
                self.mtm_values.len(),
                self.epe.len(),
                self.ene.len()
            )));
        }

        if self.dates.windows(2).any(|dates| dates[0] >= dates[1]) {
            return Err(finstack_quant_core::Error::Validation(
                "ExposureProfile dates must strictly increase".into(),
            ));
        }

        for (i, &t) in self.times.iter().enumerate() {
            if !t.is_finite() || t < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ExposureProfile: times[{i}] = {t} must be nonnegative and finite"
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
    fn exposure_profile_rejects_unknown_fields() {
        let base = serde_json::json!({
            "as_of": "2025-01-01", "dates": ["2025-01-01"], "currency": "USD",
            "basis": {"kind": "external", "source_id": "test-data", "policy_id": "flat-test"},
            "times": [0.0], "mtm_values": [1.0], "epe": [1.0], "ene": [0.0],
            "diagnostics": {"market_roll_failures": 0, "valuation_failures": 0, "total_time_points": 1}
        });
        serde_json::from_value::<ExposureProfile>(base.clone()).expect("base profile parses");
        let diagnostics = serde_json::json!({
            "market_roll_failures": 3, "valuation_failures": 0, "total_time_points": 1
        });
        let mut typo = base.clone();
        typo["diagnostic"] = diagnostics.clone();
        let error = serde_json::from_value::<ExposureProfile>(typo)
            .expect_err("a misspelled diagnostics key must not bypass the XVA gate");
        assert!(
            error.to_string().contains("unknown field `diagnostic`"),
            "{error}"
        );
        let mut nested = base;
        let mut bad_diagnostics = diagnostics;
        bad_diagnostics["bogus"] = serde_json::json!(1);
        nested["diagnostics"] = bad_diagnostics;
        let error = serde_json::from_value::<ExposureProfile>(nested)
            .expect_err("unknown diagnostics fields must be rejected");
        assert!(
            error.to_string().contains("unknown field `bogus`"),
            "{error}"
        );
    }

    #[test]
    fn funding_config_defaults_leave_mva_off() {
        let fva_only = FundingConfig {
            funding_spread_bp: 50.0,
            ..Default::default()
        };
        assert!(fva_only.im_profile.is_none());
        assert!(fva_only.margin_funding_spread.is_none());
        assert!((fva_only.effective_benefit_bp() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn funding_config_optional_mva_fields_may_be_omitted() {
        let minimal = r#"{"funding_spread_bp":50.0,"funding_benefit_bp":30.0}"#;
        let parsed: FundingConfig = serde_json::from_str(minimal).expect("minimal payload parses");
        assert!(parsed.im_profile.is_none());
        assert!(parsed.margin_funding_spread.is_none());

        // Unset optionals stay off the wire.
        let json = serde_json::to_string(&parsed).expect("serialize");
        assert!(!json.contains("im_profile"), "{json}");
        assert!(!json.contains("margin_funding_spread"), "{json}");
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
            as_of: time::macros::date!(2025 - 01 - 01),
            dates: vec![
                time::macros::date!(2025 - 01 - 01),
                time::macros::date!(2025 - 07 - 01),
                time::macros::date!(2026 - 01 - 01),
            ],
            currency: Currency::USD,
            basis: ExposureBasis::External {
                source_id: "test-grid".into(),
                policy_id: "test-basis".into(),
            },
            times: vec![0.0, 0.5, 1.0],
            mtm_values: vec![100.0, -50.0, 25.0],
            epe: vec![100.0, 0.0, 25.0],
            ene: vec![0.0, 50.0, 0.0],
            diagnostics: ExposureDiagnostics {
                total_time_points: 3,
                ..Default::default()
            },
        };
        profile.validate().expect("Valid profile should pass");
    }

    #[test]
    fn profile_validate_rejects_mismatched_lengths() {
        let profile = ExposureProfile {
            as_of: time::macros::date!(2025 - 01 - 01),
            dates: vec![
                time::macros::date!(2025 - 01 - 01),
                time::macros::date!(2025 - 07 - 01),
            ],
            currency: Currency::USD,
            basis: ExposureBasis::External {
                source_id: "test-grid".into(),
                policy_id: "test-basis".into(),
            },
            times: vec![0.0, 0.5],
            mtm_values: vec![100.0],
            epe: vec![100.0, 0.0],
            ene: vec![0.0, 50.0],
            diagnostics: ExposureDiagnostics {
                total_time_points: 2,
                ..Default::default()
            },
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_negative_epe() {
        let profile = ExposureProfile {
            as_of: time::macros::date!(2025 - 01 - 01),
            dates: vec![time::macros::date!(2025 - 01 - 01)],
            currency: Currency::USD,
            basis: ExposureBasis::External {
                source_id: "test-grid".into(),
                policy_id: "test-basis".into(),
            },
            times: vec![0.0],
            mtm_values: vec![100.0],
            epe: vec![-1.0],
            ene: vec![0.0],
            diagnostics: ExposureDiagnostics {
                total_time_points: 1,
                ..Default::default()
            },
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_nan_mtm() {
        let profile = ExposureProfile {
            as_of: time::macros::date!(2025 - 01 - 01),
            dates: vec![time::macros::date!(2025 - 01 - 01)],
            currency: Currency::USD,
            basis: ExposureBasis::External {
                source_id: "test-grid".into(),
                policy_id: "test-basis".into(),
            },
            times: vec![0.0],
            mtm_values: vec![f64::NAN],
            epe: vec![0.0],
            ene: vec![0.0],
            diagnostics: ExposureDiagnostics {
                total_time_points: 1,
                ..Default::default()
            },
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn profile_validate_rejects_non_increasing_times() {
        let profile = ExposureProfile {
            as_of: time::macros::date!(2025 - 01 - 01),
            dates: vec![
                time::macros::date!(2025 - 01 - 01),
                time::macros::date!(2025 - 07 - 01),
            ],
            currency: Currency::USD,
            basis: ExposureBasis::External {
                source_id: "test-grid".into(),
                policy_id: "test-basis".into(),
            },
            times: vec![0.0, 0.0],
            mtm_values: vec![100.0, 50.0],
            epe: vec![100.0, 50.0],
            ene: vec![0.0, 0.0],
            diagnostics: ExposureDiagnostics {
                total_time_points: 2,
                ..Default::default()
            },
        };
        assert!(profile.validate().is_err());
    }
}
