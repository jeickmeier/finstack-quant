//! Quanto adjustment specification for cross-currency instruments.

use finstack_core::types::CurveId;

/// Quanto adjustment parameters for instruments where payoff currency differs from
/// underlying currency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct QuantoSpec {
    /// Correlation between the underlying asset and the FX rate.
    /// Must be in [-1, 1].
    pub correlation: f64,
    /// FX volatility surface ID (required for quanto vol lookup).
    pub fx_vol_surface_id: CurveId,
    /// FX spot price identifier (for proper quanto vol lookup).
    /// Falls back to ATM approximation (1.0) if not provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_spot_id: Option<String>,
}

impl QuantoSpec {
    /// Create a new quanto adjustment specification with validation.
    ///
    /// # Arguments
    /// * `correlation` - Correlation between the underlying asset and the FX
    ///   rate. Must be a finite value in `[-1.0, 1.0]`.
    /// * `fx_vol_surface_id` - FX volatility surface identifier.
    ///
    /// # Errors
    /// Returns an error if `correlation` is not finite or lies outside
    /// `[-1.0, 1.0]`. A correlation outside the unit interval has no
    /// probabilistic meaning and produces a quanto drift adjustment that
    /// silently corrupts every dependent valuation.
    pub fn new(
        correlation: f64,
        fx_vol_surface_id: impl Into<CurveId>,
    ) -> finstack_core::Result<Self> {
        let spec = Self {
            correlation,
            fx_vol_surface_id: fx_vol_surface_id.into(),
            fx_spot_id: None,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Set the FX spot price identifier used for quanto vol lookup.
    #[must_use]
    pub fn with_fx_spot_id(mut self, fx_spot_id: impl Into<String>) -> Self {
        self.fx_spot_id = Some(fx_spot_id.into());
        self
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
    pub fn validate(&self) -> finstack_core::Result<()> {
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_core::Error::Validation(format!(
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
        let err = QuantoSpec::new(1.5, "FXVOL")
            .expect_err("correlation 1.5 is outside [-1, 1] and must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("correlation") && msg.contains("[-1.0, 1.0]"),
            "error should explain the correlation bound: {msg}"
        );
    }

    #[test]
    fn new_rejects_correlation_below_minus_one_and_non_finite() {
        assert!(QuantoSpec::new(-1.0001, "FXVOL").is_err());
        assert!(QuantoSpec::new(f64::NAN, "FXVOL").is_err());
        assert!(QuantoSpec::new(f64::INFINITY, "FXVOL").is_err());
    }

    #[test]
    fn new_accepts_correlation_within_unit_interval() {
        for rho in [-1.0, -0.5, 0.0, 0.3, 1.0] {
            let spec = QuantoSpec::new(rho, "FXVOL")
                .unwrap_or_else(|e| panic!("correlation {rho} should be accepted: {e}"));
            assert!((spec.correlation - rho).abs() < 1e-15);
            assert!(spec.fx_spot_id.is_none());
        }
    }

    #[test]
    fn validate_catches_out_of_range_correlation_on_deserialized_spec() {
        // A struct-literal / deserialized spec that bypassed `new` must still be
        // checkable via `validate`.
        let spec = QuantoSpec {
            correlation: 2.0,
            fx_vol_surface_id: CurveId::new("FXVOL"),
            fx_spot_id: None,
        };
        assert!(spec.validate().is_err());

        let ok = QuantoSpec {
            correlation: 0.25,
            fx_vol_surface_id: CurveId::new("FXVOL"),
            fx_spot_id: Some("FXSPOT".to_string()),
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn with_fx_spot_id_sets_optional_identifier() {
        let spec = QuantoSpec::new(0.4, "FXVOL")
            .expect("valid correlation")
            .with_fx_spot_id("EURUSD-SPOT");
        assert_eq!(spec.fx_spot_id.as_deref(), Some("EURUSD-SPOT"));
    }
}
