//! SABR model, smile, parameter, and calibration support.
//!
use finstack_quant_core::{Error, Result};

/// SABR model parameters
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct SabrParameters {
    /// Initial volatility (alpha)
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
    pub alpha: f64,
    /// CEV exponent (beta) - typically 0 to 1
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_closed_unit_interval_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_closed_unit_interval_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::ClosedUnitIntervalF64Wire")
    )]
    pub beta: f64,
    /// Volatility of volatility (nu/volvol)
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_non_negative_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_non_negative_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonNegativeF64Wire")
    )]
    pub nu: f64,
    /// Correlation between asset and volatility (rho)
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_correlation",
        deserialize_with = "finstack_quant_core::wire::deserialize_correlation"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::CorrelationWire")
    )]
    pub rho: f64,
    /// Shift parameter for handling negative rates (optional)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "finstack_quant_core::wire::serialize_optional_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_optional_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::PositiveF64Wire>")
    )]
    pub shift: Option<f64>,
}

impl SabrParameters {
    /// Create new SABR parameters with validation.
    ///
    /// Enforces market-standard parameter bounds:
    /// - α (alpha) > 0: Initial volatility must be positive
    /// - β (beta) ∈ [0, 1]: CEV exponent (0=normal, 1=lognormal)
    /// - ν (nu) ≥ 0: Volatility of volatility must be non-negative
    /// - ρ (rho) ∈ [-1, 1]: Correlation must be valid
    pub fn new(alpha: f64, beta: f64, nu: f64, rho: f64) -> Result<Self> {
        let parameters = Self {
            alpha,
            beta,
            nu,
            rho,
            shift: None,
        };
        parameters.validate()?;
        Ok(parameters)
    }

    /// Validate the complete SABR parameter set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when any parameter is non-finite or lies
    /// outside its market-standard range, including a non-positive shift.
    pub fn validate(&self) -> Result<()> {
        if !self.alpha.is_finite() || self.alpha <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR parameter α (alpha) must be positive, got: {:.6}",
                self.alpha
            )));
        }
        if !self.beta.is_finite() || !(0.0..=1.0).contains(&self.beta) {
            return Err(Error::Validation(format!(
                "SABR parameter β (beta) must be in [0, 1], got: {:.6}",
                self.beta
            )));
        }
        if !self.nu.is_finite() || self.nu < 0.0 {
            return Err(Error::Validation(format!(
                "SABR parameter ν (nu) must be non-negative, got: {:.6}",
                self.nu
            )));
        }
        if !self.rho.is_finite() || !(-1.0..=1.0).contains(&self.rho) {
            return Err(Error::Validation(format!(
                "SABR parameter ρ (rho) must be in [-1, 1], got: {:.6}",
                self.rho
            )));
        }
        if let Some(shift) = self.shift {
            if !shift.is_finite() || shift <= 0.0 {
                return Err(Error::Validation(format!(
                    "SABR shift parameter must be positive for negative rate support, got: {shift:.6}"
                )));
            }
        }
        Ok(())
    }

    /// Rates market standard (beta = 0.5).
    ///
    /// # Errors
    ///
    /// Returns an error if any SABR parameter is invalid.
    pub fn rates_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.5, nu, rho)
    }

    /// Create new SABR parameters with shift for negative rates.
    ///
    /// Same validation as `new()` plus shift validation:
    /// - shift > 0: Shift must be positive for negative rate support
    ///
    /// # Arguments
    ///
    /// * `alpha` - SABR or SVI alpha parameter controlling the overall volatility level
    /// * `beta` - SABR beta CEV exponent controlling the backbone between 0 and 1
    /// * `nu` - SABR vol-of-vol parameter controlling smile wing curvature
    /// * `rho` - Instantaneous correlation between Brownian drivers, in `[-1, 1]`
    /// * `shift` - Displacement shift applied to forward/strike for negative-rate SABR
    pub fn new_with_shift(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        let mut params = Self::new(alpha, beta, nu, rho)?;
        params.shift = Some(shift);
        params.validate()?;
        Ok(params)
    }

    /// Create parameters for normal SABR (beta = 0)
    pub fn normal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.0, nu, rho)
    }

    /// Create parameters for lognormal SABR (beta = 1)
    pub fn lognormal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 1.0, nu, rho)
    }

    /// Create parameters for shifted normal SABR (beta = 0, with shift)
    pub fn shifted_normal(alpha: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        Self::new_with_shift(alpha, 0.0, nu, rho, shift)
    }

    /// Create parameters for shifted lognormal SABR (beta = 1, with shift)
    pub fn shifted_lognormal(alpha: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        Self::new_with_shift(alpha, 1.0, nu, rho, shift)
    }

    /// Create default equity-standard SABR parameters.
    ///
    /// Returns parameters suitable as a starting point for equity options:
    /// - α = 0.20 (20% initial volatility)
    /// - β = 1.0 (lognormal, standard for equities)
    /// - ν = 0.30 (30% vol-of-vol)
    /// - ρ = -0.20 (mild negative correlation, typical equity skew)
    ///
    /// These are reasonable defaults for calibration initialization but
    /// should always be calibrated to market data for production use.
    #[must_use]
    pub const fn equity_default() -> Self {
        Self {
            alpha: 0.20,
            beta: 1.0,
            nu: 0.30,
            rho: -0.20,
            shift: None,
        }
    }

    /// Create default rates-standard SABR parameters.
    ///
    /// Returns parameters suitable as a starting point for rates options:
    /// - α = 0.02 (2% normal vol level, typical for rates)
    /// - β = 0.5 (mixed normal/lognormal, common for rates)
    /// - ν = 0.30 (30% vol-of-vol)
    /// - ρ = 0.0 (neutral correlation as starting point)
    ///
    /// These are reasonable defaults for calibration initialization but
    /// should always be calibrated to market data for production use.
    #[must_use]
    pub const fn rates_default() -> Self {
        Self {
            alpha: 0.02,
            beta: 0.5,
            nu: 0.30,
            rho: 0.0,
            shift: None,
        }
    }

    /// Get the shift parameter
    pub fn shift(&self) -> Option<f64> {
        self.shift
    }

    /// Check if this is a shifted SABR model
    pub fn is_shifted(&self) -> bool {
        self.shift.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serde_and_validation_reject_the_same_invalid_parameters() {
        let invalid = [
            SabrParameters {
                alpha: -0.1,
                ..SabrParameters::rates_default()
            },
            SabrParameters {
                beta: 1.1,
                ..SabrParameters::rates_default()
            },
            SabrParameters {
                nu: -0.1,
                ..SabrParameters::rates_default()
            },
            SabrParameters {
                rho: 1.1,
                ..SabrParameters::rates_default()
            },
            SabrParameters {
                shift: Some(0.0),
                ..SabrParameters::rates_default()
            },
        ];
        for parameters in invalid {
            assert!(parameters.validate().is_err());
            assert!(serde_json::to_value(parameters).is_err());
        }

        for value in [
            json!({"alpha": -0.1, "beta": 0.5, "nu": 0.3, "rho": 0.0}),
            json!({"alpha": 0.02, "beta": 1.1, "nu": 0.3, "rho": 0.0}),
            json!({"alpha": 0.02, "beta": 0.5, "nu": -0.1, "rho": 0.0}),
            json!({"alpha": 0.02, "beta": 0.5, "nu": 0.3, "rho": 1.1}),
            json!({"alpha": 0.02, "beta": 0.5, "nu": 0.3, "rho": 0.0, "shift": 0.0}),
            json!({"alpha": 0.02, "beta": 0.5, "nu": 0.3, "rho": 0.0, "extra": true}),
        ] {
            assert!(serde_json::from_value::<SabrParameters>(value).is_err());
        }
    }

    #[test]
    fn generated_schema_carries_all_sabr_bounds() {
        let schema =
            serde_json::to_value(schemars::schema_for!(SabrParameters)).expect("SABR schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["$defs"]["PositiveF64Wire"]["exclusiveMinimum"], 0.0);
        assert_eq!(schema["$defs"]["ClosedUnitIntervalF64Wire"]["minimum"], 0.0);
        assert_eq!(schema["$defs"]["ClosedUnitIntervalF64Wire"]["maximum"], 1.0);
        assert_eq!(schema["$defs"]["NonNegativeF64Wire"]["minimum"], 0.0);
        assert_eq!(schema["$defs"]["CorrelationWire"]["minimum"], -1.0);
        assert_eq!(schema["$defs"]["CorrelationWire"]["maximum"], 1.0);
    }
}
