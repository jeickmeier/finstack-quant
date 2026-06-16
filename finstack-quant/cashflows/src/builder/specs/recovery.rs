//! Recovery model specifications for credit instruments.

use finstack_quant_core::types::Percentage;

/// Recovery model specification.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RecoveryModelSpec {
    /// Recovery rate as fraction (0.0 to 1.0, e.g., 0.40 for 40%)
    pub rate: f64,
    /// Recovery lag in months
    pub recovery_lag: u32,
}

impl RecoveryModelSpec {
    /// Standard recovery with lag.
    ///
    /// # Arguments
    ///
    /// * `rate` - Recovery rate as a decimal share in `[0.0, 1.0]`.
    /// * `recovery_lag` - Number of months between default and recovery cashflow.
    ///
    /// # Returns
    ///
    /// Recovery model with the supplied rate and lag. Call
    /// [`validate`](Self::validate) for range checking.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::RecoveryModelSpec;
    ///
    /// let spec = RecoveryModelSpec::with_lag(0.40, 12);
    /// spec.validate()?;
    /// assert_eq!(spec.recovery_lag, 12);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    pub fn with_lag(rate: f64, recovery_lag: u32) -> Self {
        Self { rate, recovery_lag }
    }

    /// Standard recovery with lag using a typed percentage.
    ///
    /// # Arguments
    ///
    /// * `rate` - Recovery rate as a typed percentage.
    /// * `recovery_lag` - Number of months between default and recovery cashflow.
    ///
    /// # Returns
    ///
    /// Recovery model with the supplied rate converted to decimal form.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::RecoveryModelSpec;
    /// use finstack_quant_core::types::Percentage;
    ///
    /// let spec = RecoveryModelSpec::with_lag_pct(Percentage::new(40.0), 6);
    /// assert_eq!(spec.rate, 0.40);
    /// assert_eq!(spec.recovery_lag, 6);
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    pub fn with_lag_pct(rate: Percentage, recovery_lag: u32) -> Self {
        Self {
            rate: rate.as_decimal(),
            recovery_lag,
        }
    }

    /// Validate the recovery model parameters.
    ///
    /// # Errors
    ///
    /// Returns `Validation` error if:
    /// - `rate` is not in `[0.0, 1.0]`
    /// - `rate` is NaN or infinite
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.rate.is_finite() || !(0.0..=1.0).contains(&self.rate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RecoveryModelSpec rate ({}) must be in [0.0, 1.0] and finite",
                self.rate
            )));
        }
        Ok(())
    }
}
