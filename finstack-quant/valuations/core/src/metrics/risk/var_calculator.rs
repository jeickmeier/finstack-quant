//! Historical VaR configuration and method wire types.
//!
//! Full Historical VaR calculation (`calculate_var`, `VarResult`) lives in
//! `finstack_quant_valuations::metrics::risk::var_calculator`.

use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;

/// VaR calculation method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VarMethod {
    /// Full revaluation of instrument under each historical scenario.
    ///
    /// Most accurate method - reprices the instrument under each historical
    /// market scenario. Captures all non-linearities and path dependencies.
    #[default]
    FullRevaluation,

    /// Taylor approximation using sensitivities (Greeks).
    ///
    /// Computes first and second directional derivatives by selected-model
    /// repricing along each exact historical shock. Curve interpolation support,
    /// quote calibration, surface nodes, and reporting-currency FX all remain
    /// part of the direction. It may be inaccurate for large nonlinear shocks
    /// and need not be faster than full revaluation. At zero volatility a
    /// second-order forward stencil replaces the central stencil.
    TaylorApproximation,
}

/// Configuration for VaR calculation.
///
/// Controls statistical properties such as confidence level and pricing method.
/// The historical window/observation count is derived from
/// [`crate::metrics::risk::MarketHistory`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct VarConfig {
    /// Confidence level (e.g., 0.95 for 95% VaR, 0.99 for 99% VaR)
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_open_unit_interval_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_open_unit_interval_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::OpenUnitIntervalF64Wire")
    )]
    pub confidence_level: f64,

    /// VaR calculation method
    pub method: VarMethod,

    /// Optional reporting currency for portfolio aggregation.
    ///
    /// When omitted, same-currency portfolios use their natural currency.
    /// Mixed-currency portfolios must set this explicitly.
    pub reporting_currency: Option<Currency>,
}

impl Default for VarConfig {
    fn default() -> Self {
        Self::var_95()
    }
}

impl VarConfig {
    /// Create a new VaR configuration with standard settings.
    ///
    /// # Arguments
    ///
    /// * `confidence_level` - Confidence level (e.g., 0.95, 0.99)
    pub fn new(confidence_level: f64) -> Self {
        Self {
            confidence_level,
            method: VarMethod::FullRevaluation,
            reporting_currency: None,
        }
    }

    /// Standard 95% VaR configuration.
    pub fn var_95() -> Self {
        Self::new(0.95)
    }

    /// Standard 99% VaR configuration.
    pub fn var_99() -> Self {
        Self::new(0.99)
    }

    /// Set the calculation method.
    pub fn with_method(mut self, method: VarMethod) -> Self {
        self.method = method;
        self
    }

    /// Set the reporting currency for portfolio aggregation.
    pub fn with_reporting_currency(mut self, currency: Currency) -> Self {
        self.reporting_currency = Some(currency);
        self
    }

    /// Validate that this VaR configuration is numerically well-defined.
    ///
    /// # Errors
    ///
    /// Returns an error when `confidence_level` is not finite or is outside
    /// the open interval `(0, 1)`.
    pub fn validate(&self) -> Result<()> {
        validate_confidence_level(self.confidence_level)
    }
}

fn validate_confidence_level(confidence_level: f64) -> Result<()> {
    if confidence_level.is_finite() && confidence_level > 0.0 && confidence_level < 1.0 {
        Ok(())
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "VaR confidence level must be finite and strictly between 0 and 1, got {confidence_level}"
        )))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn var_config_wire_contract_is_strict_and_bounded() {
        for confidence_level in [0.0, 1.0, -0.01, 1.01] {
            let value = serde_json::json!({ "confidence_level": confidence_level });
            assert!(
                serde_json::from_value::<VarConfig>(value).is_err(),
                "confidence level {confidence_level} must be rejected"
            );
        }
        assert!(
            serde_json::from_value::<VarConfig>(serde_json::json!({
                "confidence_level": 0.95,
                "unexpected": true
            }))
            .is_err()
        );

        let schema =
            serde_json::to_value(schemars::schema_for!(VarConfig)).expect("VaR config schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["$defs"]["OpenUnitIntervalF64Wire"]["exclusiveMinimum"],
            0.0
        );
        assert_eq!(
            schema["$defs"]["OpenUnitIntervalF64Wire"]["exclusiveMaximum"],
            1.0
        );
    }
}
