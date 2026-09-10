//! Quanto adjustment specification for cross-currency instruments.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::{CurveId, PriceId};

/// Quanto adjustment parameters for instruments where payoff currency differs from
/// underlying currency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QuantoSpec {
    /// Currency in which the underlying asset is quoted and financed.
    pub asset_currency: Currency,
    /// Discount curve for financing the underlying in its asset currency.
    pub asset_discount_curve_id: CurveId,
    /// Correlation between the asset price and payoff-currency units per asset-currency unit.
    /// Must be in [-1, 1].
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_correlation",
        deserialize_with = "finstack_quant_core::wire::deserialize_correlation"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::CorrelationWire")
    )]
    pub correlation: f64,
    /// FX volatility surface ID (required for quanto vol lookup).
    pub fx_vol_surface_id: CurveId,
    /// Required positive FX spot scalar in payoff-currency units per asset-currency unit.
    /// A monetary scalar must use the payoff currency. The FX surface is queried
    /// at the forward FX rate implied by the asset and payoff discount curves.
    pub fx_spot_id: PriceId,
}

impl QuantoSpec {
    /// Create a new quanto adjustment specification with validation.
    ///
    /// # Arguments
    /// * `correlation` - Correlation between the underlying asset and the FX
    ///   rate. Must be a finite value in `[-1.0, 1.0]`.
    /// * `fx_vol_surface_id` - FX volatility surface identifier, quoted against payoff currency per asset currency.
    /// * `asset_currency` - Currency of the underlying asset price and financing.
    /// * `asset_discount_curve_id` - Asset-currency financing curve identifier.
    /// * `fx_spot_id` - Required FX spot identifier, in payoff currency per asset currency.
    ///
    /// # Errors
    /// Returns an error if `correlation` is not finite or lies outside
    /// `[-1.0, 1.0]`. A correlation outside the unit interval has no
    /// probabilistic meaning and produces a quanto drift adjustment that
    /// silently corrupts every dependent valuation.
    pub fn new(
        correlation: f64,
        fx_vol_surface_id: impl Into<CurveId>,
        asset_currency: Currency,
        asset_discount_curve_id: impl Into<CurveId>,
        fx_spot_id: impl Into<PriceId>,
    ) -> finstack_quant_core::Result<Self> {
        let spec = Self {
            correlation,
            fx_vol_surface_id: fx_vol_surface_id.into(),
            asset_currency,
            asset_discount_curve_id: asset_discount_curve_id.into(),
            fx_spot_id: fx_spot_id.into(),
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Validate that the correlation is a finite value in `[-1.0, 1.0]`.
    ///
    /// Use this to enforce the documented `correlation ∈ [-1, 1]` invariant on a
    /// `QuantoSpec` obtained by deserialization or struct-literal construction,
    /// where the [`QuantoSpec::new`] constructor was bypassed.
    ///
    /// # Errors
    /// Returns an error stating the attempted value and the required range when
    /// `correlation` is not finite or lies outside `[-1.0, 1.0]`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoSpec.correlation must be a finite value in [-1.0, 1.0]; \
                 attempted to construct a quanto spec with correlation = {}. \
                 A correlation outside the unit interval is not a valid \
                 probabilistic quantity and corrupts the quanto drift adjustment.",
                self.correlation
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_correlation_above_one() {
        // Failure mode: correlation > 1 is documented as invalid ([-1, 1]) but
        // was previously unenforced at the constructor boundary.
        let err = QuantoSpec::new(1.5, "FXVOL", Currency::EUR, "EUR-OIS", "EURUSD")
            .expect_err("correlation 1.5 is outside [-1, 1] and must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("correlation") && msg.contains("[-1.0, 1.0]"),
            "error should explain the correlation bound: {msg}"
        );
    }

    #[test]
    fn new_rejects_correlation_below_minus_one_and_non_finite() {
        assert!(QuantoSpec::new(-1.0001, "FXVOL", Currency::EUR, "EUR-OIS", "EURUSD").is_err());
        assert!(QuantoSpec::new(f64::NAN, "FXVOL", Currency::EUR, "EUR-OIS", "EURUSD").is_err());
        assert!(
            QuantoSpec::new(f64::INFINITY, "FXVOL", Currency::EUR, "EUR-OIS", "EURUSD").is_err()
        );
    }

    #[test]
    fn new_accepts_correlation_within_unit_interval() {
        for rho in [-1.0, -0.5, 0.0, 0.3, 1.0] {
            let spec = QuantoSpec::new(rho, "FXVOL", Currency::EUR, "EUR-OIS", "EURUSD")
                .unwrap_or_else(|e| panic!("correlation {rho} should be accepted: {e}"));
            assert!((spec.correlation - rho).abs() < 1e-15);
            assert_eq!(spec.fx_spot_id.as_str(), "EURUSD");
        }
    }

    #[test]
    fn validate_catches_out_of_range_correlation_on_deserialized_spec() {
        // A struct-literal / deserialized spec that bypassed `new` must still be
        // checkable via `validate`.
        let spec = QuantoSpec {
            correlation: 2.0,
            fx_vol_surface_id: CurveId::new("FXVOL"),
            asset_currency: Currency::EUR,
            asset_discount_curve_id: CurveId::new("EUR-OIS"),
            fx_spot_id: PriceId::new("EURUSD"),
        };
        assert!(spec.validate().is_err());

        let ok = QuantoSpec {
            correlation: 0.25,
            fx_vol_surface_id: CurveId::new("FXVOL"),
            asset_currency: Currency::EUR,
            asset_discount_curve_id: CurveId::new("EUR-OIS"),
            fx_spot_id: PriceId::new("FXSPOT"),
        };
        assert!(ok.validate().is_ok());
    }
}
