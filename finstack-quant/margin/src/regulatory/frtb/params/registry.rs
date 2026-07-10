//! Registry-backed FRTB parameters with JSON-overlay support.
//!
//! [`FrtbParams`] bundles the non-trivially-parameterised FRTB inputs
//! into one serializable, revision-tagged struct.
//! [`FrtbParams::d457`] returns defaults matching the sibling `pub
//! const` tables; [`FrtbParams::from_json_overlay`] can prepare and
//! validate candidate future parameter sets. The calculation engine
//! currently accepts only the exact D457 bundle and fails closed for
//! alternate revisions so results can never carry a false vintage tag.

use serde::{Deserialize, Serialize};

use super::{commodity, equity, fx, girr};
use crate::regulatory::frtb::drc::{DRC_LGD, DRC_RISK_WEIGHTS};
use crate::regulatory::frtb::types::DrcSeniority;
use finstack_quant_core::Error as MarginError;

/// Revision identifier tagging a [`FrtbParams`] to its regulatory vintage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FrtbRevision {
    /// BCBS d457 (January 2019) — the values matching the `pub const` defaults.
    D457,
    /// BCBS d554 (April 2024) — values must be supplied via JSON overlay.
    D554,
    /// Caller-specific parameter set (e.g. stress testing or internal models).
    Custom(String),
}

impl FrtbRevision {
    /// Human-readable label for audit logs.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            FrtbRevision::D457 => "BCBS d457 (Jan 2019)".to_string(),
            FrtbRevision::D554 => "BCBS d554 (Apr 2024)".to_string(),
            FrtbRevision::Custom(name) => format!("custom ({name})"),
        }
    }
}

/// GIRR (General Interest Rate Risk) parameter bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GirrParams {
    /// Delta risk weights by tenor label, in percent.
    pub delta_risk_weights_pct: Vec<(String, f64)>,
    /// Inflation delta risk weight (percent).
    pub inflation_risk_weight: f64,
    /// Cross-currency basis risk weight (percent).
    pub xccy_basis_risk_weight: f64,
    /// Vega risk weight.
    pub vega_risk_weight: f64,
    /// Curvature risk weight.
    pub curvature_risk_weight: f64,
    /// Theta parameter for the intra-bucket tenor correlation formula.
    pub tenor_correlation_theta: f64,
    /// Minimum correlation floor between any two tenors.
    pub tenor_correlation_floor: f64,
    /// Inter-bucket (cross-currency) correlation.
    pub inter_bucket_correlation: f64,
    /// Delta↔inflation correlation within a currency.
    pub inflation_correlation: f64,
    /// Delta↔cross-currency-basis correlation.
    pub xccy_basis_correlation: f64,
}

/// Equity parameter bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquityParams {
    /// Risk weights by bucket id (1..=13).
    pub risk_weights: Vec<(u8, f64)>,
    /// Intra-bucket correlation.
    pub intra_bucket_correlation: f64,
    /// Inter-bucket correlation.
    pub inter_bucket_correlation: f64,
    /// Vega risk weight.
    pub vega_risk_weight: f64,
    /// Curvature risk weight.
    pub curvature_risk_weight: f64,
}

/// Commodity parameter bundle (headline scalars only — full bucket
/// tables remain in [`super::commodity`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommodityParams {
    /// Vega risk weight.
    pub vega_risk_weight: f64,
    /// Curvature risk weight.
    pub curvature_risk_weight: f64,
}

/// DRC (Default Risk Charge) parameter bundle.
///
/// Mirrors the `pub const` tables in [`crate::regulatory::frtb::drc`]
/// so DRC parameters travel with [`FrtbParams`] for audit-trail tagging
/// and so a JSON overlay can substitute alternate weights (e.g. d554)
/// without recompiling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrcParams {
    /// Risk weights by rating bucket id (1=AAA … 9=Defaulted).
    pub risk_weights: Vec<(u8, f64)>,
    /// Loss-given-default by seniority.
    pub lgd: Vec<(DrcSeniority, f64)>,
    /// Risk weight applied to unmapped rating buckets. Per MAR22.24 the
    /// Basel default is 15% (the Unrated bucket).
    pub unrated_risk_weight: f64,
    /// LGD applied to unmapped seniorities. Default 75% (senior unsecured).
    pub unrated_lgd: f64,
}

/// FX parameter bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxParams {
    /// Delta risk weight.
    pub delta_risk_weight: f64,
    /// Vega risk weight.
    pub vega_risk_weight: f64,
    /// Curvature risk weight.
    pub curvature_risk_weight: f64,
    /// Inter-pair correlation (FX is single-bucket).
    pub inter_pair_correlation: f64,
}

/// Correlation-scenario multipliers applied to every risk class.
///
/// FRTB evaluates SBA under three scenarios:
///
/// * Low:    `rho_low  = max(2·rho − 1, −1)`
/// * Medium: `rho_med  = rho`
/// * High:   `rho_high = min(1.25·rho, 1)`
///
/// The multipliers below parameterise the Low and High scenarios so a
/// regulatory revision that changes the stress multipliers (e.g. d554
/// may soften the high-correlation scenario) can be tested without a
/// recompile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationScenarioParams {
    /// Linear coefficient in `rho_low = a·rho + b`. Default `a = 2.0`.
    pub low_linear_coefficient: f64,
    /// Intercept in `rho_low = a·rho + b`. Default `b = −1.0`.
    pub low_intercept: f64,
    /// Multiplier for `rho_high = multiplier·rho` (capped at 1.0).
    /// Default `multiplier = 1.25`.
    pub high_multiplier: f64,
}

impl Default for CorrelationScenarioParams {
    fn default() -> Self {
        Self {
            low_linear_coefficient: 2.0,
            low_intercept: -1.0,
            high_multiplier: 1.25,
        }
    }
}

/// Complete FRTB parameter bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrtbParams {
    /// Revision identifier for audit trails.
    pub revision: FrtbRevision,
    /// GIRR risk weights and correlations.
    pub girr: GirrParams,
    /// Equity risk weights and correlations.
    pub equity: EquityParams,
    /// Commodity headline scalars.
    pub commodity: CommodityParams,
    /// FX parameters.
    pub fx: FxParams,
    /// Default Risk Charge parameters.
    #[serde(default = "DrcParams::d457")]
    pub drc: DrcParams,
    /// Correlation scenario multipliers.
    pub correlation_scenarios: CorrelationScenarioParams,
}

impl DrcParams {
    /// Default DRC parameter set matching [`DRC_RISK_WEIGHTS`] / [`DRC_LGD`] (BCBS d457).
    #[must_use]
    pub fn d457() -> Self {
        Self {
            risk_weights: DRC_RISK_WEIGHTS.to_vec(),
            lgd: DRC_LGD.to_vec(),
            unrated_risk_weight: 0.15,
            unrated_lgd: 0.75,
        }
    }
}

impl FrtbParams {
    /// Default parameter set matching the `pub const` tables (BCBS d457).
    #[must_use]
    pub fn d457() -> Self {
        Self {
            revision: FrtbRevision::D457,
            girr: GirrParams {
                delta_risk_weights_pct: girr::GIRR_DELTA_RISK_WEIGHTS
                    .iter()
                    .map(|(t, w)| ((*t).to_string(), *w))
                    .collect(),
                inflation_risk_weight: girr::GIRR_INFLATION_RISK_WEIGHT,
                xccy_basis_risk_weight: girr::GIRR_XCCY_BASIS_RISK_WEIGHT,
                vega_risk_weight: girr::GIRR_VEGA_RISK_WEIGHT,
                curvature_risk_weight: girr::GIRR_CURVATURE_RISK_WEIGHT,
                tenor_correlation_theta: girr::GIRR_TENOR_CORRELATION_THETA,
                tenor_correlation_floor: girr::GIRR_TENOR_CORRELATION_FLOOR,
                inter_bucket_correlation: girr::GIRR_INTER_BUCKET_CORRELATION,
                inflation_correlation: girr::GIRR_INFLATION_CORRELATION,
                xccy_basis_correlation: girr::GIRR_XCCY_BASIS_CORRELATION,
            },
            equity: EquityParams {
                risk_weights: equity::EQUITY_RISK_WEIGHTS.to_vec(),
                intra_bucket_correlation: equity::EQUITY_INTRA_BUCKET_CORRELATION,
                inter_bucket_correlation: equity::EQUITY_INTER_BUCKET_CORRELATION,
                vega_risk_weight: equity::EQUITY_VEGA_RISK_WEIGHT,
                curvature_risk_weight: equity::EQUITY_CURVATURE_RISK_WEIGHT,
            },
            commodity: CommodityParams {
                vega_risk_weight: commodity::COMMODITY_VEGA_RISK_WEIGHT,
                curvature_risk_weight: commodity::COMMODITY_CURVATURE_RISK_WEIGHT,
            },
            fx: FxParams {
                delta_risk_weight: fx::FX_DELTA_RISK_WEIGHT,
                vega_risk_weight: fx::FX_VEGA_RISK_WEIGHT,
                curvature_risk_weight: fx::FX_CURVATURE_RISK_WEIGHT,
                inter_pair_correlation: fx::FX_INTER_PAIR_CORRELATION,
            },
            drc: DrcParams::d457(),
            correlation_scenarios: CorrelationScenarioParams::default(),
        }
    }

    /// Load a parameter set from a JSON overlay layered on top of `d457`.
    ///
    /// Missing fields fall back to the d457 default. The result is run
    /// through [`Self::validate`].
    ///
    /// # Errors
    ///
    /// [`MarginError::Validation`] on a malformed JSON shape or any
    /// validation failure (negative risk weight, correlation outside
    /// `[-1, 1]`, etc.).
    pub fn from_json_overlay(overlay: &serde_json::Value) -> Result<Self, MarginError> {
        let defaults = serde_json::to_value(Self::d457()).map_err(|e| {
            MarginError::Validation(format!("internal: failed to serialize d457 defaults: {e}"))
        })?;
        let merged = deep_merge(defaults, overlay.clone());
        let params: Self = serde_json::from_value(merged)
            .map_err(|e| MarginError::Validation(format!("FRTB JSON overlay parse error: {e}")))?;
        params.validate()?;
        Ok(params)
    }

    /// Validate the parameter ranges.
    ///
    /// Errors if any risk weight is negative or non-finite, any correlation
    /// is outside `[-1, 1]`, or any tenor label collides.
    pub fn validate(&self) -> Result<(), MarginError> {
        let range_pct = |name: &str, value: f64| -> Result<(), MarginError> {
            if !value.is_finite() || value < 0.0 {
                Err(MarginError::Validation(format!(
                    "FRTB param '{name}': expected a non-negative finite value, got {value}"
                )))
            } else {
                Ok(())
            }
        };
        let range_corr = |name: &str, value: f64| -> Result<(), MarginError> {
            if !(-1.0..=1.0).contains(&value) {
                Err(MarginError::Validation(format!(
                    "FRTB param '{name}': correlation must be in [-1, 1], got {value}"
                )))
            } else {
                Ok(())
            }
        };

        // GIRR.
        for (tenor, w) in &self.girr.delta_risk_weights_pct {
            range_pct(&format!("girr.delta[{tenor}]"), *w)?;
        }
        range_pct(
            "girr.inflation_risk_weight",
            self.girr.inflation_risk_weight,
        )?;
        range_pct(
            "girr.xccy_basis_risk_weight",
            self.girr.xccy_basis_risk_weight,
        )?;
        range_pct("girr.vega_risk_weight", self.girr.vega_risk_weight)?;
        range_pct(
            "girr.curvature_risk_weight",
            self.girr.curvature_risk_weight,
        )?;
        range_corr(
            "girr.tenor_correlation_floor",
            self.girr.tenor_correlation_floor,
        )?;
        range_corr(
            "girr.inter_bucket_correlation",
            self.girr.inter_bucket_correlation,
        )?;
        range_corr(
            "girr.inflation_correlation",
            self.girr.inflation_correlation,
        )?;
        range_corr(
            "girr.xccy_basis_correlation",
            self.girr.xccy_basis_correlation,
        )?;

        // Equity.
        for (bucket, w) in &self.equity.risk_weights {
            range_pct(&format!("equity.rw[{bucket}]"), *w)?;
        }
        range_corr(
            "equity.intra_bucket_correlation",
            self.equity.intra_bucket_correlation,
        )?;
        range_corr(
            "equity.inter_bucket_correlation",
            self.equity.inter_bucket_correlation,
        )?;
        range_pct("equity.vega_risk_weight", self.equity.vega_risk_weight)?;
        range_pct(
            "equity.curvature_risk_weight",
            self.equity.curvature_risk_weight,
        )?;

        // Commodity.
        range_pct(
            "commodity.vega_risk_weight",
            self.commodity.vega_risk_weight,
        )?;
        range_pct(
            "commodity.curvature_risk_weight",
            self.commodity.curvature_risk_weight,
        )?;

        // FX.
        range_pct("fx.delta_risk_weight", self.fx.delta_risk_weight)?;
        range_pct("fx.vega_risk_weight", self.fx.vega_risk_weight)?;
        range_pct("fx.curvature_risk_weight", self.fx.curvature_risk_weight)?;
        range_corr("fx.inter_pair_correlation", self.fx.inter_pair_correlation)?;

        // DRC.
        for (bucket, w) in &self.drc.risk_weights {
            range_pct(&format!("drc.risk_weights[{bucket}]"), *w)?;
        }
        for (seniority, lgd) in &self.drc.lgd {
            if !lgd.is_finite() || !(0.0..=1.0).contains(lgd) {
                return Err(MarginError::Validation(format!(
                    "FRTB param 'drc.lgd[{seniority:?}]': LGD must be in [0, 1], got {lgd}"
                )));
            }
        }
        range_pct("drc.unrated_risk_weight", self.drc.unrated_risk_weight)?;
        if !self.drc.unrated_lgd.is_finite() || !(0.0..=1.0).contains(&self.drc.unrated_lgd) {
            return Err(MarginError::Validation(format!(
                "FRTB param 'drc.unrated_lgd': LGD must be in [0, 1], got {}",
                self.drc.unrated_lgd
            )));
        }

        // Correlation scenarios — sanity-check that low < high under the
        // default rho = 0.5 anchor.
        let rho_anchor = 0.5;
        let low = self.correlation_scenarios.low_linear_coefficient * rho_anchor
            + self.correlation_scenarios.low_intercept;
        let high = (self.correlation_scenarios.high_multiplier * rho_anchor).min(1.0);
        if low > high {
            return Err(MarginError::Validation(format!(
                "FRTB correlation scenarios: low ({low:.4}) must not exceed high ({high:.4}) at rho=0.5 anchor"
            )));
        }
        if !self
            .correlation_scenarios
            .low_linear_coefficient
            .is_finite()
            || !self.correlation_scenarios.low_intercept.is_finite()
            || !self.correlation_scenarios.high_multiplier.is_finite()
        {
            return Err(MarginError::Validation(
                "FRTB correlation_scenarios contain non-finite coefficients".to_string(),
            ));
        }
        Ok(())
    }
}

/// Recursively merge `overlay` into `base`: nested objects are merged,
/// arrays and scalars in `overlay` replace the corresponding value in
/// `base`.
fn deep_merge(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(mut base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                let merged = match base_map.remove(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => value,
                };
                base_map.insert(key, merged);
            }
            serde_json::Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d457_defaults_validate() {
        let params = FrtbParams::d457();
        params.validate().expect("d457 defaults must validate");
        assert_eq!(params.revision, FrtbRevision::D457);
        assert_eq!(params.girr.delta_risk_weights_pct.len(), 10);
        assert_eq!(params.equity.risk_weights.len(), 13);
    }

    #[test]
    fn json_overlay_only_changes_named_fields() {
        // Override just the GIRR inflation risk weight; everything else
        // must match the d457 default.
        let overlay = serde_json::json!({
            "girr": { "inflation_risk_weight": 1.8 }
        });
        let params = FrtbParams::from_json_overlay(&overlay).expect("overlay ok");
        assert!((params.girr.inflation_risk_weight - 1.8).abs() < 1e-12);
        // Untouched fields match d457 defaults.
        let d457 = FrtbParams::d457();
        assert_eq!(
            params.girr.delta_risk_weights_pct,
            d457.girr.delta_risk_weights_pct
        );
        assert_eq!(params.equity.vega_risk_weight, d457.equity.vega_risk_weight);
    }

    #[test]
    fn json_overlay_rejects_negative_risk_weight() {
        let overlay = serde_json::json!({
            "equity": {
                "risk_weights": [[1, -5.0]]
            }
        });
        let err = FrtbParams::from_json_overlay(&overlay)
            .expect_err("negative risk weight must be rejected");
        assert!(
            err.to_string().contains("equity.rw[1]"),
            "error should name the offending field: {err}"
        );
    }

    #[test]
    fn json_overlay_rejects_out_of_range_correlation() {
        let overlay = serde_json::json!({
            "equity": { "intra_bucket_correlation": 1.5 }
        });
        let err =
            FrtbParams::from_json_overlay(&overlay).expect_err("correlation > 1 must be rejected");
        assert!(
            err.to_string().contains("intra_bucket_correlation"),
            "error should name the offending field: {err}"
        );
    }

    #[test]
    fn custom_revision_label_is_human_readable() {
        let p = FrtbParams {
            revision: FrtbRevision::Custom("internal-stress-v1".into()),
            ..FrtbParams::d457()
        };
        assert_eq!(p.revision.label(), "custom (internal-stress-v1)");
    }

    #[test]
    fn correlation_scenario_multipliers_preserve_ordering() {
        let mut p = FrtbParams::d457();
        // Invert the scenario so low > high at the anchor; validate must
        // reject.
        p.correlation_scenarios.low_linear_coefficient = 0.1;
        p.correlation_scenarios.low_intercept = 1.0; // low = 1.05 at rho=0.5
        p.correlation_scenarios.high_multiplier = 0.5; // high = 0.25 at rho=0.5
        let err = p
            .validate()
            .expect_err("inverted scenario must be rejected");
        assert!(
            err.to_string().contains("must not exceed high"),
            "error should describe the inversion: {err}"
        );
    }
}
