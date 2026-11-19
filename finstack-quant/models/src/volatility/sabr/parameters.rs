//! SABR model, smile, parameter, and calibration support.
//!
use finstack_quant_core::Result;
use finstack_quant_core::market_data::surfaces::SabrParameterData;

/// Parameters of a SABR backbone, with an optional signed rate displacement.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(from = "SabrParameterData")]
pub struct SabrParameters {
    /// Positive initial volatility level.
    pub alpha: f64,
    /// CEV elasticity in the closed unit interval.
    pub beta: f64,
    /// Nonnegative volatility of volatility.
    pub nu: f64,
    /// Brownian correlation strictly between minus one and one.
    pub rho: f64,
    /// Finite model displacement in forward and strike units.
    pub shift: Option<f64>,
}

impl From<SabrParameterData> for SabrParameters {
    fn from(node: SabrParameterData) -> Self {
        Self {
            alpha: node.alpha(),
            beta: node.beta(),
            rho: node.rho(),
            nu: node.nu(),
            shift: node.shift(),
        }
    }
}

impl serde::Serialize for SabrParameters {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.to_node()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl SabrParameters {
    /// Create new SABR parameters with validation.
    ///
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `beta` - CEV elasticity between zero and one, inclusive.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
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
    /// Returns [`finstack_quant_core::Error::Validation`] when any parameter is non-finite or lies
    /// outside its market-standard range, including a non-finite shift.
    pub fn validate(&self) -> Result<()> {
        self.to_node().map(|_| ())
    }

    fn to_node(&self) -> Result<SabrParameterData> {
        SabrParameterData::new(self.alpha, self.beta, self.nu, self.rho, self.shift)
    }

    /// Rates market standard (beta = 0.5).
    ///
    /// # Errors
    ///
    /// Returns an error if any SABR parameter is invalid.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
    pub fn rates_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.5, nu, rho)
    }

    /// Create new SABR parameters with shift for negative rates.
    ///
    /// Same validation as `new()` plus shift validation:
    /// - The signed shift must be finite; Black evaluation checks effective rates.
    ///
    /// # Arguments
    ///
    /// * `alpha` - SABR or SVI alpha parameter controlling the overall volatility level
    /// * `beta` - SABR beta CEV exponent controlling the backbone between 0 and 1
    /// * `nu` - SABR vol-of-vol parameter controlling smile wing curvature
    /// * `rho` - Instantaneous correlation between Brownian drivers, in `(-1, 1)`
    /// * `shift` - Displacement shift applied to forward/strike for negative-rate SABR
    pub fn new_with_shift(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        let mut params = Self::new(alpha, beta, nu, rho)?;
        params.shift = Some(shift);
        params.validate()?;
        Ok(params)
    }

    /// Create an additive SABR backbone (beta = 0).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
    pub fn normal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.0, nu, rho)
    }

    /// Create a multiplicative SABR backbone (beta = 1).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
    pub fn lognormal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 1.0, nu, rho)
    }

    /// Create a displaced additive SABR backbone (beta = 0).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
    /// * `shift` - Finite displacement added to rates and strikes in their native units.
    pub fn shifted_normal(alpha: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        Self::new_with_shift(alpha, 0.0, nu, rho, shift)
    }

    /// Create a displaced multiplicative SABR backbone (beta = 1).
    ///
    /// # Arguments
    ///
    /// * `alpha` - Positive initial volatility coefficient, expressed in the units of the CEV backbone.
    /// * `nu` - Nonnegative volatility of volatility per square root of a year.
    /// * `rho` - Instantaneous Brownian correlation strictly between −1 and 1.
    /// * `shift` - Finite displacement added to rates and strikes in their native units.
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
                rho: 1.0,
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
            json!({"alpha": 0.02, "beta": 0.5, "nu": 0.3, "rho": -1.0, "shift": 0.0}),
            json!({"alpha": 0.02, "beta": 0.5, "nu": 0.3, "rho": 0.0, "extra": true}),
        ] {
            assert!(serde_json::from_value::<SabrParameters>(value).is_err());
        }
    }

    #[test]
    fn generated_schema_carries_all_sabr_bounds() {
        let schema =
            serde_json::to_value(schemars::schema_for!(SabrParameters)).expect("SABR schema");
        let node = &schema["$defs"]["SabrParameterData"];
        let node = if node.is_null() { &schema } else { node };
        assert_eq!(node["additionalProperties"], false);
        assert_eq!(node["properties"]["alpha"]["exclusiveMinimum"], 0.0);
        assert_eq!(node["properties"]["rho"]["exclusiveMinimum"], -1.0);
        assert_eq!(node["properties"]["rho"]["exclusiveMaximum"], 1.0);
    }
}
